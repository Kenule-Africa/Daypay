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

mod controls;

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

/// Only the trusted verifier receives the signing key. SQL permissions alone cannot issue a session.
pub struct PgSessions {
    pool: PgPool,
    release: Uuid,
    key: Zeroizing<[u8; 32]>,
}
pub struct AuthPolicy {
    pub origin: String,
    pub lifetime: SessionLifetime,
    pub hash_costs: [u32; 4],
}
impl PgSessions {
    pub fn new(pool: PgPool, release: Uuid, key: Zeroizing<[u8; 32]>) -> Self {
        Self { pool, release, key }
    }
    pub async fn from_env(
        settings: &deypay_common::config::Settings,
    ) -> Result<(Self, AuthPolicy), &'static str> {
        let pool = crate::database::pool(settings)?;
        let release = std::env::var("DEYPAY_AUTH_RELEASE")
            .ok()
            .and_then(|s| s.parse().ok())
            .ok_or("Missing authentication policy release")?;
        let path = std::env::var_os("DEYPAY_AUTH_SIGNING_KEY_FILE")
            .ok_or("Missing authentication key mount")?;
        let encoded = Zeroizing::new(
            std::fs::read_to_string(path).map_err(|_| "Cannot read authentication key")?,
        );
        let mut key = Zeroizing::new([0; 32]);
        hex::decode_to_slice(encoded.trim_end(), key.as_mut())
            .map_err(|_| "Invalid authentication key")?;
        let store = Self::new(pool, release, key);
        let policy = store.policy(settings.environment).await?;
        Ok((store, policy))
    }
    fn mac(&self, message: &str) -> hmac::Hmac<Sha256> {
        use hmac::Mac;
        let mut mac =
            hmac::Hmac::new_from_slice(self.key.as_ref()).expect("fixed length authentication key");
        mac.update(message.as_bytes());
        mac
    }
    fn proof(&self, op: &str, fields: &[&str]) -> (i64, Vec<u8>) {
        use hmac::Mac;
        let issued = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs() as i64)
            .unwrap_or(-1);
        let payload = format!(
            "deypay-auth-v1\n{op}\n{}\n{issued}\n{}",
            self.release,
            fields.join("\n")
        );
        (issued, self.mac(&payload).finalize().into_bytes().to_vec())
    }
    pub async fn policy(
        &self,
        environment: deypay_common::config::Environment,
    ) -> Result<AuthPolicy, &'static str> {
        use hmac::Mac;
        use sqlx::Row;
        let env = environment_name(environment);
        let nonce = SessionToken::generate()?.digest();
        let row = sqlx::query(
            "SELECT * FROM identity.auth_configuration($1,$2,$3) WHERE current_user='deypay_auth' AND ($2 IN ('local','test') OR session_user=current_user)",
        )
        .bind(self.release)
        .bind(env)
        .bind(&nonce)
        .fetch_optional(&self.pool)
        .await
        .map_err(|_| "Authentication readiness check failed")?
        .ok_or("Authentication policy or runtime role unavailable")?;
        let bad = "Invalid authentication policy";
        let tag: Vec<u8> = row.try_get("key_check").map_err(|_| bad)?;
        self.mac(&format!("ready\n{}\n{nonce}", self.release))
            .verify_slice(&tag)
            .map_err(|_| "Authentication signing key mismatch")?;
        let lifetime = SessionLifetime::new(
            row.try_get("idle_seconds").map_err(|_| bad)?,
            row.try_get("absolute_seconds").map_err(|_| bad)?,
        )
        .ok_or(bad)?;
        let cost = |name| {
            row.try_get::<i32, _>(name)
                .ok()
                .and_then(|v| u32::try_from(v).ok())
                .filter(|v| *v > 0)
                .ok_or(bad)
        };
        Ok(AuthPolicy {
            origin: row.try_get("origin").map_err(|_| bad)?,
            lifetime,
            hash_costs: [
                cost("memory_kib")?,
                cost("passes")?,
                cost("lanes")?,
                cost("slots")?,
            ],
        })
    }
    pub async fn ready(&self) -> bool {
        let check =
            sqlx::query_scalar("SELECT identity.auth_ready($1) AND current_user='deypay_auth'")
                .bind(self.release)
                .fetch_one(&self.pool);
        matches!(
            tokio::time::timeout(std::time::Duration::from_secs(2), check).await,
            Ok(Ok(true))
        )
    }
}
pub fn environment_name(environment: deypay_common::config::Environment) -> &'static str {
    use deypay_common::config::Environment::*;
    match environment {
        Local => "local",
        Test => "test",
        Staging => "staging",
        Production => "production",
    }
}

/// The general API holds no verifier key and cannot rotate or issue tokens.
pub struct PgSessionReader {
    pub pool: PgPool,
    pub release: Uuid,
    pub environment: &'static str,
    pub origin: String,
}
impl PgSessionReader {
    pub async fn ready(&self) -> bool {
        let probe=sqlx::query_scalar("SELECT current_user='deypay_app' AND ($2 IN ('local','test') OR session_user=current_user)
            AND identity.auth_reader_ready($1,$2,$3)").bind(self.release).bind(self.environment).bind(&self.origin).fetch_one(&self.pool);
        matches!(
            tokio::time::timeout(std::time::Duration::from_secs(2), probe).await,
            Ok(Ok(true))
        )
    }
}
impl SessionStore for PgSessionReader {
    async fn validate(
        &self,
        digest: &str,
        replacement: Option<&str>,
    ) -> Result<Option<SessionIdentity>, CredentialError> {
        if replacement.is_some() {
            return Err(CredentialError::Unavailable);
        }
        let row: Option<(Uuid, String)> = sqlx::query_as(
            "SELECT customer_id,account_status FROM identity.auth_principal($1,$2,$3,$4)",
        )
        .bind(self.release)
        .bind(self.environment)
        .bind(&self.origin)
        .bind(digest)
        .fetch_optional(&self.pool)
        .await
        .map_err(|_| CredentialError::Unavailable)?;
        Ok(row.map(|(customer_id, account_status)| SessionIdentity {
            customer_id,
            account_status,
        }))
    }
}
fn digest(value: &str) -> String {
    hex::encode(Sha256::digest(value.as_bytes()))
}

impl LoginStore for PgSessions {
    async fn admit(&self, identifier: Option<&str>) -> Result<(), CredentialError> {
        let ok: bool = sqlx::query_scalar("SELECT identity.auth_admit($1,$2)")
            .bind(self.release)
            .bind(identifier)
            .fetch_one(&self.pool)
            .await
            .map_err(|_| CredentialError::Unavailable)?;
        if ok {
            Ok(())
        } else {
            Err(CredentialError::Busy)
        }
    }
    async fn credential(&self, email: &str) -> Result<Option<LoginCredential>, CredentialError> {
        let (at, tag) = self.proof("lookup", &[&digest(email)]);
        let row: Option<(Uuid, String)> = sqlx::query_as(
            "SELECT customer_id,password_hash FROM identity.auth_credential($1,$2,$3,$4) WHERE octet_length(password_hash)<=$5",
        )
        .bind(self.release)
        .bind(email)
        .bind(at)
        .bind(tag)
        .bind(crate::password_hashing::MAX_PHC_BYTES as i32)
        .fetch_optional(&self.pool)
        .await
        .map_err(|_| CredentialError::Unavailable)?;
        Ok(row.map(|(customer_id, password_hash)| LoginCredential {
            customer_id,
            password_hash,
        }))
    }
    async fn record_rejection(&self, customer: Option<Uuid>) -> Result<(), CredentialError> {
        let (at, tag) = self.proof(
            "reject",
            &[&customer.map(|id| id.to_string()).unwrap_or_default()],
        );
        sqlx::query("SELECT identity.auth_reject($1,$2,$3,$4)")
            .bind(self.release)
            .bind(customer)
            .bind(at)
            .bind(tag)
            .execute(&self.pool)
            .await
            .map(|_| ())
            .map_err(|_| CredentialError::Unavailable)
    }
    async fn issue(
        &self,
        checked: CheckedCredential,
        email: &str,
        new_digest: &str,
        previous: Option<&str>,
    ) -> Result<(), CredentialError> {
        let record = checked.snapshot();
        let fingerprint = digest(&record.password_hash);
        let (at, tag) = self.proof(
            "issue",
            &[
                &record.customer_id.to_string(),
                &digest(email),
                &fingerprint,
                new_digest,
                previous.unwrap_or(""),
            ],
        );
        let issued: bool =
            sqlx::query_scalar("SELECT identity.auth_issue($1,$2,$3,$4,$5,$6,$7,$8)")
                .bind(self.release)
                .bind(record.customer_id)
                .bind(email)
                .bind(fingerprint)
                .bind(new_digest)
                .bind(previous)
                .bind(at)
                .bind(tag)
                .fetch_one(&self.pool)
                .await
                .map_err(|_| CredentialError::Unavailable)?;
        if issued {
            Ok(())
        } else {
            Err(CredentialError::Rejected)
        }
    }
}
// Only the admission SQLSTATE is public. Never propagate query details or identifiers.
fn session_error(error: sqlx::Error) -> CredentialError {
    if error.as_database_error().and_then(|e| e.code()).as_deref() == Some("DP429") {
        CredentialError::Busy
    } else {
        CredentialError::Unavailable
    }
}

impl SessionStore for PgSessions {
    async fn validate(
        &self,
        current: &str,
        replacement: Option<&str>,
    ) -> Result<Option<SessionIdentity>, CredentialError> {
        let (at, tag) = self.proof("validate", &[current, replacement.unwrap_or("")]);
        let row: Option<(Uuid, String)> = sqlx::query_as(
            "SELECT customer_id,account_status FROM identity.auth_validate($1,$2,$3,$4,$5)",
        )
        .bind(self.release)
        .bind(current)
        .bind(replacement)
        .bind(at)
        .bind(tag)
        .fetch_optional(&self.pool)
        .await
        .map_err(session_error)?;
        Ok(row.map(|(customer_id, account_status)| SessionIdentity {
            customer_id,
            account_status,
        }))
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
