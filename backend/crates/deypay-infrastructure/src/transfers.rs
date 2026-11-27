//! PostgreSQL internal-transfer adapter (IMP-P7-001–009).
//!
//! [`PgTransfers`] runs the customer functions of migration
//! `202611271601_internal_transfers.sql` as the pool's login (`deypay_app`),
//! one function per transaction. Each function validates its input again,
//! scopes every read to the authenticated customer and, for a transfer, posts
//! the ledger journal atomically with the business record, the consumed
//! authorization proof and the idempotency record.
use deypay_application::transfers::*;
use deypay_domain::money::Precision;
use deypay_domain::units::MinorUnits;
use sqlx::{PgPool, Row, postgres::PgRow};
use uuid::Uuid;
use zeroize::Zeroizing;

/// Attempts for deadlocks and serialization failures. A refused attempt
/// rolls back completely, so repeating it cannot duplicate a transfer.
const MAX_ATTEMPTS: usize = 3;

/// A fresh single-use authorization token: 32 bytes from the operating
/// system's secure random source, as 64 lowercase hex characters. Only its
/// SHA-256 digest is stored.
pub fn generate_proof_token() -> Result<String, TransferError> {
    let mut bytes = Zeroizing::new([0_u8; 32]);
    getrandom::getrandom(bytes.as_mut()).map_err(|_| TransferError::Unavailable)?;
    Ok(hex::encode(bytes.as_ref()))
}

fn map_error(error: &sqlx::Error) -> TransferError {
    let sqlx::Error::Database(db) = error else {
        return TransferError::Unavailable;
    };
    match db.code().as_deref() {
        Some("TR001" | "22P02") => TransferError::InvalidRequest,
        Some("TR002") => TransferError::InvalidAmount,
        Some("TR003") => TransferError::UnsupportedAsset,
        Some("TR004") => TransferError::RecipientNotFound,
        Some("TR005") => TransferError::RecipientUnavailable,
        Some("TR006") => TransferError::SelfTransfer,
        Some("TR007") => TransferError::KycRequired,
        Some("TR008" | "LE003" | "LE009") => TransferError::Restricted,
        Some("TR009") => TransferError::AuthorizationInvalid,
        Some("TR010") => TransferError::IdempotencyConflict,
        Some("TR011") => TransferError::NotEnabled,
        Some("TR012" | "LE010") => TransferError::AssetUnavailable,
        Some("TR013") => TransferError::LookupLimited,
        Some("TR014") => TransferError::LookupUnavailable,
        Some("TR015") => TransferError::SessionInvalid,
        Some("LE002") => TransferError::InsufficientFunds,
        // Activation was checked first, so the ledger refuses only the ceiling.
        Some("LE014") => TransferError::LimitExceeded,
        Some("55P03" | "40P01" | "40001") => TransferError::Busy,
        _ => TransferError::Unavailable,
    }
}

fn retryable(error: &sqlx::Error) -> bool {
    matches!(error, sqlx::Error::Database(db)
        if matches!(db.code().as_deref(), Some("40P01" | "40001")))
}

fn corrupt(error: sqlx::Error) -> TransferError {
    tracing::error!(event = "transfer_decode_failure", error = %error, "Transfer result could not be decoded");
    TransferError::Corrupt
}

enum Arg {
    Text(Option<String>),
    Id(Option<Uuid>),
    Int(i32),
}

/// The customer API's transfer store. Runs as the pool's login (`deypay_app`).
#[derive(Clone)]
pub struct PgTransfers {
    pool: PgPool,
}

impl PgTransfers {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    async fn call(&self, sql: &str, args: Vec<Arg>) -> Result<Vec<PgRow>, TransferError> {
        let mut attempt = 0;
        loop {
            attempt += 1;
            match self.try_call(sql, &args).await {
                Ok(rows) => return Ok(rows),
                Err(error) if retryable(&error) && attempt < MAX_ATTEMPTS => continue,
                Err(error) => {
                    let mapped = map_error(&error);
                    if matches!(mapped, TransferError::Unavailable | TransferError::Busy) {
                        let sqlstate = error.as_database_error().and_then(|db| db.code());
                        tracing::warn!(event = "transfer_database_failure", sqlstate = sqlstate.as_deref(), error = %error, "Transfer call failed");
                    }
                    return Err(mapped);
                }
            }
        }
    }

    async fn try_call(&self, sql: &str, args: &[Arg]) -> Result<Vec<PgRow>, sqlx::Error> {
        let mut tx = self.pool.begin().await?;
        let mut query = sqlx::query(sql);
        for arg in args {
            query = match arg {
                Arg::Text(value) => query.bind(value.clone()),
                Arg::Id(value) => query.bind(*value),
                Arg::Int(value) => query.bind(*value),
            };
        }
        let rows = query.fetch_all(&mut *tx).await?;
        tx.commit().await?;
        Ok(rows)
    }
}

fn text(value: &str) -> Arg {
    Arg::Text(Some(value.to_owned()))
}

fn precision(row: &PgRow) -> Result<Precision, TransferError> {
    let value: i16 = row.try_get("accounting_precision").map_err(corrupt)?;
    u8::try_from(value)
        .ok()
        .and_then(Precision::new)
        .ok_or(TransferError::Corrupt)
}

fn units(row: &PgRow, column: &str) -> Result<MinorUnits, TransferError> {
    let value: String = row.try_get(column).map_err(corrupt)?;
    value.parse().map_err(|_| TransferError::Corrupt)
}

fn terms_args(terms: &TransferTerms) -> [Arg; 4] {
    [
        Arg::Id(Some(terms.recipient)),
        text(&terms.asset),
        text(&terms.amount.units().to_string()),
        Arg::Text(terms.description.clone()),
    ]
}

fn record(row: &PgRow) -> Result<TransferRecord, TransferError> {
    let status: String = row.try_get("status").map_err(corrupt)?;
    let direction: String = row.try_get("direction").map_err(corrupt)?;
    if public_status(&status).is_none() {
        return Err(TransferError::Corrupt);
    }
    Ok(TransferRecord {
        id: row.try_get("transaction_id").map_err(corrupt)?,
        reference: row.try_get("reference").map_err(corrupt)?,
        status,
        direction: Direction::parse(&direction).ok_or(TransferError::Corrupt)?,
        asset: row.try_get("asset").map_err(corrupt)?,
        precision: precision(row)?,
        amount: units(row, "amount_minor")?,
        fee: units(row, "fee_minor")?,
        description: row.try_get("description").map_err(corrupt)?,
        counterparty: Counterparty {
            id: row.try_get("counterparty_id").map_err(corrupt)?,
            username: row.try_get("counterparty_username").map_err(corrupt)?,
            display_name: row.try_get("counterparty_display_name").map_err(corrupt)?,
        },
        created_at: row.try_get("created_at").map_err(corrupt)?,
        completed_at: row.try_get("completed_at").map_err(corrupt)?,
    })
}

impl TransferStore for PgTransfers {
    async fn resolve(
        &self,
        customer: Uuid,
        handle: &str,
    ) -> Result<Option<Recipient>, TransferError> {
        let rows = self
            .call(
                "SELECT recipient_id, username, display_name, can_receive
                 FROM payments.resolve_recipient($1, $2)",
                vec![Arg::Id(Some(customer)), text(handle)],
            )
            .await?;
        rows.first()
            .map(|row| {
                Ok(Recipient {
                    id: row.try_get("recipient_id").map_err(corrupt)?,
                    username: row.try_get("username").map_err(corrupt)?,
                    display_name: row.try_get("display_name").map_err(corrupt)?,
                    can_receive: row.try_get("can_receive").map_err(corrupt)?,
                })
            })
            .transpose()
    }

    async fn assets(&self) -> Result<Vec<TransferAsset>, TransferError> {
        self.call(
            "SELECT asset_id, symbol, accounting_precision FROM payments.transfer_assets()",
            vec![],
        )
        .await?
        .iter()
        .map(|row| {
            Ok(TransferAsset {
                id: row.try_get("asset_id").map_err(corrupt)?,
                symbol: row.try_get("symbol").map_err(corrupt)?,
                precision: precision(row)?,
            })
        })
        .collect()
    }

    async fn authorize(
        &self,
        customer: Uuid,
        session_digest: &str,
        terms: &TransferTerms,
    ) -> Result<AuthorizationChallenge, TransferError> {
        let mut args = vec![Arg::Id(Some(customer)), text(session_digest)];
        args.extend(terms_args(terms));
        let rows = self
            .call(
                "SELECT * FROM payments.authorize_internal_transfer($1, $2, $3, $4, $5, $6)",
                args,
            )
            .await?;
        let row = rows.first().ok_or(TransferError::Corrupt)?;
        Ok(AuthorizationChallenge {
            id: row.try_get("challenge_id").map_err(corrupt)?,
            expires_at: row.try_get("expires_at").map_err(corrupt)?,
            recipient: row.try_get("recipient_id").map_err(corrupt)?,
            recipient_username: row.try_get("recipient_username").map_err(corrupt)?,
            recipient_display_name: row.try_get("recipient_display_name").map_err(corrupt)?,
            asset: row.try_get("asset").map_err(corrupt)?,
            precision: precision(row)?,
            amount: units(row, "amount_minor")?,
            fee: units(row, "fee_minor")?,
            description: row.try_get("description").map_err(corrupt)?,
            verification_method: row.try_get("verification_method").map_err(corrupt)?,
        })
    }

    async fn confirm(
        &self,
        customer: Uuid,
        session_digest: &str,
        challenge: Uuid,
        proof_digest: &str,
    ) -> Result<String, TransferError> {
        let rows = self
            .call(
                "SELECT payments.confirm_transfer_authorization($1, $2, $3, $4) AS expires_at",
                vec![
                    Arg::Id(Some(customer)),
                    text(session_digest),
                    Arg::Id(Some(challenge)),
                    text(proof_digest),
                ],
            )
            .await?;
        let row = rows.first().ok_or(TransferError::Corrupt)?;
        row.try_get("expires_at").map_err(corrupt)
    }

    async fn execute(
        &self,
        customer: Uuid,
        session_digest: &str,
        idempotency_key: &str,
        proof_digest: &str,
        terms: &TransferTerms,
    ) -> Result<Executed, TransferError> {
        let mut args = vec![
            Arg::Id(Some(customer)),
            text(session_digest),
            text(idempotency_key),
            text(proof_digest),
        ];
        args.extend(terms_args(terms));
        let rows = self
            .call(
                "SELECT * FROM payments.execute_internal_transfer($1, $2, $3, $4, $5, $6, $7, $8)",
                args,
            )
            .await?;
        let row = rows.first().ok_or(TransferError::Corrupt)?;
        Ok(Executed {
            replayed: row.try_get("replayed").map_err(corrupt)?,
            transfer: record(row)?,
        })
    }

    async fn history(
        &self,
        customer: Uuid,
        cursor: Option<Uuid>,
        limit: u32,
    ) -> Result<Vec<TransferRecord>, TransferError> {
        let limit = i32::try_from(limit).map_err(|_| TransferError::InvalidRequest)?;
        self.call(
            "SELECT * FROM payments.customer_transactions($1, $2, $3)",
            vec![Arg::Id(Some(customer)), Arg::Id(cursor), Arg::Int(limit)],
        )
        .await?
        .iter()
        .map(record)
        .collect()
    }

    async fn transaction(
        &self,
        customer: Uuid,
        id: Uuid,
    ) -> Result<Option<TransferRecord>, TransferError> {
        self.call(
            "SELECT * FROM payments.customer_transaction($1, $2)",
            vec![Arg::Id(Some(customer)), Arg::Id(Some(id))],
        )
        .await?
        .first()
        .map(record)
        .transpose()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn proof_tokens_are_fresh_lowercase_hex() {
        let first = generate_proof_token().unwrap();
        let second = generate_proof_token().unwrap();
        assert_ne!(first, second);
        assert!(token_digest(&first).is_some());
        assert_eq!(first.len(), 64);
    }
}
