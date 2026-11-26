//! Reset-email delivery (IMP-P3-008). Mirrors the verification worker: the
//! link secret is derived from a dedicated key, PostgreSQL stores only its
//! SHA-256 digest, and a retry resends byte-identical content. The verifier
//! redeems links; this worker never sees a password.
use crate::email_delivery::{
    Delivery, EmailDelivery, MockDelivery, Resend, SENDER, password_reset_message,
};
use deypay_application::{email_verification::EmailProof, registration::valid_email};
use deypay_common::{
    config::Environment,
    email::{number, scope},
};
use hmac::{Hmac, Mac};
use reqwest::Url;
use sha2::{Digest, Sha256};
use sqlx::{FromRow, PgPool};
use std::{sync::Arc, time::Duration};
use uuid::Uuid;
use zeroize::Zeroizing;

/// Owner-approved default link lifetime (2026-11-25).
pub const DEFAULT_TTL_SECONDS: i32 = 1800;

pub struct RecoveryWorker {
    delivery: Arc<dyn EmailDelivery>,
    secret: Zeroizing<[u8; 32]>,
    base_url: Url,
    scope: &'static str,
    ttl: i32,
    tries: i32,
    lease: i32,
    backoff: i32,
}

/// `PASSWORD_RECOVERY_ENABLED` gates both the worker and the verifier routes.
pub fn recovery_enabled() -> Result<bool, &'static str> {
    match std::env::var("PASSWORD_RECOVERY_ENABLED").as_deref() {
        Ok("true") => Ok(true),
        Err(std::env::VarError::NotPresent) | Ok("false") => Ok(false),
        _ => Err("Invalid PASSWORD_RECOVERY_ENABLED"),
    }
}

impl RecoveryWorker {
    pub fn from_env(env: Environment) -> Result<Option<Self>, &'static str> {
        if !recovery_enabled()? {
            return Ok(None);
        }
        let local = scope(env) == "sandbox";
        let timeout = number("EMAIL_TIMEOUT_SECONDS", 10, 1, 60)?;
        let delivery: Arc<dyn EmailDelivery> =
            match (local, std::env::var("EMAIL_PROVIDER").as_deref()) {
                (true, Ok("mock")) => Arc::new(MockDelivery::default()),
                (false, Ok("resend")) => {
                    if std::env::var("EMAIL_FROM").as_deref() != Ok(SENDER) {
                        return Err("Invalid EMAIL_FROM");
                    }
                    let key = Zeroizing::new(
                        std::env::var("RESEND_API_KEY").map_err(|_| "Missing RESEND_API_KEY")?,
                    );
                    Arc::new(Resend::new(&key, Duration::from_secs(timeout as u64))?)
                }
                _ => return Err("EMAIL_PROVIDER must match the deployment environment"),
            };
        let encoded = Zeroizing::new(
            std::env::var("PASSWORD_RESET_TOKEN_KEY")
                .map_err(|_| "Missing PASSWORD_RESET_TOKEN_KEY")?,
        );
        if std::env::var("EMAIL_VERIFY_TOKEN_KEY")
            .is_ok_and(|other| other.trim().eq_ignore_ascii_case(encoded.trim()))
        {
            return Err("PASSWORD_RESET_TOKEN_KEY must differ from EMAIL_VERIFY_TOKEN_KEY");
        }
        let mut key = Zeroizing::new([0u8; 32]);
        hex::decode_to_slice(encoded.trim(), key.as_mut())
            .map_err(|_| "Invalid PASSWORD_RESET_TOKEN_KEY")?;
        let url = std::env::var("PASSWORD_RESET_BASE_URL")
            .map_err(|_| "Missing PASSWORD_RESET_BASE_URL")?;
        Ok(Some(Self {
            delivery,
            secret: key,
            base_url: reset_url(&url, local)?,
            scope: scope(env),
            ttl: number("PASSWORD_RESET_TTL_SECONDS", DEFAULT_TTL_SECONDS, 60, 3600)?,
            tries: number("EMAIL_MAX_ATTEMPTS", 5, 1, 20)?,
            lease: timeout + 15,
            backoff: number("EMAIL_RETRY_BASE_SECONDS", 5, 1, 300)?,
        }))
    }

    /// Explicit local harness. Hosted processes can select only the Resend adapter.
    pub fn sandbox(
        delivery: Arc<dyn EmailDelivery>,
        key: [u8; 32],
        url: &str,
    ) -> Result<Self, &'static str> {
        Ok(Self {
            delivery,
            secret: Zeroizing::new(key),
            base_url: reset_url(url, true)?,
            scope: "sandbox",
            ttl: DEFAULT_TTL_SECONDS,
            tries: 5,
            lease: 25,
            backoff: 5,
        })
    }

    /// Publishes one pending intent and handles at most one reset email.
    pub async fn tick(&self, pool: &PgPool) -> sqlx::Result<bool> {
        sqlx::query("SELECT integrations.dispatch_password_reset_email($1)")
            .bind(self.tries)
            .execute(pool)
            .await?;
        let mut tx = pool.begin().await?;
        let claim = sqlx::query_as::<_, Claim>(
            "SELECT *,to_char(to_timestamp(expiry) AT TIME ZONE 'UTC','YYYY-MM-DD HH24:MI:SS') AS display_expiry
             FROM integrations.claim_password_reset_email($1,$2,$3)",
        )
        .bind(self.scope)
        .bind(self.ttl)
        .bind(self.lease)
        .fetch_optional(&mut *tx)
        .await?;
        let Some(claim) = claim else {
            tx.commit().await?;
            return Ok(false);
        };
        if !valid_email(&claim.address) {
            tx.commit().await?;
            self.finish(pool, &claim, "INVALID_ADDRESS", 0, None)
                .await?;
            return Ok(true);
        }
        // Deterministic per message, so a retry sends the same link.
        let mut mac =
            Hmac::<Sha256>::new_from_slice(self.secret.as_slice()).expect("fixed HMAC key");
        mac.update(b"DEYPAY/password-reset/v1\0");
        mac.update(claim.message_id.as_bytes());
        mac.update(claim.customer.as_bytes());
        mac.update(claim.address.as_bytes());
        let secret = Zeroizing::new(<[u8; 32]>::from(mac.finalize().into_bytes()));
        let digest = EmailProof::digest(&secret);
        let mut link = self.base_url.clone();
        link.set_fragment(Some(&format!(
            "{}.{}",
            claim.message_id,
            hex::encode(secret.as_slice())
        )));
        let message = password_reset_message(&claim.address, &link, &claim.display_expiry);
        let bytes = serde_json::to_vec(&message).expect("string-only email message");
        let fingerprint = hex::encode(Sha256::digest(bytes));
        let ready = sqlx::query_scalar::<_, bool>(
            "SELECT integrations.prepare_password_reset_email($1,$2,$3,$4)",
        )
        .bind(claim.message_id)
        .bind(claim.lease)
        .bind(digest)
        .bind(fingerprint)
        .fetch_one(&mut *tx)
        .await?;
        tx.commit().await?;
        let backoff = (i64::from(self.backoff) << (claim.attempt - 1).min(16)).min(900);
        let (outcome, delay, receipt) = if ready {
            match self.delivery.send(claim.message_id, &message).await {
                Delivery::Accepted(receipt) => ("ACCEPTED", 0, receipt),
                Delivery::Permanent => ("PERMANENT", 0, None),
                Delivery::Transient => ("TRANSIENT", backoff, None),
                Delivery::RateLimited(secs) => {
                    ("RATE_LIMITED", backoff.max(secs.min(86400) as i64), None)
                }
            }
        } else {
            ("MESSAGE_CHANGED", 0, None)
        };
        self.finish(pool, &claim, outcome, delay, receipt).await?;
        Ok(true)
    }

    async fn finish(
        &self,
        pool: &PgPool,
        claim: &Claim,
        outcome: &'static str,
        delay: i64,
        receipt: Option<Uuid>,
    ) -> sqlx::Result<()> {
        let updated = sqlx::query_scalar::<_, bool>(
            "SELECT integrations.finish_password_reset_email($1,$2,$3,$4,$5)",
        )
        .bind(claim.message_id)
        .bind(claim.lease)
        .bind(outcome)
        .bind(delay)
        .bind(receipt)
        .fetch_one(pool)
        .await?;
        if updated {
            tracing::info!(message_id=%claim.message_id,attempt=claim.attempt,outcome,"Password reset email delivery");
        }
        Ok(())
    }
}

#[derive(FromRow)]
struct Claim {
    message_id: Uuid,
    lease: Uuid,
    customer: Uuid,
    address: String,
    display_expiry: String,
    attempt: i32,
}

fn reset_url(value: &str, local: bool) -> Result<Url, &'static str> {
    let url = Url::parse(value).map_err(|_| "Invalid PASSWORD_RESET_BASE_URL")?;
    let loopback = matches!(url.host_str(), Some("localhost" | "127.0.0.1" | "[::1]"));
    if url.host().is_none()
        || !(url.scheme() == "https" || local && loopback && url.scheme() == "http")
        || !url.username().is_empty()
        || url.password().is_some()
        || url.query().is_some()
        || url.fragment().is_some()
        || url.path() != "/reset-password"
    {
        return Err("Invalid PASSWORD_RESET_BASE_URL");
    }
    Ok(url)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_the_configured_reset_page_can_receive_links() {
        assert!(reset_url("https://app.example.test/reset-password", false).is_ok());
        assert!(reset_url("http://127.0.0.1:8000/reset-password", true).is_ok());
        for url in [
            "http://app.example.test/reset-password",
            "http://localhost/reset-password",
            "https://user:pass@app.example.test/reset-password",
            "https://app.example.test/verify-email",
            "https://app.example.test/reset-password?next=elsewhere",
            "https://app.example.test/reset-password#old",
        ] {
            assert!(reset_url(url, false).is_err(), "{url}");
        }
    }

    #[test]
    fn reset_emails_carry_the_link_once_and_no_tracking_or_account_data() {
        let link = Url::parse("https://app.example.test/reset-password#sel.secret").unwrap();
        let message = password_reset_message("ada@example.test", &link, "2030-05-20 09:15:30");
        assert_eq!(message.kind, "reset-password");
        assert_eq!(message.to, ["ada@example.test".to_owned()]);
        assert!(message.text.contains(link.as_str()));
        assert!(message.text.contains("signs you out on every device"));
        assert!(message.text.contains("did not ask to reset"));
        assert_eq!(message.html.matches(link.as_str()).count(), 1);
        let wire = serde_json::to_value(&message).unwrap();
        assert!(wire.get("kind").is_none());
    }
}
