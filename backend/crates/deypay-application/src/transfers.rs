//! Internal transfers between DEYPAY customers (IMP-P7-001–010).
//!
//! FRS §16, API §§16, 20, 28 and 38, security SEC-TXN-001–007, ledger §14.
//! The service checks request formats and the workflow-level
//! [`FinancialGate`] (KYC policy, limits and step-up once approved), then hands
//! the exact terms to a [`TransferStore`]. The PostgreSQL store validates the
//! terms again, binds the customer's confirmation to them and to the session,
//! and posts the single balanced ledger journal atomically with the business
//! record, the consumed authorization and the idempotency record.
//!
//! Nothing here enables a transfer: the shipped gate (`NoApprovedPolicy`)
//! refuses every customer movement, and the ledger refuses transfers of an
//! asset until its `CUSTOMER_TRANSFER` activation is recorded.
use crate::ledger::{FinancialGate, GatedOperation, LedgerError};
use crate::usernames::handle;
use deypay_domain::money::{Precision, format_decimal, parse_decimal};
use deypay_domain::units::{MinorUnits, PostingAmount};
use sha2::{Digest, Sha256};
use std::future::Future;
use uuid::Uuid;

/// Longest transfer description, in characters (engineering default).
pub const MAX_DESCRIPTION_CHARS: usize = 140;
/// Largest history page.
pub const MAX_PAGE: u32 = 100;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TransferError {
    /// Malformed request: identifiers, description, key, cursor or page size.
    InvalidRequest,
    /// Not a positive amount at the asset's accounting precision.
    InvalidAmount,
    /// Not a customer transfer asset.
    UnsupportedAsset,
    /// No eligible customer has this identifier.
    RecipientNotFound,
    /// The recipient cannot receive this transfer now (reason withheld).
    RecipientUnavailable,
    /// Sender and recipient are the same customer (FR-INT-007).
    SelfTransfer,
    /// The sender needs KYC approval.
    KycRequired,
    /// The sender's account or wallet may not send.
    Restricted,
    /// The authorization proof or challenge is missing, expired, used,
    /// revoked, from another session or bound to other terms.
    AuthorizationInvalid,
    /// The idempotency key was used for different transfer terms.
    IdempotencyConflict,
    /// Internal transfers are not enabled (no approved policy or activation).
    NotEnabled,
    /// The asset is suspended or on hold.
    AssetUnavailable,
    InsufficientFunds,
    /// Above the approved per-operation ceiling or another approved limit.
    LimitExceeded,
    /// The customer's recipient-lookup budget is exhausted for now.
    LookupLimited,
    /// Recipient lookups are not enabled (no approved budget recorded).
    LookupUnavailable,
    /// The session presented is no longer active.
    SessionInvalid,
    /// Unknown transaction, or one the customer did not take part in.
    NotFound,
    /// A lock wait or serialization failure; safe to retry the same request.
    Busy,
    /// The database could not be reached; the outcome may be unknown, so a
    /// transfer must be retried with the same idempotency key.
    Unavailable,
    /// Stored data failed an integrity check.
    Corrupt,
}

/// A resolved recipient: only the permitted identity details (API §16.4).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Recipient {
    pub id: Uuid,
    pub username: String,
    pub display_name: Option<String>,
    pub can_receive: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TransferAsset {
    pub id: Uuid,
    pub symbol: String,
    pub precision: Precision,
}

/// The exact terms a customer confirms and then submits (SEC-TXN-002).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TransferTerms {
    pub recipient: Uuid,
    pub asset: String,
    pub amount: PostingAmount,
    pub description: Option<String>,
}

/// A confirmation challenge bound to the terms and the session.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuthorizationChallenge {
    pub id: Uuid,
    pub expires_at: String,
    pub recipient: Uuid,
    pub recipient_username: Option<String>,
    pub recipient_display_name: Option<String>,
    pub asset: String,
    pub precision: Precision,
    pub amount: MinorUnits,
    pub fee: MinorUnits,
    pub description: Option<String>,
    /// How the customer verifies: explicit confirmation until a step-up method
    /// is approved (DEC-002).
    pub verification_method: String,
}

impl AuthorizationChallenge {
    pub fn format(&self, units: MinorUnits) -> String {
        format_decimal(units, self.precision)
    }
}

/// A single-use authorization token and when it stops being accepted.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuthorizationProof {
    pub token: String,
    pub expires_at: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Direction {
    /// The viewer sent the transfer.
    Debit,
    /// The viewer received it.
    Credit,
}

impl Direction {
    pub fn parse(text: &str) -> Option<Self> {
        match text {
            "DEBIT" => Some(Self::Debit),
            "CREDIT" => Some(Self::Credit),
            _ => None,
        }
    }

    pub const fn public_name(self) -> &'static str {
        match self {
            Self::Debit => "debit",
            Self::Credit => "credit",
        }
    }
}

/// Public transaction status for an internal workflow state (API §38.3).
pub fn public_status(internal: &str) -> Option<&'static str> {
    Some(match internal {
        "CREATED" => "created",
        "VALIDATED" | "BENEFICIARY_VERIFIED" | "QUOTED" => "awaiting_authorization",
        "DETECTED" | "CONFIRMING" | "SCREENING" | "AWAITING_FUNDS" | "AUTHORIZED" | "RESERVED" => {
            "pending"
        }
        "SUBMITTED" | "PROCESSING" | "CONVERTING" | "PAYOUT_PROCESSING" | "REFUND_PROCESSING" => {
            "processing"
        }
        "CREDITED" | "COMPLETED" => "completed",
        "REJECTED" | "FAILED" => "failed",
        "CANCELLED" | "EXPIRED" => "cancelled",
        "UNDER_REVIEW" | "REORG_REVIEW" => "under_review",
        "PENDING_RECONCILIATION" => "pending_reconciliation",
        "REVERSED" => "reversed",
        "REFUNDED" => "refunded",
        _ => return None,
    })
}

/// The other participant, as the viewer may see them.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Counterparty {
    pub id: Uuid,
    pub username: Option<String>,
    pub display_name: Option<String>,
}

/// One internal transfer as one participant sees it: both share the
/// transaction identifier and reference (FR-INT-014).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TransferRecord {
    pub id: Uuid,
    pub reference: String,
    /// Internal workflow state; see [`public_status`].
    pub status: String,
    pub direction: Direction,
    pub asset: String,
    pub precision: Precision,
    pub amount: MinorUnits,
    /// Zero for the recipient, and zero for everyone until a fee is approved.
    pub fee: MinorUnits,
    pub description: Option<String>,
    pub counterparty: Counterparty,
    pub created_at: String,
    pub completed_at: Option<String>,
}

impl TransferRecord {
    pub fn format(&self, units: MinorUnits) -> String {
        format_decimal(units, self.precision)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Executed {
    /// The idempotency key was already used for these terms; nothing new happened.
    pub replayed: bool,
    pub transfer: TransferRecord,
}

/// Transfer persistence. Every call is for the authenticated customer;
/// `session_digest` is the SHA-256 hex digest of that customer's session
/// cookie, which the store resolves to the session the authorization is bound to.
pub trait TransferStore: Sync {
    /// Charges the customer's lookup budget, then resolves an exact handle.
    fn resolve(
        &self,
        customer: Uuid,
        handle: &str,
    ) -> impl Future<Output = Result<Option<Recipient>, TransferError>> + Send;
    fn assets(&self) -> impl Future<Output = Result<Vec<TransferAsset>, TransferError>> + Send;
    fn authorize(
        &self,
        customer: Uuid,
        session_digest: &str,
        terms: &TransferTerms,
    ) -> impl Future<Output = Result<AuthorizationChallenge, TransferError>> + Send;
    /// Completes the challenge and records the proof digest; returns its expiry.
    fn confirm(
        &self,
        customer: Uuid,
        session_digest: &str,
        challenge: Uuid,
        proof_digest: &str,
    ) -> impl Future<Output = Result<String, TransferError>> + Send;
    fn execute(
        &self,
        customer: Uuid,
        session_digest: &str,
        idempotency_key: &str,
        proof_digest: &str,
        terms: &TransferTerms,
    ) -> impl Future<Output = Result<Executed, TransferError>> + Send;
    fn history(
        &self,
        customer: Uuid,
        cursor: Option<Uuid>,
        limit: u32,
    ) -> impl Future<Output = Result<Vec<TransferRecord>, TransferError>> + Send;
    fn transaction(
        &self,
        customer: Uuid,
        id: Uuid,
    ) -> impl Future<Output = Result<Option<TransferRecord>, TransferError>> + Send;
}

/// A transfer as submitted: free-form text the service checks.
#[derive(Debug, Clone, Copy)]
pub struct TransferInput<'a> {
    pub recipient: Uuid,
    pub asset: &'a str,
    pub amount: &'a str,
    pub description: Option<&'a str>,
}

/// SHA-256 digest of a 64-hex-character token, or None if malformed.
pub fn token_digest(token: &str) -> Option<String> {
    (token.len() == 64
        && token
            .bytes()
            .all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f')))
    .then(|| hex::encode(Sha256::digest(token.as_bytes())))
}

/// Idempotency keys: 8–128 ASCII letters, digits, `_`, `.`, `:` or `-`.
pub fn valid_idempotency_key(key: &str) -> bool {
    (8..=128).contains(&key.len())
        && key
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'_' | b'.' | b':' | b'-'))
}

/// Trims a description; empty means none. Refuses control characters and
/// anything longer than [`MAX_DESCRIPTION_CHARS`].
pub fn normalize_description(text: Option<&str>) -> Result<Option<String>, TransferError> {
    let Some(text) = text.map(str::trim).filter(|t| !t.is_empty()) else {
        return Ok(None);
    };
    if text.chars().count() > MAX_DESCRIPTION_CHARS || text.chars().any(char::is_control) {
        return Err(TransferError::InvalidRequest);
    }
    Ok(Some(text.to_owned()))
}

pub struct TransferService<S, G> {
    store: S,
    gate: G,
}

impl<S: TransferStore, G: FinancialGate> TransferService<S, G> {
    pub const fn new(store: S, gate: G) -> Self {
        Self { store, gate }
    }

    pub fn policy_approved(&self) -> bool {
        self.gate.policy_approved()
    }

    /// FR-INT-002: an exact username (optional `@`) to the recipient's
    /// immutable identifier. Malformed handles never reach the store.
    pub async fn resolve(
        &self,
        customer: Uuid,
        username: &str,
    ) -> Result<Option<Recipient>, TransferError> {
        let handle = handle(username).ok_or(TransferError::InvalidRequest)?;
        self.store.resolve(customer, handle).await
    }

    /// FR-INT-006–010 step 1: validated terms and a confirmation challenge.
    pub async fn authorize(
        &self,
        customer: Uuid,
        session_digest: &str,
        input: TransferInput<'_>,
    ) -> Result<AuthorizationChallenge, TransferError> {
        let terms = self.permitted_terms(customer, input).await?;
        self.store.authorize(customer, session_digest, &terms).await
    }

    /// Step 2: the customer confirms; returns the single-use token, of which
    /// only the digest is stored. `token` must be freshly generated (64
    /// lowercase hex characters from a secure random source).
    pub async fn confirm(
        &self,
        customer: Uuid,
        session_digest: &str,
        challenge: Uuid,
        token: String,
    ) -> Result<AuthorizationProof, TransferError> {
        let digest = token_digest(&token).ok_or(TransferError::InvalidRequest)?;
        let expires_at = self
            .store
            .confirm(customer, session_digest, challenge, &digest)
            .await?;
        Ok(AuthorizationProof { token, expires_at })
    }

    /// Step 3: executes the confirmed transfer once (FR-INT-011/012).
    pub async fn execute(
        &self,
        customer: Uuid,
        session_digest: &str,
        idempotency_key: &str,
        token: &str,
        input: TransferInput<'_>,
    ) -> Result<Executed, TransferError> {
        if !valid_idempotency_key(idempotency_key) {
            return Err(TransferError::InvalidRequest);
        }
        let digest = token_digest(token).ok_or(TransferError::AuthorizationInvalid)?;
        let terms = self.permitted_terms(customer, input).await?;
        self.store
            .execute(customer, session_digest, idempotency_key, &digest, &terms)
            .await
    }

    pub async fn history(
        &self,
        customer: Uuid,
        cursor: Option<Uuid>,
        limit: u32,
    ) -> Result<Vec<TransferRecord>, TransferError> {
        if !(1..=MAX_PAGE).contains(&limit) {
            return Err(TransferError::InvalidRequest);
        }
        self.store.history(customer, cursor, limit).await
    }

    pub async fn transaction(
        &self,
        customer: Uuid,
        id: Uuid,
    ) -> Result<TransferRecord, TransferError> {
        self.store
            .transaction(customer, id)
            .await?
            .ok_or(TransferError::NotFound)
    }

    /// Exact terms at the asset's precision, then the financial gate.
    async fn permitted_terms(
        &self,
        customer: Uuid,
        input: TransferInput<'_>,
    ) -> Result<TransferTerms, TransferError> {
        let description = normalize_description(input.description)?;
        let asset = self
            .store
            .assets()
            .await?
            .into_iter()
            .find(|asset| asset.symbol == input.asset)
            .ok_or(TransferError::UnsupportedAsset)?;
        let amount = parse_decimal(input.amount, asset.precision)
            .ok()
            .and_then(PostingAmount::new)
            .ok_or(TransferError::InvalidAmount)?;
        if input.recipient == customer {
            return Err(TransferError::SelfTransfer);
        }
        let operation = GatedOperation::Transfer {
            recipient: input.recipient,
            asset_id: asset.id,
            amount,
        };
        if let Err(problem) = self.gate.authorize(customer, operation).await {
            return Err(if self.gate.policy_approved() {
                gate_error(problem)
            } else {
                TransferError::NotEnabled
            });
        }
        Ok(TransferTerms {
            recipient: input.recipient,
            asset: asset.symbol,
            amount,
            description,
        })
    }
}

/// Refusals from an approved gate.
fn gate_error(problem: LedgerError) -> TransferError {
    match problem {
        LedgerError::NotActivated => TransferError::NotEnabled,
        LedgerError::InsufficientFunds => TransferError::InsufficientFunds,
        LedgerError::AssetOnHold => TransferError::AssetUnavailable,
        LedgerError::ApprovalRequired => TransferError::AuthorizationInvalid,
        LedgerError::Unavailable | LedgerError::ConcurrentModification => {
            TransferError::Unavailable
        }
        _ => TransferError::Restricted,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    #[derive(Default)]
    struct Recorder {
        calls: Mutex<Vec<String>>,
    }

    impl Recorder {
        fn calls(&self) -> Vec<String> {
            self.calls.lock().unwrap().clone()
        }
    }

    fn record() -> TransferRecord {
        TransferRecord {
            id: Uuid::from_u128(77),
            reference: "DP-0000000000000001".into(),
            status: "COMPLETED".into(),
            direction: Direction::Debit,
            asset: "USDT".into(),
            precision: Precision::new(6).unwrap(),
            amount: MinorUnits::new(25_000_000).unwrap(),
            fee: MinorUnits::ZERO,
            description: None,
            counterparty: Counterparty {
                id: Uuid::from_u128(2),
                username: Some("bob".into()),
                display_name: None,
            },
            created_at: "2026-11-27T00:00:00.000000Z".into(),
            completed_at: Some("2026-11-27T00:00:00.000000Z".into()),
        }
    }

    impl TransferStore for &Recorder {
        async fn resolve(&self, _: Uuid, handle: &str) -> Result<Option<Recipient>, TransferError> {
            self.calls.lock().unwrap().push(format!("resolve {handle}"));
            Ok(None)
        }
        async fn assets(&self) -> Result<Vec<TransferAsset>, TransferError> {
            Ok(vec![TransferAsset {
                id: Uuid::from_u128(9),
                symbol: "USDT".into(),
                precision: Precision::new(6).unwrap(),
            }])
        }
        async fn authorize(
            &self,
            _: Uuid,
            _: &str,
            terms: &TransferTerms,
        ) -> Result<AuthorizationChallenge, TransferError> {
            self.calls
                .lock()
                .unwrap()
                .push(format!("authorize {}", terms.amount));
            Err(TransferError::Unavailable)
        }
        async fn confirm(
            &self,
            _: Uuid,
            _: &str,
            _: Uuid,
            digest: &str,
        ) -> Result<String, TransferError> {
            self.calls.lock().unwrap().push(format!("confirm {digest}"));
            Ok("later".into())
        }
        async fn execute(
            &self,
            _: Uuid,
            _: &str,
            key: &str,
            _: &str,
            terms: &TransferTerms,
        ) -> Result<Executed, TransferError> {
            self.calls
                .lock()
                .unwrap()
                .push(format!("execute {key} {}", terms.amount));
            Ok(Executed {
                replayed: false,
                transfer: record(),
            })
        }
        async fn history(
            &self,
            _: Uuid,
            _: Option<Uuid>,
            limit: u32,
        ) -> Result<Vec<TransferRecord>, TransferError> {
            self.calls.lock().unwrap().push(format!("history {limit}"));
            Ok(vec![])
        }
        async fn transaction(
            &self,
            _: Uuid,
            _: Uuid,
        ) -> Result<Option<TransferRecord>, TransferError> {
            Ok(None)
        }
    }

    /// Test-only gate standing in for a future approved policy.
    struct Approved(Result<(), LedgerError>);
    impl FinancialGate for Approved {
        async fn authorize(&self, _: Uuid, _: GatedOperation<'_>) -> Result<(), LedgerError> {
            self.0
        }
        fn policy_approved(&self) -> bool {
            true
        }
    }

    const ALICE: Uuid = Uuid::from_u128(1);
    const BOB: Uuid = Uuid::from_u128(2);
    const TOKEN: &str = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";

    fn input(amount: &str) -> TransferInput<'_> {
        TransferInput {
            recipient: BOB,
            asset: "USDT",
            amount,
            description: Some("  Lunch  "),
        }
    }

    /// Drives a future that never waits (the recorder answers immediately).
    fn run<F: Future>(future: F) -> F::Output {
        let mut future = std::pin::pin!(future);
        let mut context = std::task::Context::from_waker(std::task::Waker::noop());
        match future.as_mut().poll(&mut context) {
            std::task::Poll::Ready(output) => output,
            std::task::Poll::Pending => panic!("test future must complete in one poll"),
        }
    }

    fn send<G: FinancialGate>(gate: G, recorder: &Recorder) -> Result<Executed, TransferError> {
        run(TransferService::new(recorder, gate).execute(
            ALICE,
            "digest",
            "key-00000001",
            TOKEN,
            input("25.5"),
        ))
    }

    #[test]
    fn the_shipped_gate_refuses_before_the_store_is_asked() {
        run(async {
            let recorder = Recorder::default();
            assert_eq!(
                send(crate::ledger::NoApprovedPolicy, &recorder),
                Err(TransferError::NotEnabled)
            );
            let service = TransferService::new(&recorder, crate::ledger::NoApprovedPolicy);
            assert_eq!(
                service.authorize(ALICE, "digest", input("1")).await,
                Err(TransferError::NotEnabled)
            );
            assert!(recorder.calls().is_empty());
            assert!(!service.policy_approved());
        });
    }

    #[test]
    fn terms_are_exact_before_any_gate_or_store_call() {
        run(async {
            let recorder = Recorder::default();
            let service = TransferService::new(&recorder, Approved(Ok(())));
            for (amount, expected) in [
                ("0", TransferError::InvalidAmount),
                ("1.0000001", TransferError::InvalidAmount),
                ("-1", TransferError::InvalidAmount),
                ("1e3", TransferError::InvalidAmount),
                (" 1", TransferError::InvalidAmount),
            ] {
                assert_eq!(
                    service
                        .execute(ALICE, "d", "key-00000001", TOKEN, input(amount))
                        .await,
                    Err(expected),
                    "{amount}"
                );
            }
            let mut wrong_asset = input("1");
            wrong_asset.asset = "usdt";
            assert_eq!(
                service.authorize(ALICE, "d", wrong_asset).await,
                Err(TransferError::UnsupportedAsset)
            );
            let mut to_self = input("1");
            to_self.recipient = ALICE;
            assert_eq!(
                service.authorize(ALICE, "d", to_self).await,
                Err(TransferError::SelfTransfer)
            );
            let long = "x".repeat(MAX_DESCRIPTION_CHARS + 1);
            let mut described = input("1");
            described.description = Some(&long);
            assert_eq!(
                service.authorize(ALICE, "d", described).await,
                Err(TransferError::InvalidRequest)
            );
            for key in ["short", "has space-0001", &"k".repeat(129)] {
                assert_eq!(
                    service.execute(ALICE, "d", key, TOKEN, input("1")).await,
                    Err(TransferError::InvalidRequest),
                    "{key}"
                );
            }
            for token in ["", "ABCDEF", &TOKEN.to_uppercase()] {
                assert_eq!(
                    service
                        .execute(ALICE, "d", "key-00000001", token, input("1"))
                        .await,
                    Err(TransferError::AuthorizationInvalid)
                );
            }
            assert!(recorder.calls().is_empty(), "nothing reached the store");
        });
    }

    #[test]
    fn approved_terms_reach_the_store_in_accounting_units() {
        run(async {
            let recorder = Recorder::default();
            let executed = send(Approved(Ok(())), &recorder).unwrap();
            assert_eq!(
                executed.transfer.format(executed.transfer.amount),
                "25.000000"
            );
            assert_eq!(recorder.calls(), ["execute key-00000001 25500000"]);
            assert_eq!(
                normalize_description(Some("  Lunch  ")),
                Ok(Some("Lunch".into()))
            );
            assert_eq!(normalize_description(Some("   ")), Ok(None));
            assert_eq!(
                normalize_description(Some("a\u{7}b")),
                Err(TransferError::InvalidRequest)
            );
        });
    }

    #[test]
    fn approved_gate_refusals_keep_their_meaning() {
        run(async {
            for (problem, expected) in [
                (LedgerError::NotEligible, TransferError::Restricted),
                (LedgerError::NotActivated, TransferError::NotEnabled),
                (
                    LedgerError::InsufficientFunds,
                    TransferError::InsufficientFunds,
                ),
                (LedgerError::AssetOnHold, TransferError::AssetUnavailable),
                (
                    LedgerError::ApprovalRequired,
                    TransferError::AuthorizationInvalid,
                ),
            ] {
                let recorder = Recorder::default();
                assert_eq!(send(Approved(Err(problem)), &recorder), Err(expected));
                assert!(recorder.calls().is_empty());
            }
        });
    }

    #[test]
    fn only_the_token_digest_is_stored_and_handles_are_checked() {
        run(async {
            let recorder = Recorder::default();
            let service = TransferService::new(&recorder, Approved(Ok(())));
            let proof = service
                .confirm(ALICE, "d", Uuid::from_u128(5), TOKEN.into())
                .await
                .unwrap();
            assert_eq!(proof.token, TOKEN);
            let digest = token_digest(TOKEN).unwrap();
            assert_ne!(digest, TOKEN);
            assert_eq!(recorder.calls(), [format!("confirm {digest}")]);
            assert_eq!(
                service
                    .confirm(ALICE, "d", Uuid::from_u128(5), "bad".into())
                    .await,
                Err(TransferError::InvalidRequest)
            );
            assert_eq!(
                service.resolve(ALICE, "a..b").await,
                Err(TransferError::InvalidRequest)
            );
            assert_eq!(service.resolve(ALICE, "@Bob_1").await, Ok(None));
            assert_eq!(
                service.history(ALICE, None, 0).await,
                Err(TransferError::InvalidRequest)
            );
            assert_eq!(
                service.history(ALICE, None, MAX_PAGE + 1).await,
                Err(TransferError::InvalidRequest)
            );
            assert_eq!(
                service.transaction(ALICE, Uuid::from_u128(3)).await,
                Err(TransferError::NotFound)
            );
            assert_eq!(
                recorder.calls()[1..],
                ["resolve Bob_1".to_owned()],
                "the store receives the handle without @"
            );
        });
    }

    #[test]
    fn public_statuses_follow_the_api_mapping() {
        for (internal, public) in [
            ("CREATED", "created"),
            ("VALIDATED", "awaiting_authorization"),
            ("AUTHORIZED", "pending"),
            ("COMPLETED", "completed"),
            ("REJECTED", "failed"),
            ("FAILED", "failed"),
        ] {
            assert_eq!(public_status(internal), Some(public));
        }
        assert_eq!(public_status("completed"), None);
        assert_eq!(Direction::parse("CREDIT"), Some(Direction::Credit));
        assert_eq!(Direction::parse("credit"), None);
        assert!(valid_idempotency_key(
            "2c5f2e63-5ec1-4c1f-8a39-507ce66bde31"
        ));
    }
}
