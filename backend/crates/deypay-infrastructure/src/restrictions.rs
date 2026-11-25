//! PostgreSQL eligibility evidence (IMP-P4-006).
//!
//! Reads the account status, the number of active account restrictions and the
//! customer's KYC case statuses in one statement, and so in one snapshot. The
//! runtime role reads only `account_restrictions(user_id, active)`: never reason
//! codes, restriction types or actors (`infrastructure/postgres/kyc-grants.psql`).
//! Nothing here writes, so restriction history is never changed.
use crate::repositories::storage_error;
use deypay_application::repositories::Lookup;
use deypay_application::restrictions::EligibilityEvidenceSource;
use deypay_domain::restrictions::EligibilityRecord;
use sqlx::{PgPool, Row};
use uuid::Uuid;

pub struct PgEligibilityEvidence {
    pool: PgPool,
}

impl PgEligibilityEvidence {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

impl EligibilityEvidenceSource for PgEligibilityEvidence {
    async fn eligibility_record(&self, customer_id: Uuid) -> Lookup<EligibilityRecord> {
        // Two KYC cases are enough to detect a breach of the one-case rule.
        let row = sqlx::query(
            "SELECT u.status,
                (SELECT count(*) FROM compliance.account_restrictions r
                    WHERE r.user_id = u.id AND r.active) AS active_restrictions,
                ARRAY(SELECT k.status FROM compliance.kyc_cases k
                    WHERE k.user_id = u.id LIMIT 2) AS kyc_case_statuses
            FROM identity.users u WHERE u.id = $1",
        )
        .bind(customer_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(storage_error)?;
        row.map(|row| {
            Ok(EligibilityRecord {
                account_status: row.try_get("status").map_err(storage_error)?,
                active_restrictions: row.try_get("active_restrictions").map_err(storage_error)?,
                kyc_case_statuses: row.try_get("kyc_case_statuses").map_err(storage_error)?,
            })
        })
        .transpose()
    }
}
