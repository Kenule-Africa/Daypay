//! Controlled ledger use cases (IMP-P5-007–016).
//!
//! The PostgreSQL ledger functions are the only way to post. This module
//! defines their typed commands, the store contract the infrastructure
//! implements, and [`LedgerService`], which validates requests and applies the
//! workflow-level [`FinancialGate`] before any customer funds move.
use deypay_domain::ledger::{EventIdentity, JournalDraft, valid_key};
use deypay_domain::units::{MinorUnits, PostingAmount};
use std::future::Future;
use uuid::Uuid;

/// Failures reported by the ledger. Names follow ledger spec §38.6.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LedgerError {
    /// Malformed amount, key, line set or other input.
    InvalidRequest,
    /// A line's asset differs from its account's asset.
    AssetMismatch,
    /// Debits and credits differ for at least one asset.
    UnbalancedJournal,
    /// The posting would overdraw an account that cannot go negative.
    InsufficientFunds,
    /// The account is frozen or closed.
    AccountRestricted,
    /// The accounting event was already posted with a different request.
    DuplicateAccountingEvent,
    /// The journal cannot be reversed (already reversed, a reversal, or a reservation movement).
    InvalidReversal,
    /// The reservation's lifecycle does not allow the operation.
    ReservationState,
    /// Unknown account, reservation, journal or request.
    NotFound,
    /// The customer is not eligible to spend funds.
    NotEligible,
    /// The asset is on hold after a critical reconciliation discrepancy.
    AssetOnHold,
    /// The posting is outside what this interface may do.
    NotPermitted,
    /// Missing permission, self-approval or an undecided correction.
    ApprovalRequired,
    /// Lock timeout or repeated serialization failure; safe to retry with the same identity.
    ConcurrentModification,
    /// The ledger database could not be reached or failed unexpectedly. The
    /// outcome may be unknown: retry with the same identity, never a new one.
    Unavailable,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Actor {
    System,
    Service(Uuid),
    Customer(Uuid),
    Admin(Uuid),
}

impl Actor {
    pub const fn kind(self) -> &'static str {
        match self {
            Self::System => "SYSTEM",
            Self::Service(_) => "SERVICE",
            Self::Customer(_) => "CUSTOMER",
            Self::Admin(_) => "ADMIN",
        }
    }

    pub const fn id(self) -> Option<Uuid> {
        match self {
            Self::System => None,
            Self::Service(id) | Self::Customer(id) | Self::Admin(id) => Some(id),
        }
    }
}

/// A workflow posting such as a recognized deposit or a settlement. It may
/// credit customers but can never debit customer funds or touch reservations.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PostingCommand {
    pub identity: EventIdentity,
    pub source_transaction: Option<Uuid>,
    pub journal: JournalDraft,
    pub description: Option<String>,
    pub actor: Actor,
    pub correlation_id: Option<Uuid>,
}

/// Customer-to-customer transfer of available funds (ledger spec §14).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TransferCommand {
    pub operation_key: String,
    pub sender: Uuid,
    pub recipient: Uuid,
    pub asset_id: Uuid,
    pub amount: PostingAmount,
    pub source_transaction: Option<Uuid>,
    pub actor: Actor,
    pub correlation_id: Option<Uuid>,
}

/// Moves funds from available to reserved for one external operation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReserveCommand {
    pub operation_key: String,
    pub customer: Uuid,
    pub asset_id: Uuid,
    pub amount: PostingAmount,
    pub transaction: Option<Uuid>,
    /// For example `WITHDRAWAL_RESERVED` or `BANK_PAYMENT_RESERVED`.
    pub event_type: String,
    pub actor: Actor,
    pub correlation_id: Option<Uuid>,
}

/// Settles reserved funds to platform accounts on verified settlement evidence.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConsumeCommand {
    pub reservation: Uuid,
    pub identity: EventIdentity,
    pub credits: Vec<(Uuid, PostingAmount)>,
    pub evidence_reference: String,
    pub actor: Actor,
    pub correlation_id: Option<Uuid>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReleaseEvidence {
    /// Verified proof the external operation did not and will not execute.
    DefinitiveNonExecution,
    /// Cancellation before anything was submitted externally.
    PermittedCancellation,
    /// The unneeded remainder after verified settlement.
    SettlementRemainder,
}

impl ReleaseEvidence {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::DefinitiveNonExecution => "DEFINITIVE_NON_EXECUTION",
            Self::PermittedCancellation => "PERMITTED_CANCELLATION",
            Self::SettlementRemainder => "SETTLEMENT_REMAINDER",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReleaseCommand {
    pub reservation: Uuid,
    pub economic_key: String,
    pub purpose: String,
    pub amount: PostingAmount,
    pub evidence: ReleaseEvidence,
    pub evidence_reference: String,
    pub actor: Actor,
    pub correlation_id: Option<Uuid>,
}

/// External execution state of a reserved operation. SUBMITTED and UNKNOWN
/// hold the reservation until definitive evidence arrives.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExecutionState {
    NotSubmitted,
    Submitted,
    Unknown,
    Executed,
}

impl ExecutionState {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::NotSubmitted => "NOT_SUBMITTED",
            Self::Submitted => "SUBMITTED",
            Self::Unknown => "UNKNOWN",
            Self::Executed => "EXECUTED",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        Some(match value {
            "NOT_SUBMITTED" => Self::NotSubmitted,
            "SUBMITTED" => Self::Submitted,
            "UNKNOWN" => Self::Unknown,
            "EXECUTED" => Self::Executed,
            _ => return None,
        })
    }
}

/// Ledger spec §38.5. `replayed` is true when an identical request had
/// already posted and the existing journal is returned.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PostingReceipt {
    pub journal_id: Uuid,
    pub accounting_event_id: Uuid,
    pub posted_at: String,
    pub replayed: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReservationReceipt {
    pub reservation_id: Uuid,
    pub journal_id: Uuid,
    pub posted_at: String,
    pub replayed: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CustomerAccounts {
    pub available: Uuid,
    pub reserved: Uuid,
}

/// Available and reserved are separate liabilities; nothing is subtracted twice.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CustomerBalance {
    pub available: MinorUnits,
    pub reserved: MinorUnits,
    pub total: MinorUnits,
}

pub trait LedgerStore: Sync {
    fn open_customer_accounts(
        &self,
        wallet_profile: Uuid,
    ) -> impl Future<Output = Result<CustomerAccounts, LedgerError>> + Send;
    fn post(
        &self,
        command: &PostingCommand,
    ) -> impl Future<Output = Result<PostingReceipt, LedgerError>> + Send;
    fn transfer(
        &self,
        command: &TransferCommand,
    ) -> impl Future<Output = Result<PostingReceipt, LedgerError>> + Send;
    fn reserve(
        &self,
        command: &ReserveCommand,
    ) -> impl Future<Output = Result<ReservationReceipt, LedgerError>> + Send;
    fn record_execution(
        &self,
        reservation: Uuid,
        state: ExecutionState,
        evidence_reference: &str,
    ) -> impl Future<Output = Result<ExecutionState, LedgerError>> + Send;
    fn consume(
        &self,
        command: &ConsumeCommand,
    ) -> impl Future<Output = Result<PostingReceipt, LedgerError>> + Send;
    fn release(
        &self,
        command: &ReleaseCommand,
    ) -> impl Future<Output = Result<PostingReceipt, LedgerError>> + Send;
    fn customer_balance(
        &self,
        customer: Uuid,
        asset_id: Uuid,
    ) -> impl Future<Output = Result<Option<CustomerBalance>, LedgerError>> + Send;
}

/// A customer-initiated movement of funds that needs financial authorization.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GatedOperation<'a> {
    Transfer {
        recipient: Uuid,
        asset_id: Uuid,
        amount: PostingAmount,
    },
    Reserve {
        asset_id: Uuid,
        amount: PostingAmount,
        event_type: &'a str,
    },
}

/// Workflow-level financial authorization: KYC eligibility, restrictions,
/// limits and step-up (FRS §23, security SEC-TXN-*). The database additionally
/// refuses spending by customers who are not KYC-approved, are restricted, or
/// whose wallet is inactive, but that minimum is not the approved policy.
pub trait FinancialGate: Sync {
    fn authorize(
        &self,
        customer: Uuid,
        operation: GatedOperation<'_>,
    ) -> impl Future<Output = Result<(), LedgerError>> + Send;
}

/// The only gate shipped until the eligibility policy, limits and their
/// persistence are approved and implemented (BLOCKERS: KYC eligibility and
/// restrictions). It refuses every customer-initiated movement, so the ledger
/// stays fail-closed in every environment.
#[derive(Debug, Clone, Copy, Default)]
pub struct NoApprovedPolicy;

impl FinancialGate for NoApprovedPolicy {
    async fn authorize(&self, _: Uuid, _: GatedOperation<'_>) -> Result<(), LedgerError> {
        Err(LedgerError::NotEligible)
    }
}

pub struct LedgerService<S, G> {
    store: S,
    gate: G,
}

fn require_evidence(reference: &str) -> Result<(), LedgerError> {
    if reference.is_empty()
        || reference.chars().count() > 512
        || reference.chars().any(char::is_control)
    {
        return Err(LedgerError::InvalidRequest);
    }
    Ok(())
}

impl<S: LedgerStore, G: FinancialGate> LedgerService<S, G> {
    pub fn new(store: S, gate: G) -> Self {
        Self { store, gate }
    }

    pub fn store(&self) -> &S {
        &self.store
    }

    /// Creates the zero-balance available and reserved accounts for a wallet
    /// profile. No journal is posted and no value is created.
    pub async fn open_customer_accounts(
        &self,
        wallet_profile: Uuid,
    ) -> Result<CustomerAccounts, LedgerError> {
        self.store.open_customer_accounts(wallet_profile).await
    }

    pub async fn post(&self, command: &PostingCommand) -> Result<PostingReceipt, LedgerError> {
        if command
            .description
            .as_deref()
            .is_some_and(|text| text.chars().count() > 500 || text.chars().any(char::is_control))
        {
            return Err(LedgerError::InvalidRequest);
        }
        self.store.post(command).await
    }

    pub async fn transfer(&self, command: &TransferCommand) -> Result<PostingReceipt, LedgerError> {
        if !valid_key(&command.operation_key) || command.sender == command.recipient {
            return Err(LedgerError::InvalidRequest);
        }
        let operation = GatedOperation::Transfer {
            recipient: command.recipient,
            asset_id: command.asset_id,
            amount: command.amount,
        };
        self.gate.authorize(command.sender, operation).await?;
        self.store.transfer(command).await
    }

    pub async fn reserve(
        &self,
        command: &ReserveCommand,
    ) -> Result<ReservationReceipt, LedgerError> {
        if !valid_key(&command.operation_key)
            || EventIdentity::new(&command.event_type, &command.operation_key, "RESERVE").is_err()
        {
            return Err(LedgerError::InvalidRequest);
        }
        let operation = GatedOperation::Reserve {
            asset_id: command.asset_id,
            amount: command.amount,
            event_type: &command.event_type,
        };
        self.gate.authorize(command.customer, operation).await?;
        self.store.reserve(command).await
    }

    pub async fn record_execution(
        &self,
        reservation: Uuid,
        state: ExecutionState,
        evidence_reference: &str,
    ) -> Result<ExecutionState, LedgerError> {
        require_evidence(evidence_reference)?;
        self.store
            .record_execution(reservation, state, evidence_reference)
            .await
    }

    pub async fn consume(&self, command: &ConsumeCommand) -> Result<PostingReceipt, LedgerError> {
        require_evidence(&command.evidence_reference)?;
        if command.credits.is_empty() || command.credits.len() > 63 {
            return Err(LedgerError::InvalidRequest);
        }
        self.store.consume(command).await
    }

    pub async fn release(&self, command: &ReleaseCommand) -> Result<PostingReceipt, LedgerError> {
        require_evidence(&command.evidence_reference)?;
        EventIdentity::new(
            "RESERVATION_RELEASED",
            &command.economic_key,
            &command.purpose,
        )
        .map_err(|_| LedgerError::InvalidRequest)?;
        self.store.release(command).await
    }

    pub async fn customer_balance(
        &self,
        customer: Uuid,
        asset_id: Uuid,
    ) -> Result<Option<CustomerBalance>, LedgerError> {
        self.store.customer_balance(customer, asset_id).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;
    use std::sync::atomic::{AtomicBool, Ordering};

    /// Records calls; returns fixed receipts. The ledger rules themselves are
    /// tested against PostgreSQL in deypay-infrastructure/tests/ledger.rs.
    #[derive(Default)]
    struct Recorder(Mutex<Vec<&'static str>>);

    fn receipt() -> PostingReceipt {
        PostingReceipt {
            journal_id: Uuid::nil(),
            accounting_event_id: Uuid::nil(),
            posted_at: String::new(),
            replayed: false,
        }
    }

    impl LedgerStore for Recorder {
        async fn open_customer_accounts(&self, _: Uuid) -> Result<CustomerAccounts, LedgerError> {
            self.0.lock().unwrap().push("open");
            Ok(CustomerAccounts {
                available: Uuid::nil(),
                reserved: Uuid::nil(),
            })
        }
        async fn post(&self, _: &PostingCommand) -> Result<PostingReceipt, LedgerError> {
            self.0.lock().unwrap().push("post");
            Ok(receipt())
        }
        async fn transfer(&self, _: &TransferCommand) -> Result<PostingReceipt, LedgerError> {
            self.0.lock().unwrap().push("transfer");
            Ok(receipt())
        }
        async fn reserve(&self, _: &ReserveCommand) -> Result<ReservationReceipt, LedgerError> {
            self.0.lock().unwrap().push("reserve");
            Ok(ReservationReceipt {
                reservation_id: Uuid::nil(),
                journal_id: Uuid::nil(),
                posted_at: String::new(),
                replayed: false,
            })
        }
        async fn record_execution(
            &self,
            _: Uuid,
            state: ExecutionState,
            _: &str,
        ) -> Result<ExecutionState, LedgerError> {
            self.0.lock().unwrap().push("execution");
            Ok(state)
        }
        async fn consume(&self, _: &ConsumeCommand) -> Result<PostingReceipt, LedgerError> {
            self.0.lock().unwrap().push("consume");
            Ok(receipt())
        }
        async fn release(&self, _: &ReleaseCommand) -> Result<PostingReceipt, LedgerError> {
            self.0.lock().unwrap().push("release");
            Ok(receipt())
        }
        async fn customer_balance(
            &self,
            _: Uuid,
            _: Uuid,
        ) -> Result<Option<CustomerBalance>, LedgerError> {
            Ok(None)
        }
    }

    struct Allow(AtomicBool);
    impl FinancialGate for Allow {
        async fn authorize(&self, _: Uuid, _: GatedOperation<'_>) -> Result<(), LedgerError> {
            self.0.store(true, Ordering::SeqCst);
            Ok(())
        }
    }

    fn run<F: Future>(future: F) -> F::Output {
        let mut future = std::pin::pin!(future);
        let mut context = std::task::Context::from_waker(std::task::Waker::noop());
        match future.as_mut().poll(&mut context) {
            std::task::Poll::Ready(output) => output,
            std::task::Poll::Pending => panic!("test future must complete in one poll"),
        }
    }

    fn transfer(key: &str, sender: u128, recipient: u128) -> TransferCommand {
        TransferCommand {
            operation_key: key.into(),
            sender: Uuid::from_u128(sender),
            recipient: Uuid::from_u128(recipient),
            asset_id: Uuid::from_u128(9),
            amount: PostingAmount::from_value(25).unwrap(),
            source_transaction: None,
            actor: Actor::Customer(Uuid::from_u128(sender)),
            correlation_id: None,
        }
    }

    fn reserve(event_type: &str) -> ReserveCommand {
        ReserveCommand {
            operation_key: "withdrawal:1".into(),
            customer: Uuid::from_u128(1),
            asset_id: Uuid::from_u128(9),
            amount: PostingAmount::from_value(25).unwrap(),
            transaction: None,
            event_type: event_type.into(),
            actor: Actor::Customer(Uuid::from_u128(1)),
            correlation_id: None,
        }
    }

    #[test]
    fn the_shipped_gate_refuses_every_customer_movement() {
        let service = LedgerService::new(Recorder::default(), NoApprovedPolicy);
        assert_eq!(
            run(service.transfer(&transfer("t", 1, 2))),
            Err(LedgerError::NotEligible)
        );
        assert_eq!(
            run(service.reserve(&reserve("WITHDRAWAL_RESERVED"))),
            Err(LedgerError::NotEligible)
        );
        assert!(
            service.store().0.lock().unwrap().is_empty(),
            "nothing reached the ledger"
        );
    }

    #[test]
    fn requests_are_validated_before_authorization_and_posting() {
        let service = LedgerService::new(Recorder::default(), Allow(AtomicBool::new(false)));
        assert_eq!(
            run(service.transfer(&transfer("t", 1, 1))),
            Err(LedgerError::InvalidRequest)
        );
        assert_eq!(
            run(service.transfer(&transfer("", 1, 2))),
            Err(LedgerError::InvalidRequest)
        );
        assert_eq!(
            run(service.reserve(&reserve("withdrawal"))),
            Err(LedgerError::InvalidRequest)
        );
        assert!(!service.gate.0.load(Ordering::SeqCst));
        assert_eq!(
            run(service.record_execution(Uuid::nil(), ExecutionState::Unknown, "")),
            Err(LedgerError::InvalidRequest)
        );
        let release = ReleaseCommand {
            reservation: Uuid::nil(),
            economic_key: "provider:op:1".into(),
            purpose: "release".into(),
            amount: PostingAmount::from_value(1).unwrap(),
            evidence: ReleaseEvidence::DefinitiveNonExecution,
            evidence_reference: "provider:status:failed".into(),
            actor: Actor::System,
            correlation_id: None,
        };
        assert_eq!(
            run(service.release(&release)),
            Err(LedgerError::InvalidRequest)
        );
        assert!(service.store().0.lock().unwrap().is_empty());

        assert!(run(service.transfer(&transfer("t", 1, 2))).is_ok());
        assert!(run(service.reserve(&reserve("WITHDRAWAL_RESERVED"))).is_ok());
        assert!(service.gate.0.load(Ordering::SeqCst));
        assert_eq!(
            *service.store().0.lock().unwrap(),
            vec!["transfer", "reserve"]
        );
    }
}
