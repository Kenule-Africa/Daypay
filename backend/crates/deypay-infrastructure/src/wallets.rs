//! PostgreSQL wallet adapters (IMP-P6-004–009).
//!
//! Each call runs one function from migration
//! `202611261401_wallet_asset_management.sql` in its own transaction:
//! - [`PgWallets`]: the customer API (`deypay_app`), always for the
//!   authenticated customer. Balances come from `ledger.customer_balance`.
//! - [`PgProvisioning`]: provider-integration workers
//!   (`deypay_wallet_provisioner`).
//! - [`PgWalletAdmin`]: operator status changes (`deypay_ledger_controller`)
//!   for the operator bound to the pool's login.
use deypay_application::wallets::*;
use deypay_domain::money::Precision;
use deypay_domain::units::MinorUnits;
use deypay_domain::wallet::{ChainFamily, Restriction, WalletStatus};
use sqlx::{PgPool, Row, postgres::PgRow};
use uuid::Uuid;

/// Attempts for deadlocks and serialization failures.
const MAX_ATTEMPTS: usize = 3;

enum Arg {
    Text(Option<String>),
    Id(Option<Uuid>),
    Int(i32),
    Bytes(Vec<u8>),
}

fn map_error(error: &sqlx::Error) -> WalletError {
    let sqlx::Error::Database(db) = error else {
        return WalletError::Unavailable;
    };
    match db.code().as_deref() {
        Some("WA001") => WalletError::NotEligible,
        Some("WA002" | "22P02" | "23514") => WalletError::InvalidRequest,
        Some("WA003") => WalletError::InvalidTransition,
        Some("WA004" | "LE008") => WalletError::NotFound,
        Some("WA005") => WalletError::UnsupportedNetwork,
        Some("WA006") => WalletError::Restricted,
        Some("WA007") => WalletError::DepositsNotEnabled,
        Some("WA008") => WalletError::ProviderUnavailable,
        Some("WA009" | "WA010" | "23505" | "55000") => WalletError::Conflict,
        Some("WA011") => WalletError::NotEmpty,
        Some("WA012") => WalletError::KycRequired,
        Some("LE013") => WalletError::ApprovalRequired,
        Some("LE016") => WalletError::OperatorNotAuthenticated,
        Some("42501") => WalletError::NotPermitted,
        _ => WalletError::Unavailable,
    }
}

fn retryable(error: &sqlx::Error) -> bool {
    matches!(error, sqlx::Error::Database(db)
        if matches!(db.code().as_deref(), Some("40P01" | "40001")))
}

fn corrupt(error: sqlx::Error) -> WalletError {
    tracing::error!(event = "wallet_decode_failure", error = %error, "Wallet result could not be decoded");
    WalletError::Corrupt
}

#[derive(Clone)]
struct Calls {
    pool: PgPool,
    role: Option<&'static str>,
}

impl Calls {
    async fn call(&self, sql: &str, args: Vec<Arg>) -> Result<Vec<PgRow>, WalletError> {
        let mut attempt = 0;
        loop {
            attempt += 1;
            match self.try_call(sql, &args).await {
                Ok(rows) => return Ok(rows),
                Err(error) if retryable(&error) && attempt < MAX_ATTEMPTS => continue,
                Err(error) => {
                    let mapped = map_error(&error);
                    if mapped == WalletError::Unavailable {
                        tracing::warn!(event = "wallet_database_failure", error = %error, "Wallet call failed");
                    }
                    return Err(mapped);
                }
            }
        }
    }

    async fn try_call(&self, sql: &str, args: &[Arg]) -> Result<Vec<PgRow>, sqlx::Error> {
        let mut tx = self.pool.begin().await?;
        if let Some(role) = self.role {
            sqlx::query(role).execute(&mut *tx).await?;
        }
        let mut query = sqlx::query(sql);
        for arg in args {
            query = match arg {
                Arg::Text(value) => query.bind(value.clone()),
                Arg::Id(value) => query.bind(*value),
                Arg::Int(value) => query.bind(*value),
                Arg::Bytes(value) => query.bind(value.clone()),
            };
        }
        let rows = query.fetch_all(&mut *tx).await?;
        tx.commit().await?;
        Ok(rows)
    }

    async fn one(&self, sql: &str, args: Vec<Arg>) -> Result<PgRow, WalletError> {
        self.call(sql, args)
            .await?
            .into_iter()
            .next()
            .ok_or(WalletError::Corrupt)
    }
}

fn units(row: &PgRow, column: &str) -> Result<Option<MinorUnits>, WalletError> {
    let text: Option<String> = row.try_get(column).map_err(corrupt)?;
    text.map(|value| value.parse().map_err(|_| WalletError::Corrupt))
        .transpose()
}

fn wallet(row: &PgRow) -> Result<Wallet, WalletError> {
    let status: String = row.try_get("status").map_err(corrupt)?;
    let precision: i16 = row.try_get("accounting_precision").map_err(corrupt)?;
    let restrictions: Vec<String> = row.try_get("restrictions").map_err(corrupt)?;
    let deposit_networks: i32 = row.try_get("deposit_networks").map_err(corrupt)?;
    let balance = match (
        units(row, "available_minor")?,
        units(row, "reserved_minor")?,
        units(row, "total_minor")?,
    ) {
        (Some(available), Some(reserved), Some(total)) => {
            Some(WalletBalance::new(available, reserved, total).ok_or(WalletError::Corrupt)?)
        }
        (None, None, None) => None,
        _ => return Err(WalletError::Corrupt),
    };
    let status = WalletStatus::parse(&status).ok_or(WalletError::Corrupt)?;
    // A provisioned wallet always has ledger accounts; never report a guess.
    if balance.is_none() && status != WalletStatus::Provisioning {
        return Err(WalletError::Corrupt);
    }
    Ok(Wallet {
        id: row.try_get("wallet_id").map_err(corrupt)?,
        asset: row.try_get("asset").map_err(corrupt)?,
        asset_name: row.try_get("asset_name").map_err(corrupt)?,
        precision: u8::try_from(precision)
            .ok()
            .and_then(Precision::new)
            .ok_or(WalletError::Corrupt)?,
        status,
        balance,
        restrictions: restrictions
            .iter()
            .map(|code| Restriction::parse(code).ok_or(WalletError::Corrupt))
            .collect::<Result<_, _>>()?,
        deposit_networks: u32::try_from(deposit_networks).map_err(|_| WalletError::Corrupt)?,
        created_at: row.try_get("created_at").map_err(corrupt)?,
        updated_at: row.try_get("updated_at").map_err(corrupt)?,
    })
}

fn family(row: &PgRow) -> Result<ChainFamily, WalletError> {
    let text: String = row.try_get("chain_family").map_err(corrupt)?;
    ChainFamily::parse(&text).ok_or(WalletError::Corrupt)
}

fn decimals(row: &PgRow) -> Result<u8, WalletError> {
    let value: i16 = row.try_get("token_decimals").map_err(corrupt)?;
    u8::try_from(value).map_err(|_| WalletError::Corrupt)
}

fn confirmations(row: &PgRow) -> Result<Option<u32>, WalletError> {
    let value: Option<i32> = row.try_get("required_confirmations").map_err(corrupt)?;
    value
        .map(|count| u32::try_from(count).map_err(|_| WalletError::Corrupt))
        .transpose()
}

/// The customer API's wallet store. Runs as the pool's login (`deypay_app`).
#[derive(Clone)]
pub struct PgWallets(Calls);

impl PgWallets {
    pub fn new(pool: PgPool) -> Self {
        Self(Calls { pool, role: None })
    }
}

impl WalletStore for PgWallets {
    async fn initialize(&self, customer: Uuid) -> Result<u32, WalletError> {
        let row = self
            .0
            .one(
                "SELECT wallets.initialize_customer_wallets($1) AS provisioned",
                vec![Arg::Id(Some(customer))],
            )
            .await?;
        let provisioned: i32 = row.try_get("provisioned").map_err(corrupt)?;
        u32::try_from(provisioned).map_err(|_| WalletError::Corrupt)
    }

    async fn wallets(
        &self,
        customer: Uuid,
        wallet_id: Option<Uuid>,
    ) -> Result<Vec<Wallet>, WalletError> {
        self.0
            .call(
                "SELECT wallet_id, asset, asset_name, accounting_precision, status, provisioning_status, created_at,
                    updated_at, available_minor, reserved_minor, total_minor, restrictions, deposit_networks
                 FROM wallets.customer_wallets($1, $2)",
                vec![Arg::Id(Some(customer)), Arg::Id(wallet_id)],
            )
            .await?
            .iter()
            .map(wallet)
            .collect()
    }

    async fn wallet_networks(
        &self,
        customer: Uuid,
        wallet_id: Uuid,
    ) -> Result<Vec<WalletNetwork>, WalletError> {
        self.0
            .call(
                "SELECT network_code, network_name, chain_family, token_contract, token_decimals, memo_required,
                    required_confirmations, deposit_available, withdrawal_available, address_status, address, memo
                 FROM wallets.customer_wallet_networks($1, $2)",
                vec![Arg::Id(Some(customer)), Arg::Id(Some(wallet_id))],
            )
            .await?
            .iter()
            .map(|row| {
                let deposit_available: bool = row.try_get("deposit_available").map_err(corrupt)?;
                let state: String = row.try_get("address_status").map_err(corrupt)?;
                let address: Option<String> = row.try_get("address").map_err(corrupt)?;
                let address = match (state.as_str(), address) {
                    ("ACTIVE", Some(address)) => AddressState::Active {
                        address,
                        memo: row.try_get("memo").map_err(corrupt)?,
                    },
                    ("PENDING", None) => AddressState::Pending,
                    ("NONE", None) if deposit_available => AddressState::NotRequested,
                    ("NONE" | "HIDDEN", None) => AddressState::Unavailable,
                    _ => return Err(WalletError::Corrupt),
                };
                Ok(WalletNetwork {
                    code: row.try_get("network_code").map_err(corrupt)?,
                    name: row.try_get("network_name").map_err(corrupt)?,
                    chain_family: family(row)?,
                    token_contract: row.try_get("token_contract").map_err(corrupt)?,
                    token_decimals: decimals(row)?,
                    memo_required: row.try_get("memo_required").map_err(corrupt)?,
                    required_confirmations: confirmations(row)?,
                    deposit_available,
                    withdrawal_available: row.try_get("withdrawal_available").map_err(corrupt)?,
                    address,
                })
            })
            .collect()
    }

    async fn supported_networks(
        &self,
        asset: Option<&str>,
    ) -> Result<Vec<SupportedNetwork>, WalletError> {
        self.0
            .call(
                "SELECT network_code, network_name, chain_family, asset, token_contract, token_decimals,
                    memo_required, required_confirmations, deposits, withdrawals
                 FROM wallets.supported_networks($1)",
                vec![Arg::Text(asset.map(str::to_owned))],
            )
            .await?
            .iter()
            .map(|row| {
                Ok(SupportedNetwork {
                    code: row.try_get("network_code").map_err(corrupt)?,
                    name: row.try_get("network_name").map_err(corrupt)?,
                    chain_family: family(row)?,
                    asset: row.try_get("asset").map_err(corrupt)?,
                    token_contract: row.try_get("token_contract").map_err(corrupt)?,
                    token_decimals: decimals(row)?,
                    memo_required: row.try_get("memo_required").map_err(corrupt)?,
                    required_confirmations: confirmations(row)?,
                    deposits: row.try_get("deposits").map_err(corrupt)?,
                    withdrawals: row.try_get("withdrawals").map_err(corrupt)?,
                })
            })
            .collect()
    }

    async fn request_deposit_address(
        &self,
        customer: Uuid,
        wallet_id: Uuid,
        network: &str,
    ) -> Result<DepositAddress, WalletError> {
        let row = self
            .0
            .one(
                "SELECT status, address, memo, created FROM wallets.request_deposit_address($1, $2, $3)",
                vec![
                    Arg::Id(Some(customer)),
                    Arg::Id(Some(wallet_id)),
                    Arg::Text(Some(network.to_owned())),
                ],
            )
            .await?;
        let status: String = row.try_get("status").map_err(corrupt)?;
        let address: Option<String> = row.try_get("address").map_err(corrupt)?;
        match (status.as_str(), address) {
            ("ACTIVE", Some(address)) => Ok(DepositAddress::Active {
                address,
                memo: row.try_get("memo").map_err(corrupt)?,
            }),
            ("PENDING", None) => Ok(DepositAddress::Pending {
                created: row.try_get("created").map_err(corrupt)?,
            }),
            _ => Err(WalletError::Corrupt),
        }
    }
}

/// Provisioning persistence for provider-integration workers.
#[derive(Clone)]
pub struct PgProvisioning(Calls);

impl PgProvisioning {
    /// For a login granted `deypay_wallet_provisioner`.
    pub fn new(pool: PgPool) -> Self {
        Self(Calls {
            pool,
            role: Some("SET LOCAL ROLE deypay_wallet_provisioner"),
        })
    }
}

impl ProvisioningStore for PgProvisioning {
    async fn work(&self, limit: u32) -> Result<Vec<ProvisioningJob>, WalletError> {
        self.0
            .call(
                "SELECT request_id, request_reference, status, provider_code, wallet_id, asset, network_code,
                    chain_family, token_contract, external_reference FROM wallets.provisioning_work($1)",
                vec![Arg::Int(i32::try_from(limit.min(100)).unwrap_or(100))],
            )
            .await?
            .iter()
            .map(|row| {
                Ok(ProvisioningJob {
                    id: row.try_get("request_id").map_err(corrupt)?,
                    provider_code: row.try_get("provider_code").map_err(corrupt)?,
                    status: row.try_get("status").map_err(corrupt)?,
                    request: AddressRequest {
                        reference: row.try_get("request_reference").map_err(corrupt)?,
                        wallet: row.try_get("wallet_id").map_err(corrupt)?,
                        asset: row.try_get("asset").map_err(corrupt)?,
                        network: row.try_get("network_code").map_err(corrupt)?,
                        chain_family: family(row)?,
                        token_contract: row.try_get("token_contract").map_err(corrupt)?,
                        external_reference: row.try_get("external_reference").map_err(corrupt)?,
                    },
                })
            })
            .collect()
    }

    async fn begin(&self, request: Uuid) -> Result<ProvisioningStep, WalletError> {
        let row = self
            .0
            .one(
                "SELECT wallets.begin_provisioning($1) AS step",
                vec![Arg::Id(Some(request))],
            )
            .await?;
        let step: String = row.try_get("step").map_err(corrupt)?;
        match step.as_str() {
            "SUBMIT" => Ok(ProvisioningStep::Submit),
            "LOOKUP" => Ok(ProvisioningStep::Lookup),
            "DONE" => Ok(ProvisioningStep::Done),
            _ => Err(WalletError::Corrupt),
        }
    }

    async fn record(
        &self,
        request: Uuid,
        result: &ProviderResult,
    ) -> Result<Option<Uuid>, WalletError> {
        let (kind, external, address, memo, code) = match result {
            ProviderResult::Provisioned {
                external_reference,
                address,
                memo,
            } => (
                "PROVISIONED",
                Some(external_reference.clone()),
                Some(address.clone()),
                memo.clone(),
                None,
            ),
            ProviderResult::Accepted { external_reference } => (
                "ACCEPTED",
                Some(external_reference.clone()),
                None,
                None,
                None,
            ),
            ProviderResult::Failed { code } => ("FAILED", None, None, None, Some(code.clone())),
            ProviderResult::Unknown => ("UNKNOWN", None, None, None, None),
            ProviderResult::NotReceived => ("NOT_RECEIVED", None, None, None, None),
            ProviderResult::Unsupported => ("UNSUPPORTED", None, None, None, None),
        };
        let row = self
            .0
            .one(
                "SELECT wallets.record_provisioning_result($1, $2, $3, $4, $5, $6) AS address_id",
                vec![
                    Arg::Id(Some(request)),
                    Arg::Text(Some(kind.to_owned())),
                    Arg::Text(external),
                    Arg::Text(address),
                    Arg::Text(memo),
                    Arg::Text(code),
                ],
            )
            .await?;
        row.try_get("address_id").map_err(corrupt)
    }

    async fn verify(
        &self,
        address: Uuid,
        evidence_reference: &str,
        sha256: [u8; 32],
    ) -> Result<(), WalletError> {
        self.0
            .one(
                "SELECT wallets.verify_deposit_address($1, $2, $3) AS status",
                vec![
                    Arg::Id(Some(address)),
                    Arg::Text(Some(evidence_reference.to_owned())),
                    Arg::Bytes(sha256.to_vec()),
                ],
            )
            .await
            .map(|_| ())
    }

    async fn reject(&self, address: Uuid, reason: &str) -> Result<(), WalletError> {
        self.0
            .one(
                "SELECT wallets.reject_deposit_address($1, $2) AS status",
                vec![Arg::Id(Some(address)), Arg::Text(Some(reason.to_owned()))],
            )
            .await
            .map(|_| ())
    }
}

impl PgProvisioning {
    /// Retires an address. The record stays so late deposits remain attributable.
    pub async fn retire(&self, address: Uuid, reason: &str) -> Result<(), WalletError> {
        self.0
            .one(
                "SELECT wallets.retire_deposit_address($1, $2) AS status",
                vec![Arg::Id(Some(address)), Arg::Text(Some(reason.to_owned()))],
            )
            .await
            .map(|_| ())
    }
}

/// Administrative wallet status changes (IMP-P6-005). The actor is a claim
/// that PostgreSQL accepts only for the operator bound to the pool's login.
#[derive(Clone)]
pub struct PgWalletAdmin(Calls);

impl PgWalletAdmin {
    /// For an operator's login granted `deypay_ledger_controller`.
    pub fn new(pool: PgPool) -> Self {
        Self(Calls {
            pool,
            role: Some("SET LOCAL ROLE deypay_ledger_controller"),
        })
    }

    pub async fn set_status(
        &self,
        wallet: Uuid,
        status: WalletStatus,
        reason: &str,
        actor: Uuid,
    ) -> Result<WalletStatus, WalletError> {
        let row = self
            .0
            .one(
                "SELECT wallets.set_wallet_status($1, $2, $3, $4) AS status",
                vec![
                    Arg::Id(Some(wallet)),
                    Arg::Text(Some(status.as_db().to_owned())),
                    Arg::Text(Some(reason.to_owned())),
                    Arg::Id(Some(actor)),
                ],
            )
            .await?;
        let status: String = row.try_get("status").map_err(corrupt)?;
        WalletStatus::parse(&status).ok_or(WalletError::Corrupt)
    }
}
