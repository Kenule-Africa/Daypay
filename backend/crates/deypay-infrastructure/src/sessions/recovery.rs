//! Password-recovery capabilities of the isolated verifier (IMP-P3-008).
//! Each call carries a fresh assertion signed with the verifier key, so the
//! verifier's database login alone can neither queue reset emails nor replace
//! a credential. PostgreSQL rechecks the release, the account state and the link.
use super::{PgSessions, digest};
use crate::rate_limit::{self, AdmissionError, AuthOperation};
use deypay_application::{
    email_verification::EmailProof,
    password_recovery::{RecoveryStore, RecoverySubject},
    passwords::CredentialError,
};
use std::sync::Arc;

/// Per-account limits that PostgreSQL applies inside the recovery operations.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RecoveryLimits {
    /// Minimum seconds between reset emails for one account (1–86400).
    pub cooldown_seconds: i32,
    /// Reset emails per account per hour (1–100).
    pub requests_per_hour: i32,
    /// Wrong secrets one link tolerates before it is revoked (1–50).
    pub guesses: i32,
}

pub struct PgRecovery {
    sessions: Arc<PgSessions>,
    limits: RecoveryLimits,
}

impl PgRecovery {
    pub fn new(sessions: Arc<PgSessions>, limits: RecoveryLimits) -> Self {
        Self { sessions, limits }
    }

    /// IMP-P3-012 deployment stage: call before reading the request body.
    pub async fn admit_operation(&self, operation: AuthOperation) -> Result<(), AdmissionError> {
        rate_limit::admit(&self.sessions.pool, operation).await
    }
}

impl RecoveryStore for PgRecovery {
    /// IMP-P3-012 subject stage, charged identically for unknown accounts.
    async fn admit(&self, subject: RecoverySubject<'_>) -> Result<(), CredentialError> {
        let pool = &self.sessions.pool;
        let charged = match subject {
            RecoverySubject::Email(email) => {
                rate_limit::admit_subject(pool, AuthOperation::ForgotPassword, email).await
            }
            RecoverySubject::Link(selector) => {
                let selector = selector.to_string();
                rate_limit::admit_subject(pool, AuthOperation::ResetPassword, &selector).await
            }
        };
        charged.map_err(|error| match error {
            AdmissionError::Limited => CredentialError::Busy,
            AdmissionError::Unavailable => CredentialError::Unavailable,
        })
    }

    async fn request(&self, email: &str) -> Result<(), CredentialError> {
        let store = &self.sessions;
        let RecoveryLimits {
            cooldown_seconds,
            requests_per_hour,
            ..
        } = self.limits;
        let (issued, tag) = store.proof(
            "request_reset",
            &[
                &digest(email),
                &cooldown_seconds.to_string(),
                &requests_per_hour.to_string(),
            ],
        );
        sqlx::query("SELECT identity.auth_request_reset($1,$2,$3,$4,$5,$6)")
            .bind(store.release)
            .bind(email)
            .bind(cooldown_seconds)
            .bind(requests_per_hour)
            .bind(issued)
            .bind(tag)
            .execute(&store.pool)
            .await
            .map(|_| ())
            .map_err(|_| CredentialError::Unavailable)
    }

    async fn reset(&self, link: &EmailProof, password_hash: &str) -> Result<bool, CredentialError> {
        let store = &self.sessions;
        let guesses = self.limits.guesses;
        let (issued, tag) = store.proof(
            "reset_password",
            &[
                &link.selector.to_string(),
                &link.digest,
                &digest(password_hash),
                &guesses.to_string(),
            ],
        );
        sqlx::query_scalar("SELECT identity.auth_reset_password($1,$2,$3,$4,$5,$6,$7)")
            .bind(store.release)
            .bind(link.selector)
            .bind(&link.digest)
            .bind(password_hash)
            .bind(guesses)
            .bind(issued)
            .bind(tag)
            .fetch_one(&store.pool)
            .await
            .map_err(|_| CredentialError::Unavailable)
    }
}
