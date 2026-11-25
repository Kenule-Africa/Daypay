//! PostgreSQL KYC case store for customer operations (IMP-P4-005 prerequisite).
//!
//! Writes go only through the two fixed database operations in migration
//! `202611250001_kyc_case_writes.sql`; the runtime role holds no INSERT or
//! UPDATE on `compliance.kyc_cases`. Grant them with
//! `infrastructure/postgres/kyc-grants.psql`.
//!
//! - A first case is inserted by `compliance.open_first_kyc_case`, which
//!   serializes requests per customer with a transaction-scoped advisory lock
//!   and inserts a `DRAFT` only when the customer has no case.
//! - Only customer submission is written, by `compliance.submit_kyc_case`, and
//!   only while the case still has the expected owner, ID and status. Provider
//!   and reviewer transitions are refused without writing; they need their own
//!   reviewed write paths (IMP-P4-009, Phase 8).
//!
//! Each call is one statement in its own transaction, so a database error
//! leaves nothing persisted.
use crate::repositories::storage_error;
use deypay_application::kyc::{CustomerKycCases, KycCaseStatus, KycCaseStore, NewKycCase};
use deypay_application::repositories::{KycRepository, Lookup, RepositoryError};
use deypay_domain::entities::KycCase;
use deypay_domain::kyc::{KycCaseCommand, KycCaseTransition};
use sqlx::{PgPool, Row, postgres::PgRow};
use uuid::Uuid;

pub struct PgKycCaseStore {
    pool: PgPool,
}

impl PgKycCaseStore {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

fn case(row: &PgRow, id: &str, user_id: &str, status: &str) -> Result<KycCase, RepositoryError> {
    Ok(KycCase {
        id: row.try_get(id).map_err(storage_error)?,
        user_id: row.try_get(user_id).map_err(storage_error)?,
        status: row.try_get(status).map_err(storage_error)?,
    })
}

impl KycRepository for PgKycCaseStore {
    async fn kyc_case(&self, customer_id: Uuid, case_id: Uuid) -> Lookup<KycCase> {
        let row = sqlx::query(
            "SELECT id, user_id, status FROM compliance.kyc_cases WHERE user_id = $1 AND id = $2",
        )
        .bind(customer_id)
        .bind(case_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(storage_error)?;
        row.map(|row| case(&row, "id", "user_id", "status"))
            .transpose()
    }
}

impl CustomerKycCases for PgKycCaseStore {
    async fn customer_kyc_case(&self, customer_id: Uuid) -> Lookup<KycCase> {
        // Two rows are enough to detect a breach of the one-case rule.
        let rows = sqlx::query(
            "SELECT id, user_id, status FROM compliance.kyc_cases WHERE user_id = $1 LIMIT 2",
        )
        .bind(customer_id)
        .fetch_all(&self.pool)
        .await
        .map_err(storage_error)?;
        match rows.as_slice() {
            [] => Ok(None),
            [row] => case(row, "id", "user_id", "status").map(Some),
            _ => Err(RepositoryError::InvalidRecord),
        }
    }
}

impl KycCaseStore for PgKycCaseStore {
    async fn insert_first_case(&self, request: NewKycCase) -> Lookup<KycCase> {
        // The database operation always inserts DRAFT; refuse any other request
        // rather than silently persist something different.
        if request.status() != KycCaseStatus::Draft {
            return Err(RepositoryError::InvalidRecord);
        }
        let rows = sqlx::query(
            "SELECT case_id, owner_id, case_status FROM compliance.open_first_kyc_case($1)",
        )
        .bind(request.owner_id())
        .fetch_all(&self.pool)
        .await
        .map_err(storage_error)?;
        match rows.as_slice() {
            [] => Ok(None),
            [row] => case(row, "case_id", "owner_id", "case_status").map(Some),
            _ => Err(RepositoryError::InvalidRecord),
        }
    }

    async fn apply_transition(
        &self,
        owner_id: Uuid,
        case_id: Uuid,
        transition: KycCaseTransition,
    ) -> Result<bool, RepositoryError> {
        if transition.command() != KycCaseCommand::Submit
            || transition.to() != KycCaseStatus::Submitted
        {
            return Err(RepositoryError::InvalidRecord);
        }
        sqlx::query_scalar("SELECT compliance.submit_kyc_case($1, $2, $3)")
            .bind(owner_id)
            .bind(case_id)
            .bind(transition.from().as_str())
            .fetch_one(&self.pool)
            .await
            .map_err(storage_error)
    }
}
