//! A matching password is one precondition for session issuance.
use crate::passwords::{CredentialError, CredentialVerifier};
use std::future::Future;
use uuid::Uuid;

/// Snapshot read by the credential repository. Never serialize or log this value.
pub struct LoginCredential {
    pub customer_id: Uuid,
    pub password_hash: String,
}

/// Carries the exact credential checked so issuance can detect concurrent
/// password changes. Only session persistence should consume this value.
pub struct CheckedCredential(LoginCredential);

impl CheckedCredential {
    pub fn snapshot(&self) -> &LoginCredential {
        &self.0
    }
}

pub async fn check_password(
    verifier: &impl CredentialVerifier,
    candidate: Option<LoginCredential>,
    password: &str,
) -> Result<CheckedCredential, CredentialError> {
    // Credential upgrades need a separate approved write policy.
    let _verified_password = verifier
        .verify(
            password,
            candidate.as_ref().map(|c| c.password_hash.as_str()),
        )
        .await?;
    candidate
        .map(CheckedCredential)
        .ok_or(CredentialError::Rejected)
}

/// Persistence must check the verified snapshot, email proof, account eligibility
/// and any prior cookie again inside the transaction that creates and audits the session.
/// Only digests of session secrets may cross this interface.
pub trait LoginStore: Send + Sync {
    fn credential(
        &self,
        email: &str,
    ) -> impl Future<Output = Result<Option<LoginCredential>, CredentialError>> + Send;
    fn record_rejection(
        &self,
        customer: Option<Uuid>,
    ) -> impl Future<Output = Result<(), CredentialError>> + Send;
    fn issue(
        &self,
        credential: CheckedCredential,
        email: &str,
        digest: &str,
        previous: Option<&str>,
        lifetime: SessionLifetime,
    ) -> impl Future<Output = Result<(), CredentialError>> + Send;
}

#[derive(Clone, Copy)]
pub struct SessionLifetime {
    idle_seconds: i32,
    absolute_seconds: i32,
}

impl SessionLifetime {
    pub fn new(idle_seconds: i32, absolute_seconds: i32) -> Option<Self> {
        (idle_seconds > 0 && absolute_seconds >= idle_seconds).then_some(Self {
            idle_seconds,
            absolute_seconds,
        })
    }
    pub fn idle_seconds(self) -> i32 {
        self.idle_seconds
    }
    pub fn absolute_seconds(self) -> i32 {
        self.absolute_seconds
    }
}

/// One atomic server-side operation checks the token, current customer status,
/// revocation and both deadlines. Refresh also replaces the old digest without
/// extending the original absolute deadline.
pub trait SessionStore: Send + Sync {
    fn validate(
        &self,
        current_digest: &str,
        replacement_digest: Option<&str>,
        idle_seconds: i32,
    ) -> impl Future<Output = Result<Option<Uuid>, CredentialError>> + Send;
}

pub async fn login(
    store: &impl LoginStore,
    verifier: &impl CredentialVerifier,
    email: &str,
    password: &str,
    digest: &str,
    previous_digest: Option<&str>,
    lifetime: SessionLifetime,
) -> Result<(), CredentialError> {
    let candidate = store.credential(email).await?;
    let customer = candidate.as_ref().map(|candidate| candidate.customer_id);
    let checked = match check_password(verifier, candidate, password).await {
        Err(CredentialError::Rejected) => {
            store.record_rejection(customer).await?;
            return Err(CredentialError::Rejected);
        }
        result => result?,
    };
    store
        .issue(checked, email, digest, previous_digest, lifetime)
        .await
}
