use deypay_application::{
    login::{
        CheckedCredential, LoginCredential, LoginStore, SessionIdentity, SessionLifetime,
        SessionStore,
    },
    passwords::CredentialError,
};
use sha2::{Digest, Sha256};
use sqlx::PgPool;
use uuid::Uuid;
use zeroize::Zeroizing;

/// A 256-bit bearer secret. Its only wire representation is a browser cookie.
/// Database operations take the digest. This type deliberately omits Debug and Serialize.
pub struct SessionToken(Zeroizing<[u8; 32]>);

impl SessionToken {
    pub fn generate() -> Result<Self, &'static str> {
        let mut bytes = Zeroizing::new([0; 32]);
        getrandom::getrandom(bytes.as_mut()).map_err(|_| "Session unavailable")?;
        Ok(Self(bytes))
    }

    pub fn from_cookie(value: &str) -> Option<Self> {
        if value.len() != 64
            || !value
                .bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
        {
            return None;
        }
        let mut bytes = Zeroizing::new([0; 32]);
        hex::decode_to_slice(value, bytes.as_mut()).ok()?;
        Some(Self(bytes))
    }

    pub fn cookie_value(&self) -> Zeroizing<String> {
        Zeroizing::new(hex::encode(self.0.as_ref()))
    }

    pub fn digest(&self) -> String {
        hex::encode(Sha256::digest(self.0.as_ref()))
    }
}

pub struct PgSessions(pub PgPool);

impl LoginStore for PgSessions {
    async fn credential(&self, email: &str) -> Result<Option<LoginCredential>, CredentialError> {
        let row: Option<(Uuid, String)> = sqlx::query_as(
            "SELECT customer_id,password_hash FROM identity.sandbox_login_credential($1)
             WHERE octet_length(password_hash) <= $2",
        )
        .bind(email)
        .bind(crate::password_hashing::MAX_PHC_BYTES as i32)
        .fetch_optional(&self.0)
        .await
        .map_err(|_| CredentialError::Unavailable)?;
        Ok(row.map(|(customer_id, password_hash)| LoginCredential {
            customer_id,
            password_hash,
        }))
    }
    async fn record_rejection(&self, customer: Option<Uuid>) -> Result<(), CredentialError> {
        sqlx::query("SELECT identity.record_login_rejection($1)")
            .bind(customer)
            .execute(&self.0)
            .await
            .map(|_| ())
            .map_err(|_| CredentialError::Unavailable)
    }
    async fn issue(
        &self,
        checked: CheckedCredential,
        email: &str,
        digest: &str,
        previous: Option<&str>,
        lifetime: SessionLifetime,
    ) -> Result<(), CredentialError> {
        let record = checked.snapshot();
        let issued: bool =
            sqlx::query_scalar("SELECT identity.issue_sandbox_session($1,$2,$3,$4,$5,$6,$7)")
                .bind(record.customer_id)
                .bind(email)
                .bind(&record.password_hash)
                .bind(digest)
                .bind(previous)
                .bind(lifetime.idle_seconds())
                .bind(lifetime.absolute_seconds())
                .fetch_one(&self.0)
                .await
                .map_err(|_| CredentialError::Unavailable)?;
        if issued {
            Ok(())
        } else {
            Err(CredentialError::Rejected)
        }
    }
}

impl SessionStore for PgSessions {
    async fn validate(
        &self,
        current: &str,
        replacement: Option<&str>,
        idle_seconds: i32,
    ) -> Result<Option<SessionIdentity>, CredentialError> {
        let unavailable = |_| CredentialError::Unavailable;
        let mut tx = self.0.begin().await.map_err(unavailable)?;
        let customer: Option<Uuid> =
            sqlx::query_scalar("SELECT identity.validate_sandbox_session($1,$2,$3)")
                .bind(current)
                .bind(replacement)
                .bind(idle_seconds)
                .fetch_one(&mut *tx)
                .await
                .map_err(unavailable)?;
        // Validation holds a share lock on the customer through commit. Read
        // the current status while that same lock still protects the identity.
        let identity = if let Some(customer_id) = customer {
            let account_status =
                sqlx::query_scalar("SELECT status FROM identity.users WHERE id=$1")
                    .bind(customer_id)
                    .fetch_one(&mut *tx)
                    .await
                    .map_err(unavailable)?;
            Some(SessionIdentity {
                customer_id,
                account_status,
            })
        } else {
            None
        };
        tx.commit().await.map_err(unavailable)?;
        Ok(identity)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn opaque_tokens_round_trip_without_storing_bearer_secrets() {
        let one = SessionToken::generate().unwrap();
        let two = SessionToken::generate().unwrap();
        let wire = one.cookie_value();
        assert!(one.digest() != two.digest());
        assert!(wire.as_str() != one.digest());
        assert!(SessionToken::from_cookie(&wire).unwrap().digest() == one.digest());
        for invalid in ["", &wire[..63], &"A".repeat(64), &"z".repeat(64)] {
            assert!(SessionToken::from_cookie(invalid).is_none());
        }
    }
}
