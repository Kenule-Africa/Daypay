use crate::email_delivery::{
    Delivery, EmailDelivery, MockDelivery, Resend, SENDER, verification_message,
};
use deypay_application::registration::valid_email;
use deypay_common::{
    config::Environment,
    email::{enabled, number, scope},
};
use hmac::{Hmac, Mac};
use reqwest::Url;
use sha2::{Digest, Sha256};
use sqlx::{FromRow, PgPool};
use std::{sync::Arc, time::Duration};
use uuid::Uuid;
use zeroize::Zeroizing;

pub struct VerificationWorker {
    delivery: Arc<dyn EmailDelivery>,
    secret: Zeroizing<[u8; 32]>,
    base_url: Url,
    scope: &'static str,
    ttl: i32,
    tries: i32,
    lease: i32,
    backoff: i32,
}

impl VerificationWorker {
    pub fn from_env(env: Environment) -> Result<Option<Self>, &'static str> {
        if !enabled()? {
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
        let secret = Zeroizing::new(
            std::env::var("EMAIL_VERIFY_TOKEN_KEY")
                .map_err(|_| "Missing EMAIL_VERIFY_TOKEN_KEY")?,
        );
        let mut key = Zeroizing::new([0u8; 32]);
        hex::decode_to_slice(secret.as_bytes(), key.as_mut())
            .map_err(|_| "Invalid EMAIL_VERIFY_TOKEN_KEY")?;
        let url =
            std::env::var("EMAIL_VERIFY_BASE_URL").map_err(|_| "Missing EMAIL_VERIFY_BASE_URL")?;
        Ok(Some(Self {
            delivery,
            secret: key,
            base_url: confirmation_url(&url, local)?,
            scope: scope(env),
            ttl: number("EMAIL_VERIFY_TTL_SECONDS", 900, 60, 3600)?,
            tries: number("EMAIL_MAX_ATTEMPTS", 5, 1, 20)?,
            lease: timeout + 15,
            backoff: number("EMAIL_RETRY_BASE_SECONDS", 5, 1, 300)?,
        }))
    }

    // Explicit local harness. Hosted processes can select only the Resend adapter.
    pub fn sandbox(
        delivery: Arc<dyn EmailDelivery>,
        key: [u8; 32],
        url: &str,
    ) -> Result<Self, &'static str> {
        Ok(Self {
            delivery,
            secret: Zeroizing::new(key),
            base_url: confirmation_url(url, true)?,
            scope: "sandbox",
            ttl: 900,
            tries: 5,
            lease: 25,
            backoff: 5,
        })
    }

    pub async fn tick(&self, pool: &PgPool) -> sqlx::Result<bool> {
        sqlx::query("SELECT integrations.dispatch_verification_email($1)")
            .bind(self.tries)
            .execute(pool)
            .await?;
        let mut tx = pool.begin().await?;
        let claim=sqlx::query_as::<_,Claim>("SELECT *,to_char(to_timestamp(expiry) AT TIME ZONE 'UTC','YYYY-MM-DD HH24:MI:SS') AS display_expiry FROM integrations.claim_verification_email($1,$2,$3)")
            .bind(self.scope).bind(self.ttl).bind(self.lease).fetch_optional(&mut *tx).await?;
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
        let mut mac =
            Hmac::<Sha256>::new_from_slice(self.secret.as_slice()).expect("fixed HMAC key");
        mac.update(b"DEYPAY/email-verification/v1\0");
        mac.update(claim.message_id.as_bytes());
        mac.update(claim.customer.as_bytes());
        mac.update(claim.address.as_bytes());
        let secret: [u8; 32] = mac.finalize().into_bytes().into();
        let digest = deypay_application::email_verification::EmailProof::digest(&secret);
        let mut link = self.base_url.clone();
        link.set_fragment(Some(&format!(
            "{}.{}",
            claim.message_id,
            hex::encode(secret)
        )));
        let message = verification_message(&claim.address, &link, &claim.display_expiry);
        let bytes = serde_json::to_vec(&message).expect("string-only email message");
        let fingerprint = hex::encode(Sha256::digest(bytes));
        let ready = sqlx::query_scalar::<_, bool>(
            "SELECT integrations.prepare_verification_email($1,$2,$3,$4)",
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
            "SELECT integrations.finish_verification_email($1,$2,$3,$4,$5)",
        )
        .bind(claim.message_id)
        .bind(claim.lease)
        .bind(outcome)
        .bind(delay)
        .bind(receipt)
        .fetch_one(pool)
        .await?;
        if updated {
            tracing::info!(message_id=%claim.message_id,attempt=claim.attempt,outcome,"Verification email delivery");
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

fn confirmation_url(value: &str, local: bool) -> Result<Url, &'static str> {
    let url = Url::parse(value).map_err(|_| "Invalid EMAIL_VERIFY_BASE_URL")?;
    let loopback = matches!(url.host_str(), Some("localhost" | "127.0.0.1" | "[::1]"));
    if url.host().is_none()
        || !(url.scheme() == "https" || local && loopback && url.scheme() == "http")
        || !url.username().is_empty()
        || url.password().is_some()
        || url.query().is_some()
        || url.fragment().is_some()
        || url.path() != "/verify-email"
    {
        return Err("Invalid EMAIL_VERIFY_BASE_URL");
    }
    Ok(url)
}

#[test]
fn only_the_configured_confirmation_page_can_receive_links() {
    assert!(confirmation_url("https://app.example.test/verify-email", false).is_ok());
    assert!(confirmation_url("http://127.0.0.1:8000/verify-email", true).is_ok());
    for url in [
        "http://app.example.test/verify-email",
        "http://localhost/verify-email",
        "https://user:pass@app.example.test/verify-email",
        "https://app.example.test/elsewhere",
        "https://app.example.test/verify-email?redirect=other",
        "https://app.example.test/verify-email#old",
    ] {
        assert!(confirmation_url(url, false).is_err());
    }
}

pub async fn admit(pool: &PgPool, op: &str, cap: i32) -> sqlx::Result<bool> {
    sqlx::query_scalar("SELECT identity.admit_email_request($1,$2)")
        .bind(op)
        .bind(cap)
        .fetch_one(pool)
        .await
}
pub async fn resend(
    pool: &PgPool,
    address: &str,
    scope: &str,
    cooldown: i32,
    hourly: i32,
) -> sqlx::Result<()> {
    sqlx::query("SELECT identity.request_verification_email($1,$2,$3,$4)")
        .bind(address)
        .bind(scope)
        .bind(cooldown)
        .bind(hourly)
        .execute(pool)
        .await?;
    Ok(())
}
pub async fn consume(
    pool: &PgPool,
    id: Uuid,
    digest: &str,
    scope: &str,
    attempts: i32,
) -> sqlx::Result<bool> {
    sqlx::query_scalar("SELECT identity.consume_verification_email($1,$2,$3,$4)")
        .bind(id)
        .bind(digest)
        .bind(scope)
        .bind(attempts)
        .fetch_one(pool)
        .await
}
