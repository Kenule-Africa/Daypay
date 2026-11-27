//! Customer wallets and deposit-address provisioning (IMP-P6-004–009).
//!
//! DEYPAY owns wallet identities, statuses and balances; balances come only
//! from the Phase 5 ledger. Providers supply deposit addresses through the
//! canonical [`DepositAddressProvider`] contract, so no provider's formats
//! reach the wallet model and another provider can be added without ledger or
//! customer-contract changes. A wallet existing never permits spending: that
//! needs the ledger's activation and an approved [`FinancialGate`].
use crate::ledger::FinancialGate;
use deypay_domain::money::{Precision, format_decimal};
use deypay_domain::units::MinorUnits;
pub use deypay_domain::wallet::{ChainFamily, Restriction, WalletStatus};
use std::future::Future;
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WalletError {
    /// The wallet does not exist or belongs to someone else (indistinguishable).
    NotFound,
    /// The customer's account cannot hold wallets (for example not verified).
    NotEligible,
    InvalidRequest,
    /// The network is not supported for this wallet's asset.
    UnsupportedNetwork,
    /// The wallet or the account is restricted for this operation.
    Restricted,
    /// Deposit addresses require KYC approval.
    KycRequired,
    /// Deposits are not enabled for this asset and network.
    DepositsNotEnabled,
    /// No approved provider can serve the request.
    ProviderUnavailable,
    /// The request conflicts with the stored provisioning or address state.
    Conflict,
    /// The status change is not an allowed transition.
    InvalidTransition,
    /// Only an empty wallet can close.
    NotEmpty,
    /// Missing permission for an administrative change.
    ApprovalRequired,
    /// No operator is bound to the database login, or another one was stated.
    OperatorNotAuthenticated,
    /// The database role may not perform the operation.
    NotPermitted,
    /// The wallet database or ledger could not be reached. Never answered with
    /// substitute data.
    Unavailable,
    /// Stored data failed an integrity check.
    Corrupt,
}

/// Ledger balances of one wallet. Total is available plus reserved; pending
/// deposits are not part of it and cannot be spent.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WalletBalance {
    pub available: MinorUnits,
    pub reserved: MinorUnits,
    pub total: MinorUnits,
}

impl WalletBalance {
    pub fn new(available: MinorUnits, reserved: MinorUnits, total: MinorUnits) -> Option<Self> {
        (available.checked_add(reserved) == Some(total)).then_some(Self {
            available,
            reserved,
            total,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Wallet {
    pub id: Uuid,
    pub asset: String,
    pub asset_name: String,
    pub precision: Precision,
    pub status: WalletStatus,
    /// `None` only while the wallet's ledger accounts are being opened.
    pub balance: Option<WalletBalance>,
    pub restrictions: Vec<Restriction>,
    /// Networks on which this wallet can currently receive deposits.
    pub deposit_networks: u32,
    pub created_at: String,
    pub updated_at: String,
}

impl Wallet {
    /// The status the customer sees: an active wallet is reported restricted
    /// while its account or asset is restricted, suspended or on hold.
    pub fn effective_status(&self) -> WalletStatus {
        if self.status == WalletStatus::Active
            && self.restrictions.iter().any(|r| r.limits_active_wallet())
        {
            WalletStatus::Restricted
        } else {
            self.status
        }
    }

    /// Spending needs no remaining restriction in the database and an approved
    /// eligibility policy in the application.
    pub fn can_send(&self, policy_approved: bool) -> bool {
        policy_approved && self.restrictions.is_empty() && self.status == WalletStatus::Active
    }

    pub fn format(&self, units: MinorUnits) -> String {
        format_decimal(units, self.precision)
    }
}

/// Deposit-address state on one network, as the customer may see it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AddressState {
    /// A verified address the wallet may receive on.
    Active {
        address: String,
        memo: Option<String>,
    },
    /// A provisioning request is in progress.
    Pending,
    /// Nothing requested yet, and a request is currently possible.
    NotRequested,
    /// Deposits are not available on this network for this wallet now.
    Unavailable,
}

impl AddressState {
    pub const fn public_name(&self) -> &'static str {
        match self {
            Self::Active { .. } => "active",
            Self::Pending => "pending",
            Self::NotRequested => "not_requested",
            Self::Unavailable => "unavailable",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WalletNetwork {
    pub code: String,
    pub name: String,
    pub chain_family: ChainFamily,
    pub token_contract: String,
    pub token_decimals: u8,
    pub memo_required: bool,
    pub required_confirmations: Option<u32>,
    pub deposit_available: bool,
    pub withdrawal_available: bool,
    pub address: AddressState,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SupportedNetwork {
    pub code: String,
    pub name: String,
    pub chain_family: ChainFamily,
    pub asset: String,
    pub token_contract: String,
    pub token_decimals: u8,
    pub memo_required: bool,
    pub required_confirmations: Option<u32>,
    pub deposits: bool,
    pub withdrawals: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DepositAddress {
    Active {
        address: String,
        memo: Option<String>,
    },
    /// A durable provisioning request exists; `created` is true when this call
    /// opened it.
    Pending { created: bool },
}

/// Customer wallet persistence. Every method takes the authenticated customer;
/// no wallet is ever looked up without its owner.
pub trait WalletStore: Sync {
    /// Creates any missing wallet profiles and their zero-balance ledger
    /// accounts. Returns how many wallets it finished provisioning.
    fn initialize(&self, customer: Uuid) -> impl Future<Output = Result<u32, WalletError>> + Send;
    fn wallets(
        &self,
        customer: Uuid,
        wallet: Option<Uuid>,
    ) -> impl Future<Output = Result<Vec<Wallet>, WalletError>> + Send;
    fn wallet_networks(
        &self,
        customer: Uuid,
        wallet: Uuid,
    ) -> impl Future<Output = Result<Vec<WalletNetwork>, WalletError>> + Send;
    fn supported_networks(
        &self,
        asset: Option<&str>,
    ) -> impl Future<Output = Result<Vec<SupportedNetwork>, WalletError>> + Send;
    fn request_deposit_address(
        &self,
        customer: Uuid,
        wallet: Uuid,
        network: &str,
    ) -> impl Future<Output = Result<DepositAddress, WalletError>> + Send;
}

pub struct WalletService<S, G> {
    store: S,
    gate: G,
}

impl<S: WalletStore, G: FinancialGate> WalletService<S, G> {
    pub fn new(store: S, gate: G) -> Self {
        Self { store, gate }
    }

    pub fn store(&self) -> &S {
        &self.store
    }

    pub fn policy_approved(&self) -> bool {
        self.gate.policy_approved()
    }

    /// The customer's wallets, creating any that are missing first (FRS
    /// FR-WAL-001: wallets need no manual creation). An account that cannot
    /// hold wallets simply has none.
    pub async fn wallets(&self, customer: Uuid) -> Result<Vec<Wallet>, WalletError> {
        match self.store.initialize(customer).await {
            Ok(_) | Err(WalletError::NotEligible) => {}
            Err(problem) => return Err(problem),
        }
        self.store.wallets(customer, None).await
    }

    pub async fn wallet(&self, customer: Uuid, wallet: Uuid) -> Result<Wallet, WalletError> {
        self.store
            .wallets(customer, Some(wallet))
            .await?
            .into_iter()
            .next()
            .ok_or(WalletError::NotFound)
    }

    pub async fn networks(
        &self,
        customer: Uuid,
        wallet: Uuid,
    ) -> Result<Vec<WalletNetwork>, WalletError> {
        self.store.wallet_networks(customer, wallet).await
    }

    pub async fn supported_networks(
        &self,
        asset: Option<&str>,
    ) -> Result<Vec<SupportedNetwork>, WalletError> {
        if asset.is_some_and(|symbol| {
            !(2..=12).contains(&symbol.len())
                || !symbol
                    .bytes()
                    .all(|b| b.is_ascii_uppercase() || b.is_ascii_digit())
        }) {
            return Err(WalletError::UnsupportedNetwork);
        }
        self.store.supported_networks(asset).await
    }

    pub async fn deposit_address(
        &self,
        customer: Uuid,
        wallet: Uuid,
        network: &str,
    ) -> Result<AddressState, WalletError> {
        if !deypay_domain::wallet::valid_network_code(network) {
            return Err(WalletError::UnsupportedNetwork);
        }
        self.store
            .wallet_networks(customer, wallet)
            .await?
            .into_iter()
            .find(|candidate| candidate.code == network)
            .map(|found| found.address)
            .ok_or(WalletError::UnsupportedNetwork)
    }

    pub async fn request_deposit_address(
        &self,
        customer: Uuid,
        wallet: Uuid,
        network: &str,
    ) -> Result<DepositAddress, WalletError> {
        if !deypay_domain::wallet::valid_network_code(network) {
            return Err(WalletError::UnsupportedNetwork);
        }
        self.store
            .request_deposit_address(customer, wallet, network)
            .await
    }
}

// ---------------------------------------------------------------------------
// Provider-independent deposit-address provisioning (IMP-P6-008)
// ---------------------------------------------------------------------------

/// What DEYPAY asks a provider for. `reference` is DEYPAY's durable request
/// identity and the idempotency key; `wallet` is DEYPAY's wallet identifier,
/// never a customer identity.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AddressRequest {
    pub reference: String,
    pub wallet: Uuid,
    pub asset: String,
    pub network: String,
    pub chain_family: ChainFamily,
    pub token_contract: String,
    /// The provider's reference from an earlier accepted attempt, if any.
    pub external_reference: Option<String>,
}

/// A provider's answer, normalized. Adapters map every provider-specific
/// response and transport failure onto exactly one of these.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProviderResult {
    /// The provider assigned this address.
    Provisioned {
        external_reference: String,
        address: String,
        memo: Option<String>,
    },
    /// The provider holds the request and will assign later.
    Accepted { external_reference: String },
    /// The provider definitively refused; nothing was or will be assigned.
    Failed { code: String },
    /// The request may or may not have been received (for example a timeout).
    Unknown,
    /// The request was certainly not received (for example connection refused).
    NotReceived,
    /// The provider does not offer this asset/network.
    Unsupported,
}

/// Independent confirmation that an assigned address belongs to the wallet's
/// provider account. `sha256` is the digest of the authenticated provider data.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AddressConfirmation {
    Confirmed {
        evidence_reference: String,
        sha256: [u8; 32],
    },
    Mismatch {
        reason: String,
    },
    Unknown,
}

/// The canonical provider port. Implementations live in provider adapters and
/// are registered only for capabilities that are verified and approved.
pub trait DepositAddressProvider: Sync {
    /// The `integrations.providers` code this adapter serves.
    fn provider_code(&self) -> &str;
    fn request_address(
        &self,
        request: &AddressRequest,
    ) -> impl Future<Output = ProviderResult> + Send;
    /// Status of an earlier request with the same reference. Used after an
    /// unknown outcome instead of submitting again.
    fn lookup_address(
        &self,
        request: &AddressRequest,
    ) -> impl Future<Output = ProviderResult> + Send;
    fn confirm_address(
        &self,
        request: &AddressRequest,
        address: &str,
        memo: Option<&str>,
    ) -> impl Future<Output = AddressConfirmation> + Send;
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProvisioningJob {
    pub id: Uuid,
    pub provider_code: String,
    pub status: String,
    pub request: AddressRequest,
}

/// What the store allows for a job: submit once, or look up an earlier attempt.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProvisioningStep {
    Submit,
    Lookup,
    Done,
}

/// Provisioning persistence, used only by provider-integration workers.
pub trait ProvisioningStore: Sync {
    fn work(
        &self,
        limit: u32,
    ) -> impl Future<Output = Result<Vec<ProvisioningJob>, WalletError>> + Send;
    /// Marks a pending request submitted before the provider is called.
    fn begin(
        &self,
        request: Uuid,
    ) -> impl Future<Output = Result<ProvisioningStep, WalletError>> + Send;
    /// Records the result; returns the unverified address record when one was assigned.
    fn record(
        &self,
        request: Uuid,
        result: &ProviderResult,
    ) -> impl Future<Output = Result<Option<Uuid>, WalletError>> + Send;
    fn verify(
        &self,
        address: Uuid,
        evidence_reference: &str,
        sha256: [u8; 32],
    ) -> impl Future<Output = Result<(), WalletError>> + Send;
    fn reject(
        &self,
        address: Uuid,
        reason: &str,
    ) -> impl Future<Output = Result<(), WalletError>> + Send;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct ProvisioningReport {
    pub submitted: u32,
    pub looked_up: u32,
    pub activated: u32,
    pub rejected: u32,
    /// Jobs whose provider has no registered adapter in this process.
    pub skipped: u32,
}

/// Drives provisioning requests through registered provider adapters. A job is
/// submitted at most once; an attempt whose outcome is unknown is only ever
/// looked up with its original provider. Jobs for providers without an
/// adapter are left untouched.
pub struct AddressProvisioner<S, P> {
    store: S,
    providers: Vec<P>,
}

impl<S: ProvisioningStore, P: DepositAddressProvider> AddressProvisioner<S, P> {
    pub fn new(store: S, providers: Vec<P>) -> Self {
        Self { store, providers }
    }

    pub async fn run_once(&self, limit: u32) -> Result<ProvisioningReport, WalletError> {
        let mut report = ProvisioningReport::default();
        for job in self.store.work(limit).await? {
            let Some(provider) = self
                .providers
                .iter()
                .find(|candidate| candidate.provider_code() == job.provider_code)
            else {
                report.skipped += 1;
                continue;
            };
            self.process(&job, provider, &mut report).await?;
        }
        Ok(report)
    }

    async fn process(
        &self,
        job: &ProvisioningJob,
        provider: &P,
        report: &mut ProvisioningReport,
    ) -> Result<(), WalletError> {
        let result = match self.store.begin(job.id).await? {
            ProvisioningStep::Submit => {
                report.submitted += 1;
                provider.request_address(&job.request).await
            }
            ProvisioningStep::Lookup => {
                report.looked_up += 1;
                provider.lookup_address(&job.request).await
            }
            ProvisioningStep::Done => return Ok(()),
        };
        let recorded = self.store.record(job.id, &result).await?;
        let (Some(address_id), ProviderResult::Provisioned { address, memo, .. }) =
            (recorded, &result)
        else {
            return Ok(());
        };
        match provider
            .confirm_address(&job.request, address, memo.as_deref())
            .await
        {
            AddressConfirmation::Confirmed {
                evidence_reference,
                sha256,
            } => {
                self.store
                    .verify(address_id, &evidence_reference, sha256)
                    .await?;
                report.activated += 1;
            }
            AddressConfirmation::Mismatch { reason } => {
                self.store.reject(address_id, &reason).await?;
                report.rejected += 1;
            }
            // Stays unverified and invisible until confirmed.
            AddressConfirmation::Unknown => {}
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ledger::NoApprovedPolicy;
    use std::sync::Mutex;

    fn units(value: u128) -> MinorUnits {
        MinorUnits::new(value).unwrap()
    }

    fn wallet(status: WalletStatus, restrictions: Vec<Restriction>) -> Wallet {
        Wallet {
            id: Uuid::nil(),
            asset: "USDC".into(),
            asset_name: "USD Coin".into(),
            precision: Precision::new(6).unwrap(),
            status,
            balance: WalletBalance::new(units(1_500_000), units(250_000), units(1_750_000)),
            restrictions,
            deposit_networks: 0,
            created_at: String::new(),
            updated_at: String::new(),
        }
    }

    #[test]
    fn balances_must_add_up_and_format_exactly() {
        assert!(WalletBalance::new(units(1), units(2), units(4)).is_none());
        let shown = wallet(WalletStatus::Active, vec![]);
        let balance = shown.balance.unwrap();
        assert_eq!(shown.format(balance.available), "1.500000");
        assert_eq!(shown.format(balance.reserved), "0.250000");
        assert_eq!(shown.format(MinorUnits::ZERO), "0.000000");
        assert_eq!(
            shown.format(MinorUnits::MAX),
            "99999999999999999999999999999999.999999"
        );
    }

    #[test]
    fn spending_needs_a_clear_wallet_and_an_approved_policy() {
        assert!(wallet(WalletStatus::Active, vec![]).can_send(true));
        assert!(!wallet(WalletStatus::Active, vec![]).can_send(false));
        assert!(!wallet(WalletStatus::Active, vec![Restriction::KycRequired]).can_send(true));
        assert!(!wallet(WalletStatus::Suspended, vec![]).can_send(true));
        assert!(!NoApprovedPolicy.policy_approved());
    }

    #[test]
    fn outside_restrictions_make_an_active_wallet_restricted() {
        let held = wallet(WalletStatus::Active, vec![Restriction::AssetOnHold]);
        assert_eq!(held.effective_status(), WalletStatus::Restricted);
        let unverified = wallet(WalletStatus::Active, vec![Restriction::KycRequired]);
        assert_eq!(unverified.effective_status(), WalletStatus::Active);
        let suspended = wallet(
            WalletStatus::Suspended,
            vec![Restriction::AccountRestricted],
        );
        assert_eq!(suspended.effective_status(), WalletStatus::Suspended);
    }

    /// Records calls; the real provisioning state machine is exercised against
    /// PostgreSQL in deypay-infrastructure/tests/wallets.rs.
    #[derive(Default)]
    struct Recorder {
        calls: Mutex<Vec<String>>,
        step: Option<ProvisioningStep>,
    }

    impl ProvisioningStore for Recorder {
        async fn work(&self, _: u32) -> Result<Vec<ProvisioningJob>, WalletError> {
            Ok(vec![ProvisioningJob {
                id: Uuid::nil(),
                provider_code: "FIXTURE".into(),
                status: "PENDING".into(),
                request: AddressRequest {
                    reference: "deypay-addr-x".into(),
                    wallet: Uuid::nil(),
                    asset: "USDC".into(),
                    network: "base".into(),
                    chain_family: ChainFamily::Evm,
                    token_contract: "0x1111111111111111111111111111111111111111".into(),
                    external_reference: None,
                },
            }])
        }
        async fn begin(&self, _: Uuid) -> Result<ProvisioningStep, WalletError> {
            self.calls.lock().unwrap().push("begin".into());
            Ok(self.step.unwrap_or(ProvisioningStep::Submit))
        }
        async fn record(
            &self,
            _: Uuid,
            result: &ProviderResult,
        ) -> Result<Option<Uuid>, WalletError> {
            self.calls
                .lock()
                .unwrap()
                .push(format!("record {result:?}"));
            Ok(None)
        }
        async fn verify(&self, _: Uuid, _: &str, _: [u8; 32]) -> Result<(), WalletError> {
            Ok(())
        }
        async fn reject(&self, _: Uuid, _: &str) -> Result<(), WalletError> {
            Ok(())
        }
    }

    struct Fixture(&'static str, Mutex<Vec<&'static str>>);
    impl DepositAddressProvider for Fixture {
        fn provider_code(&self) -> &str {
            self.0
        }
        async fn request_address(&self, _: &AddressRequest) -> ProviderResult {
            self.1.lock().unwrap().push("request");
            ProviderResult::Unknown
        }
        async fn lookup_address(&self, _: &AddressRequest) -> ProviderResult {
            self.1.lock().unwrap().push("lookup");
            ProviderResult::Unknown
        }
        async fn confirm_address(
            &self,
            _: &AddressRequest,
            _: &str,
            _: Option<&str>,
        ) -> AddressConfirmation {
            AddressConfirmation::Unknown
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

    #[test]
    fn jobs_without_a_registered_adapter_are_left_untouched() {
        let provisioner = AddressProvisioner::new(
            Recorder::default(),
            vec![Fixture("OTHER", Mutex::default())],
        );
        let report = run(provisioner.run_once(10)).unwrap();
        assert_eq!(report.skipped, 1);
        assert!(
            provisioner.store.calls.lock().unwrap().is_empty(),
            "never begun"
        );
    }

    #[test]
    fn earlier_attempts_are_looked_up_never_resubmitted() {
        let store = Recorder {
            step: Some(ProvisioningStep::Lookup),
            ..Recorder::default()
        };
        let provisioner =
            AddressProvisioner::new(store, vec![Fixture("FIXTURE", Mutex::default())]);
        let report = run(provisioner.run_once(10)).unwrap();
        assert_eq!((report.submitted, report.looked_up), (0, 1));
        assert_eq!(*provisioner.providers[0].1.lock().unwrap(), vec!["lookup"]);
    }
}
