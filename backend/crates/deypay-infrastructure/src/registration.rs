use deypay_application::registration::{Registration, RegistrationError, RegistrationRepository};
use sqlx::PgPool;

pub struct PgRegistrationRepository(pub PgPool);

impl PgRegistrationRepository {
    /// Reserve the deployment-wide sandbox budget before parsing or password work.
    pub async fn admit(&self, cap: std::num::NonZeroU32) -> Result<(), RegistrationError> {
        match sqlx::query_scalar("SELECT identity.admit_registration_request($1)")
            .bind(i64::from(cap.get()))
            .fetch_one(&self.0)
            .await
        {
            Ok(true) => Ok(()),
            Ok(false) => Err(RegistrationError::Busy),
            Err(_) => Err(RegistrationError::Unavailable),
        }
    }
}

impl RegistrationRepository for PgRegistrationRepository {
    async fn insert_unverified(
        &self,
        input: &Registration<'_>,
        password_hash: &str,
    ) -> Result<(), RegistrationError> {
        // No existing customer identifier crosses this boundary on a replay or collision.
        sqlx::query("SELECT identity.register_customer($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11)")
            .bind(input.email.trim())
            .bind(input.phone)
            .bind(input.username)
            .bind(password_hash)
            .bind(input.first_name.trim())
            .bind(input.last_name.trim())
            .bind(input.country_code)
            .bind(input.terms_version)
            .bind(input.privacy_version)
            .bind(input.accept_terms)
            .bind(input.acknowledge_privacy_notice)
            .execute(&self.0)
            .await
            .map_err(
                |error| match error.as_database_error().and_then(|e| e.code()).as_deref() {
                    Some("DP422") => RegistrationError::InvalidInput,
                    _ => RegistrationError::Unavailable,
                },
            )?;
        Ok(())
    }
}

/// Advisory username availability (IMP-P3-009). The general API may execute
/// only the IMP-P3-012 admission and `customers.username_available`;
/// registration's unique keys decide.
impl deypay_application::usernames::UsernameDirectory for PgRegistrationRepository {
    async fn admit(
        &self,
        handle: &str,
    ) -> Result<(), deypay_application::usernames::DirectoryError> {
        use crate::rate_limit::{AdmissionError, AuthOperation, admit_subject};
        use deypay_application::usernames::DirectoryError;
        admit_subject(&self.0, AuthOperation::UsernameAvailability, handle)
            .await
            .map_err(|error| match error {
                AdmissionError::Limited => DirectoryError::Limited,
                AdmissionError::Unavailable => DirectoryError::Unavailable,
            })
    }

    async fn available(
        &self,
        handle: &str,
    ) -> Result<bool, deypay_application::usernames::DirectoryError> {
        sqlx::query_scalar("SELECT customers.username_available($1)")
            .bind(handle)
            .fetch_one(&self.0)
            .await
            .map_err(|_| deypay_application::usernames::DirectoryError::Unavailable)
    }
}
