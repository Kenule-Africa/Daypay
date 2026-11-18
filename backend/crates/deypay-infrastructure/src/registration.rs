use deypay_application::registration::{Registration, RegistrationError, RegistrationRepository};
use sqlx::PgPool;

pub struct PgRegistrationRepository(pub PgPool);

impl RegistrationRepository for PgRegistrationRepository {
    async fn insert_unverified(
        &self,
        input: &Registration<'_>,
        password_hash: &str,
    ) -> Result<(), RegistrationError> {
        // No existing customer identifier crosses this boundary on a replay or collision.
        sqlx::query("SELECT identity.register_customer($1,$2,$3,$4,$5,$6,$7,$8,$9,$10)")
            .bind(input.email.trim())
            .bind(input.phone)
            .bind(input.username)
            .bind(password_hash)
            .bind(input.first_name.trim())
            .bind(input.last_name.trim())
            .bind(input.country_code)
            .bind(input.terms_version)
            .bind(input.privacy_version)
            .bind(input.accepted_at_unix)
            .execute(&self.0)
            .await
            .map_err(|_| RegistrationError::Unavailable)?;
        Ok(())
    }
}
