//! Registration has no authenticated result. Secrets are never part of a return value.
use std::{future::Future, pin::Pin};

pub struct Registration<'a> {
    pub email: &'a str,
    pub phone: &'a str,
    pub username: &'a str,
    pub first_name: &'a str,
    pub last_name: &'a str,
    pub country_code: &'a str,
    pub terms_version: &'a str,
    pub privacy_version: &'a str,
    /// Time the customer explicitly accepted both exact notice versions.
    pub accepted_at_unix: i64,
}

pub struct NoticeVersions<'a> {
    pub terms: &'a str,
    pub privacy: &'a str,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RegistrationError {
    InvalidInput,
    Busy,
    Unavailable,
}

pub type EnrollmentFuture<'a> =
    Pin<Box<dyn Future<Output = Result<String, RegistrationError>> + Send + 'a>>;

/// Apply the approved password policy and produce a salted Argon2id PHC string.
/// Only the trusted enrollment adapter receives the supplied password.
pub trait PasswordEnrollment: Sync {
    fn enroll<'a>(&'a self, password: &'a str) -> EnrollmentFuture<'a>;
}

pub trait RegistrationRepository: Sync {
    fn insert_unverified(
        &self,
        registration: &Registration<'_>,
        password_hash: &str,
    ) -> impl Future<Output = Result<(), RegistrationError>> + Send;
}

pub async fn register(
    repository: &impl RegistrationRepository,
    passwords: &dyn PasswordEnrollment,
    notices: &NoticeVersions<'_>,
    request: &Registration<'_>,
    password: &str,
) -> Result<(), RegistrationError> {
    request.validate(notices)?;
    if password.is_empty() {
        return Err(RegistrationError::InvalidInput);
    }
    // Apply the same password work before every valid attempt, including collisions.
    let hash = passwords.enroll(password).await?;
    if !hash.starts_with("$argon2id$") {
        return Err(RegistrationError::Unavailable);
    }
    repository.insert_unverified(request, &hash).await
}

fn valid_email(email: &str) -> bool {
    let Some((local, domain)) = email.split_once('@') else {
        return false;
    };
    email.len() <= 254
        && !local.is_empty()
        && local.len() <= 64
        && !local.starts_with('.')
        && !local.ends_with('.')
        && !local.contains("..")
        && local
            .chars()
            .all(|c| c.is_alphanumeric() || ".!#$%&'*+-/=?^_`{|}~".contains(c))
        && domain.split('.').all(|label| {
            !label.is_empty()
                && label.len() <= 63
                && !label.starts_with('-')
                && !label.ends_with('-')
                && label.chars().all(|c| c.is_alphanumeric() || c == '-')
        })
}

impl Registration<'_> {
    pub fn validate(&self, notices: &NoticeVersions<'_>) -> Result<(), RegistrationError> {
        let phone = self
            .phone
            .strip_prefix('+')
            .ok_or(RegistrationError::InvalidInput)?;
        let handle = self.username.strip_prefix('@').unwrap_or(self.username);
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_err(|_| RegistrationError::Unavailable)?
            .as_secs();
        if self.accepted_at_unix <= 0
            || self.accepted_at_unix as u64 > now
            || !valid_email(self.email.trim())
            || !(2..=15).contains(&phone.len())
            || phone.starts_with('0')
            || !phone.bytes().all(|b| b.is_ascii_digit())
            || handle.is_empty()
            || handle
                .chars()
                .any(|c| c == '@' || c.is_whitespace() || c.is_control())
            || [
                self.first_name,
                self.last_name,
                self.terms_version,
                self.privacy_version,
            ]
            .iter()
            .any(|s| s.contains('\0'))
            || self.first_name.trim().is_empty()
            || self.last_name.trim().is_empty()
            || self.country_code.len() != 2
            || !self.country_code.bytes().all(|b| b.is_ascii_uppercase())
            || notices.terms.trim().is_empty()
            || notices.privacy.trim().is_empty()
            || self.terms_version != notices.terms
            || self.privacy_version != notices.privacy
        {
            return Err(RegistrationError::InvalidInput);
        }
        Ok(())
    }
}
