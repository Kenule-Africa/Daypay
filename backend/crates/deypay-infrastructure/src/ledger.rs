//! PostgreSQL ledger adapter (IMP-P5-007–016).
//!
//! Every operation calls one SECURITY DEFINER function from migration
//! `202611260001_financial_ledger.sql` in its own transaction. The functions
//! own all validation, locking, idempotency and posting; this adapter only
//! binds typed arguments, maps the ledger's SQLSTATE codes and retries
//! transient conflicts. Retrying is safe because every call carries an
//! accounting event identity: a retry of a committed call replays its result.
use deypay_application::ledger::*;
use deypay_domain::units::MinorUnits;
use sqlx::{PgPool, Row, postgres::PgRow};
use uuid::Uuid;

/// Attempts for deadlocks, serialization failures and unique-key races.
const MAX_ATTEMPTS: usize = 3;
const POSTED_AT: &str = r#"to_char(posted_at AT TIME ZONE 'UTC', 'YYYY-MM-DD"T"HH24:MI:SS.US"Z"')"#;

/// The ledger role a connection acts as. Production logins are granted one
/// role; `acting_as` additionally narrows a login that holds several. Roles
/// never change who the session is: controller functions act for the operator
/// bound to the pool's authenticated login (`ledger.operator_logins`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LedgerRole {
    Poster,
    /// Authenticated provider adapters recording definitive outcomes.
    Evidence,
    Controller,
    Reader,
}

impl LedgerRole {
    const fn set_role(self) -> &'static str {
        match self {
            Self::Poster => "SET LOCAL ROLE deypay_ledger_poster",
            Self::Evidence => "SET LOCAL ROLE deypay_ledger_evidence",
            Self::Controller => "SET LOCAL ROLE deypay_ledger_controller",
            Self::Reader => "SET LOCAL ROLE deypay_ledger_reader",
        }
    }
}

/// The definitive external outcome of a reserved operation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProviderOutcome {
    Executed,
    NotExecuted,
}

impl ProviderOutcome {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Executed => "EXECUTED",
            Self::NotExecuted => "NOT_EXECUTED",
        }
    }
}

/// Provider evidence an adapter has already authenticated (provider signature
/// or status query) and matched to the operation it submitted. The ledger
/// keeps the digest of the verified message, not the message itself.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProviderEvidence {
    pub reservation: Uuid,
    pub outcome: ProviderOutcome,
    pub provider_operation: Uuid,
    pub provider_reference: String,
    pub sha256: [u8; 32],
    /// RFC 3339 time the provider reported the outcome.
    pub observed_at: String,
}

#[derive(Clone)]
pub struct PgLedger {
    pool: PgPool,
    role: Option<LedgerRole>,
}

enum Arg {
    Text(Option<String>),
    Id(Option<Uuid>),
    Ids(Vec<Uuid>),
    Texts(Vec<String>),
    Flag(bool),
    Bytes(Vec<u8>),
}

fn map_error(error: &sqlx::Error) -> LedgerError {
    let sqlx::Error::Database(db) = error else {
        return LedgerError::Unavailable;
    };
    match db.code().as_deref() {
        Some("LE001") => LedgerError::UnbalancedJournal,
        Some("LE002") => LedgerError::InsufficientFunds,
        Some("LE003") => LedgerError::AccountRestricted,
        Some("LE004") => LedgerError::DuplicateAccountingEvent,
        Some("LE005") => LedgerError::InvalidReversal,
        Some("LE006") => LedgerError::InvalidRequest,
        Some("LE007") => LedgerError::ReservationState,
        Some("LE008") => LedgerError::NotFound,
        Some("LE009") => LedgerError::NotEligible,
        Some("LE010") => LedgerError::AssetOnHold,
        Some("LE011") => LedgerError::AssetMismatch,
        Some("LE012") => LedgerError::NotPermitted,
        Some("LE013") => LedgerError::ApprovalRequired,
        Some("LE014") => LedgerError::NotActivated,
        Some("LE015") => LedgerError::EvidenceRequired,
        Some("LE016") => LedgerError::OperatorNotAuthenticated,
        // Deadlock, serialization failure, lock timeout or an exhausted unique race.
        Some("40P01" | "40001" | "55P03" | "23505") => LedgerError::ConcurrentModification,
        // Invalid text representation (for example a malformed identifier) or a
        // check constraint the functions should already have prevented.
        Some("22P02" | "23514" | "23503" | "22003") => LedgerError::InvalidRequest,
        Some("42501") => LedgerError::NotPermitted,
        _ => LedgerError::Unavailable,
    }
}

fn retryable(error: &sqlx::Error) -> bool {
    matches!(error, sqlx::Error::Database(db)
        if matches!(db.code().as_deref(), Some("40P01" | "40001" | "23505")))
}

fn decode(error: sqlx::Error) -> LedgerError {
    tracing::error!(event = "ledger_decode_failure", error = %error, "Ledger result could not be decoded");
    LedgerError::Unavailable
}

fn units(row: &PgRow, column: &str) -> Result<MinorUnits, LedgerError> {
    let text: String = row.try_get(column).map_err(decode)?;
    text.parse().map_err(|_| LedgerError::Unavailable)
}

impl PgLedger {
    pub fn new(pool: PgPool) -> Self {
        Self { pool, role: None }
    }

    pub fn acting_as(pool: PgPool, role: LedgerRole) -> Self {
        Self {
            pool,
            role: Some(role),
        }
    }

    async fn call(&self, sql: &str, args: Vec<Arg>) -> Result<Vec<PgRow>, LedgerError> {
        let mut attempt = 0;
        loop {
            attempt += 1;
            match self.try_call(sql, &args).await {
                Ok(rows) => return Ok(rows),
                Err(error) if retryable(&error) && attempt < MAX_ATTEMPTS => continue,
                Err(error) => {
                    let mapped = map_error(&error);
                    if mapped == LedgerError::Unavailable {
                        tracing::warn!(event = "ledger_database_failure", error = %error, "Ledger call failed");
                    }
                    return Err(mapped);
                }
            }
        }
    }

    async fn try_call(&self, sql: &str, args: &[Arg]) -> Result<Vec<PgRow>, sqlx::Error> {
        let mut tx = self.pool.begin().await?;
        if let Some(role) = self.role {
            sqlx::query(role.set_role()).execute(&mut *tx).await?;
        }
        let mut query = sqlx::query(sql);
        for arg in args {
            query = match arg {
                Arg::Text(value) => query.bind(value.clone()),
                Arg::Id(value) => query.bind(*value),
                Arg::Ids(values) => query.bind(values.clone()),
                Arg::Texts(values) => query.bind(values.clone()),
                Arg::Flag(value) => query.bind(*value),
                Arg::Bytes(value) => query.bind(value.clone()),
            };
        }
        let rows = query.fetch_all(&mut *tx).await?;
        // Deferred balance checks run here; a failure rolls everything back.
        tx.commit().await?;
        Ok(rows)
    }

    async fn one(&self, sql: &str, args: Vec<Arg>) -> Result<PgRow, LedgerError> {
        self.call(sql, args)
            .await?
            .into_iter()
            .next()
            .ok_or(LedgerError::Unavailable)
    }

    fn receipt(row: &PgRow) -> Result<PostingReceipt, LedgerError> {
        Ok(PostingReceipt {
            journal_id: row.try_get("journal_id").map_err(decode)?,
            accounting_event_id: row.try_get("accounting_event_id").map_err(decode)?,
            posted_at: row.try_get("posted_at").map_err(decode)?,
            replayed: row.try_get("replayed").map_err(decode)?,
        })
    }

    fn actor(actor: Actor) -> [Arg; 2] {
        [
            Arg::Text(Some(actor.kind().to_owned())),
            Arg::Id(actor.id()),
        ]
    }

    /// Signed normal-side balance of any account (clearing accounts may be negative).
    pub async fn account_balance(
        &self,
        account: Uuid,
    ) -> Result<Option<AccountBalance>, LedgerError> {
        let rows = self
            .call(
                "SELECT asset_id, normal_balance, balance_minor, version, status FROM ledger.account_balance($1)",
                vec![Arg::Id(Some(account))],
            )
            .await?;
        rows.first()
            .map(|row| {
                let text: String = row.try_get("balance_minor").map_err(decode)?;
                Ok(AccountBalance {
                    asset_id: row.try_get("asset_id").map_err(decode)?,
                    balance_minor: text.parse().map_err(|_| LedgerError::Unavailable)?,
                    version: row.try_get("version").map_err(decode)?,
                    status: row.try_get("status").map_err(decode)?,
                })
            })
            .transpose()
    }

    pub async fn reservation(
        &self,
        reservation: Uuid,
    ) -> Result<Option<ReservationView>, LedgerError> {
        let rows = self
            .call(
                "SELECT operation_key, user_id, asset_id, amount_minor, consumed_minor, released_minor,
                    remaining_minor, status, execution_state FROM ledger.reservation_state($1)",
                vec![Arg::Id(Some(reservation))],
            )
            .await?;
        rows.first()
            .map(|row| {
                let state: String = row.try_get("execution_state").map_err(decode)?;
                Ok(ReservationView {
                    operation_key: row.try_get("operation_key").map_err(decode)?,
                    customer: row.try_get("user_id").map_err(decode)?,
                    asset_id: row.try_get("asset_id").map_err(decode)?,
                    amount: units(row, "amount_minor")?,
                    consumed: units(row, "consumed_minor")?,
                    released: units(row, "released_minor")?,
                    remaining: units(row, "remaining_minor")?,
                    status: row.try_get("status").map_err(decode)?,
                    execution: ExecutionState::parse(&state).ok_or(LedgerError::Unavailable)?,
                })
            })
            .transpose()
    }

    /// Evidence boundary for provider adapters (`LedgerRole::Evidence`).
    /// Returns the outcome record and whether an identical outcome had already
    /// been recorded. Recording moves no funds; the reservation's workflow then
    /// settles or releases against it.
    pub async fn record_provider_outcome(
        &self,
        evidence: &ProviderEvidence,
    ) -> Result<(Uuid, bool), LedgerError> {
        let row = self
            .one(
                "SELECT outcome_id, replayed FROM ledger.record_provider_outcome($1,$2,$3,$4,$5,$6::timestamptz)",
                vec![
                    Arg::Id(Some(evidence.reservation)),
                    Arg::Text(Some(evidence.outcome.as_str().to_owned())),
                    Arg::Id(Some(evidence.provider_operation)),
                    Arg::Text(Some(evidence.provider_reference.clone())),
                    Arg::Bytes(evidence.sha256.to_vec()),
                    Arg::Text(Some(evidence.observed_at.clone())),
                ],
            )
            .await?;
        Ok((
            row.try_get("outcome_id").map_err(decode)?,
            row.try_get("replayed").map_err(decode)?,
        ))
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AccountBalance {
    pub asset_id: Uuid,
    pub balance_minor: i128,
    pub version: i64,
    pub status: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReservationView {
    pub operation_key: String,
    pub customer: Uuid,
    pub asset_id: Uuid,
    pub amount: MinorUnits,
    pub consumed: MinorUnits,
    pub released: MinorUnits,
    pub remaining: MinorUnits,
    pub status: String,
    pub execution: ExecutionState,
}

impl LedgerStore for PgLedger {
    async fn open_customer_accounts(
        &self,
        wallet_profile: Uuid,
    ) -> Result<CustomerAccounts, LedgerError> {
        let row = self
            .one(
                "SELECT available_account_id, reserved_account_id FROM ledger.open_customer_accounts($1)",
                vec![Arg::Id(Some(wallet_profile))],
            )
            .await?;
        Ok(CustomerAccounts {
            available: row.try_get("available_account_id").map_err(decode)?,
            reserved: row.try_get("reserved_account_id").map_err(decode)?,
        })
    }

    async fn post(&self, command: &PostingCommand) -> Result<PostingReceipt, LedgerError> {
        let lines = command.journal.lines();
        let mut args = vec![
            Arg::Text(Some(command.identity.event_type().to_owned())),
            Arg::Text(Some(command.identity.economic_key().to_owned())),
            Arg::Text(Some(command.identity.purpose().to_owned())),
            Arg::Id(command.source_transaction),
            Arg::Ids(lines.iter().map(|line| line.account_id).collect()),
            Arg::Ids(lines.iter().map(|line| line.asset_id).collect()),
            Arg::Texts(
                lines
                    .iter()
                    .map(|line| line.direction.as_str().to_owned())
                    .collect(),
            ),
            Arg::Texts(lines.iter().map(|line| line.amount.to_string()).collect()),
            Arg::Text(command.description.clone()),
        ];
        args.extend(Self::actor(command.actor));
        args.push(Arg::Id(command.correlation_id));
        let sql = format!(
            "SELECT journal_id, accounting_event_id, {POSTED_AT} AS posted_at, replayed
             FROM ledger.post_journal($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12)"
        );
        Self::receipt(&self.one(&sql, args).await?)
    }

    async fn transfer(&self, command: &TransferCommand) -> Result<PostingReceipt, LedgerError> {
        let mut args = vec![
            Arg::Text(Some(command.operation_key.clone())),
            Arg::Id(Some(command.sender)),
            Arg::Id(Some(command.recipient)),
            Arg::Id(Some(command.asset_id)),
            Arg::Text(Some(command.amount.to_string())),
            Arg::Id(command.source_transaction),
        ];
        args.extend(Self::actor(command.actor));
        args.push(Arg::Id(command.correlation_id));
        let sql = format!(
            "SELECT journal_id, accounting_event_id, {POSTED_AT} AS posted_at, replayed
             FROM ledger.transfer_between_customers($1,$2,$3,$4,$5,$6,$7,$8,$9)"
        );
        Self::receipt(&self.one(&sql, args).await?)
    }

    async fn reserve(&self, command: &ReserveCommand) -> Result<ReservationReceipt, LedgerError> {
        let mut args = vec![
            Arg::Text(Some(command.operation_key.clone())),
            Arg::Id(Some(command.customer)),
            Arg::Id(Some(command.asset_id)),
            Arg::Text(Some(command.amount.to_string())),
            Arg::Id(command.transaction),
            Arg::Text(Some(command.event_type.clone())),
        ];
        args.extend(Self::actor(command.actor));
        args.push(Arg::Id(command.correlation_id));
        let sql = format!(
            "SELECT reservation_id, journal_id, {POSTED_AT} AS posted_at, replayed
             FROM ledger.reserve_funds($1,$2,$3,$4,$5,$6,$7,$8,$9)"
        );
        let row = self.one(&sql, args).await?;
        Ok(ReservationReceipt {
            reservation_id: row.try_get("reservation_id").map_err(decode)?,
            journal_id: row.try_get("journal_id").map_err(decode)?,
            posted_at: row.try_get("posted_at").map_err(decode)?,
            replayed: row.try_get("replayed").map_err(decode)?,
        })
    }

    async fn record_execution(
        &self,
        reservation: Uuid,
        state: ExecutionState,
        evidence_reference: &str,
    ) -> Result<ExecutionState, LedgerError> {
        let row = self
            .one(
                "SELECT ledger.record_execution_state($1,$2,$3) AS state",
                vec![
                    Arg::Id(Some(reservation)),
                    Arg::Text(Some(state.as_str().to_owned())),
                    Arg::Text(Some(evidence_reference.to_owned())),
                ],
            )
            .await?;
        let state: String = row.try_get("state").map_err(decode)?;
        ExecutionState::parse(&state).ok_or(LedgerError::Unavailable)
    }

    async fn consume(&self, command: &ConsumeCommand) -> Result<PostingReceipt, LedgerError> {
        let mut args = vec![
            Arg::Id(Some(command.reservation)),
            Arg::Text(Some(command.identity.economic_key().to_owned())),
            Arg::Text(Some(command.identity.purpose().to_owned())),
            Arg::Text(Some(command.identity.event_type().to_owned())),
            Arg::Ids(
                command
                    .credits
                    .iter()
                    .map(|(account, _)| *account)
                    .collect(),
            ),
            Arg::Texts(
                command
                    .credits
                    .iter()
                    .map(|(_, amount)| amount.to_string())
                    .collect(),
            ),
            Arg::Text(Some(command.evidence_reference.clone())),
        ];
        args.extend(Self::actor(command.actor));
        args.push(Arg::Id(command.correlation_id));
        let sql = format!(
            "SELECT journal_id, accounting_event_id, {POSTED_AT} AS posted_at, replayed
             FROM ledger.consume_reservation($1,$2,$3,$4,$5,$6,$7,$8,$9,$10)"
        );
        Self::receipt(&self.one(&sql, args).await?)
    }

    async fn release(&self, command: &ReleaseCommand) -> Result<PostingReceipt, LedgerError> {
        let mut args = vec![
            Arg::Id(Some(command.reservation)),
            Arg::Text(Some(command.economic_key.clone())),
            Arg::Text(Some(command.purpose.clone())),
            Arg::Text(Some(command.amount.to_string())),
            Arg::Text(Some(command.evidence.as_str().to_owned())),
            Arg::Text(Some(command.evidence_reference.clone())),
        ];
        args.extend(Self::actor(command.actor));
        args.push(Arg::Id(command.correlation_id));
        let sql = format!(
            "SELECT journal_id, accounting_event_id, {POSTED_AT} AS posted_at, replayed
             FROM ledger.release_reservation($1,$2,$3,$4,$5,$6,$7,$8,$9)"
        );
        Self::receipt(&self.one(&sql, args).await?)
    }

    async fn customer_balance(
        &self,
        customer: Uuid,
        asset_id: Uuid,
    ) -> Result<Option<CustomerBalance>, LedgerError> {
        let rows = self
            .call(
                "SELECT available_minor, reserved_minor, total_minor FROM ledger.customer_balance($1,$2)",
                vec![Arg::Id(Some(customer)), Arg::Id(Some(asset_id))],
            )
            .await?;
        rows.first()
            .map(|row| {
                Ok(CustomerBalance {
                    available: units(row, "available_minor")?,
                    reserved: units(row, "reserved_minor")?,
                    total: units(row, "total_minor")?,
                })
            })
            .transpose()
    }
}

/// Outcome of executing an approved correction.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CorrectionOutcome {
    Executed {
        journal_id: Uuid,
    },
    /// The reversal would have overdrawn an account; an open recovery case
    /// records the shortfall and nothing was posted.
    RecoveryRequired {
        recovery_case_id: Uuid,
    },
}

/// Finance-operations interface: platform accounts, maker-checker
/// corrections, recovery cases and reconciliation. The operator arguments are
/// claims: PostgreSQL accepts them only when they name the operator bound to
/// this pool's authenticated login, then checks that operator's permission.
impl PgLedger {
    pub async fn open_platform_account(
        &self,
        chart_code: &str,
        asset_id: Uuid,
        instance_key: &str,
        actor: Uuid,
    ) -> Result<Uuid, LedgerError> {
        let row = self
            .one(
                "SELECT ledger.open_platform_account($1,$2,$3,$4) AS id",
                vec![
                    Arg::Text(Some(chart_code.to_owned())),
                    Arg::Id(Some(asset_id)),
                    Arg::Text(Some(instance_key.to_owned())),
                    Arg::Id(Some(actor)),
                ],
            )
            .await?;
        row.try_get("id").map_err(decode)
    }

    pub async fn set_account_status(
        &self,
        account: Uuid,
        status: &str,
        reason: &str,
        actor: Uuid,
    ) -> Result<String, LedgerError> {
        let row = self
            .one(
                "SELECT ledger.set_account_status($1,$2,$3,$4) AS status",
                vec![
                    Arg::Id(Some(account)),
                    Arg::Text(Some(status.to_owned())),
                    Arg::Text(Some(reason.to_owned())),
                    Arg::Id(Some(actor)),
                ],
            )
            .await?;
        row.try_get("status").map_err(decode)
    }

    pub async fn request_reversal(
        &self,
        journal: Uuid,
        reason: &str,
        evidence_reference: &str,
        requested_by: Uuid,
    ) -> Result<Uuid, LedgerError> {
        let row = self
            .one(
                "SELECT ledger.request_correction('REVERSAL',$1,NULL,$2,$3,$4) AS id",
                vec![
                    Arg::Id(Some(journal)),
                    Arg::Text(Some(reason.to_owned())),
                    Arg::Text(Some(evidence_reference.to_owned())),
                    Arg::Id(Some(requested_by)),
                ],
            )
            .await?;
        row.try_get("id").map_err(decode)
    }

    /// Proposes a balanced adjustment. Lines are (account, asset, direction, amount).
    pub async fn request_adjustment(
        &self,
        lines: &[(
            Uuid,
            Uuid,
            deypay_domain::ledger::Direction,
            deypay_domain::units::PostingAmount,
        )],
        reason: &str,
        evidence_reference: &str,
        requested_by: Uuid,
    ) -> Result<Uuid, LedgerError> {
        let lines: Vec<serde_json::Value> = lines
            .iter()
            .map(|(account, asset, direction, amount)| {
                serde_json::json!({
                    "account_id": account.to_string(),
                    "asset_id": asset.to_string(),
                    "direction": direction.as_str(),
                    "amount_minor": amount.to_string(),
                })
            })
            .collect();
        let row = self
            .one(
                "SELECT ledger.request_correction('ADJUSTMENT',NULL,$1::text::jsonb,$2,$3,$4) AS id",
                vec![
                    Arg::Text(Some(serde_json::Value::Array(lines).to_string())),
                    Arg::Text(Some(reason.to_owned())),
                    Arg::Text(Some(evidence_reference.to_owned())),
                    Arg::Id(Some(requested_by)),
                ],
            )
            .await?;
        row.try_get("id").map_err(decode)
    }

    pub async fn decide_correction(
        &self,
        request: Uuid,
        decided_by: Uuid,
        approve: bool,
        reason: &str,
    ) -> Result<String, LedgerError> {
        let row = self
            .one(
                "SELECT ledger.decide_correction($1,$2,$3,$4) AS status",
                vec![
                    Arg::Id(Some(request)),
                    Arg::Id(Some(decided_by)),
                    Arg::Flag(approve),
                    Arg::Text(Some(reason.to_owned())),
                ],
            )
            .await?;
        row.try_get("status").map_err(decode)
    }

    pub async fn execute_correction(
        &self,
        request: Uuid,
        executed_by: Uuid,
    ) -> Result<CorrectionOutcome, LedgerError> {
        let row = self
            .one(
                "SELECT status, journal_id, recovery_case_id FROM ledger.execute_correction($1,$2)",
                vec![Arg::Id(Some(request)), Arg::Id(Some(executed_by))],
            )
            .await?;
        let status: String = row.try_get("status").map_err(decode)?;
        match status.as_str() {
            "EXECUTED" => Ok(CorrectionOutcome::Executed {
                journal_id: row.try_get("journal_id").map_err(decode)?,
            }),
            "RECOVERY_REQUIRED" => Ok(CorrectionOutcome::RecoveryRequired {
                recovery_case_id: row.try_get("recovery_case_id").map_err(decode)?,
            }),
            _ => Err(LedgerError::Unavailable),
        }
    }

    pub async fn start_reconciliation(
        &self,
        kind: &str,
        provider: Option<Uuid>,
        period: (&str, &str),
        actor: Uuid,
    ) -> Result<Uuid, LedgerError> {
        let row = self
            .one(
                "SELECT operations.start_reconciliation($1,$2,$3::timestamptz,$4::timestamptz,$5) AS id",
                vec![
                    Arg::Text(Some(kind.to_owned())),
                    Arg::Id(provider),
                    Arg::Text(Some(period.0.to_owned())),
                    Arg::Text(Some(period.1.to_owned())),
                    Arg::Id(Some(actor)),
                ],
            )
            .await?;
        row.try_get("id").map_err(decode)
    }

    /// Records ledger self-consistency findings; returns the violation count.
    pub async fn reconcile_ledger_integrity(&self, run: Uuid) -> Result<i32, LedgerError> {
        let row = self
            .one(
                "SELECT operations.reconcile_ledger_integrity($1) AS found",
                vec![Arg::Id(Some(run))],
            )
            .await?;
        row.try_get("found").map_err(decode)
    }

    /// Compares customer liabilities with custody backing for one asset.
    pub async fn reconcile_customer_liabilities(
        &self,
        run: Uuid,
        asset_id: Uuid,
    ) -> Result<String, LedgerError> {
        let row = self
            .one(
                "SELECT operations.reconcile_customer_liabilities($1,$2) AS outcome",
                vec![Arg::Id(Some(run)), Arg::Id(Some(asset_id))],
            )
            .await?;
        row.try_get("outcome").map_err(decode)
    }

    pub async fn record_balance_evidence(
        &self,
        run: Uuid,
        account: Uuid,
        observed: MinorUnits,
        evidence_reference: &str,
        evidence_sha256: [u8; 32],
        observed_at: &str,
    ) -> Result<String, LedgerError> {
        let row = self
            .one(
                "SELECT operations.record_balance_evidence($1,$2,$3,$4,$5,$6::timestamptz) AS outcome",
                vec![
                    Arg::Id(Some(run)),
                    Arg::Id(Some(account)),
                    Arg::Text(Some(observed.to_string())),
                    Arg::Text(Some(evidence_reference.to_owned())),
                    Arg::Bytes(evidence_sha256.to_vec()),
                    Arg::Text(Some(observed_at.to_owned())),
                ],
            )
            .await?;
        row.try_get("outcome").map_err(decode)
    }

    #[allow(clippy::too_many_arguments)]
    pub async fn record_transaction_evidence(
        &self,
        run: Uuid,
        external_reference: &str,
        account: Uuid,
        asset_id: Uuid,
        direction: deypay_domain::ledger::Direction,
        amount: deypay_domain::units::PostingAmount,
        evidence: (&str, [u8; 32]),
        observed_at: &str,
    ) -> Result<String, LedgerError> {
        let row = self
            .one(
                "SELECT operations.record_transaction_evidence($1,$2,$3,$4,$5,$6,$7,$8,$9::timestamptz) AS outcome",
                vec![
                    Arg::Id(Some(run)),
                    Arg::Text(Some(external_reference.to_owned())),
                    Arg::Id(Some(account)),
                    Arg::Id(Some(asset_id)),
                    Arg::Text(Some(direction.as_str().to_owned())),
                    Arg::Text(Some(amount.to_string())),
                    Arg::Text(Some(evidence.0.to_owned())),
                    Arg::Bytes(evidence.1.to_vec()),
                    Arg::Text(Some(observed_at.to_owned())),
                ],
            )
            .await?;
        row.try_get("outcome").map_err(decode)
    }

    pub async fn complete_reconciliation(
        &self,
        run: Uuid,
        expected_event_types: &[&str],
        account: Option<Uuid>,
    ) -> Result<String, LedgerError> {
        let row = self
            .one(
                "SELECT operations.complete_reconciliation($1,$2,$3) AS status",
                vec![
                    Arg::Id(Some(run)),
                    Arg::Texts(
                        expected_event_types
                            .iter()
                            .map(|value| (*value).to_owned())
                            .collect(),
                    ),
                    Arg::Id(account),
                ],
            )
            .await?;
        row.try_get("status").map_err(decode)
    }
}
