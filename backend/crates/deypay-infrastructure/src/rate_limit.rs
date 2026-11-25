//! Call deployment admission before parsing, then subject admission before any
//! business lookup. Forgot-password subject exhaustion gets the generic acknowledgement.
use deypay_application::{registration::valid_email, usernames::handle};
use sqlx::PgPool;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AdmissionError {
    Limited,
    Unavailable,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AuthOperation {
    ForgotPassword,
    ResetPassword,
    UsernameAvailability,
}

impl AuthOperation {
    fn name(self) -> &'static str {
        match self {
            Self::ForgotPassword => "forgot_password",
            Self::ResetPassword => "reset_password",
            Self::UsernameAvailability => "username_availability",
        }
    }

    fn subject(self, raw: &str) -> Option<String> {
        if raw.is_empty() || raw.len() > 512 {
            return None;
        }
        match self {
            // The database applies the same lower(btrim(...)) as credential lookup.
            Self::ForgotPassword => valid_email(raw.trim()).then(|| raw.trim().to_owned()),
            Self::UsernameAvailability => handle(raw).map(str::to_ascii_lowercase),
            // A public selector is opaque; never accept a complete selector.secret token.
            Self::ResetPassword => {
                let valid = raw.bytes().all(|b| b.is_ascii_graphic() && b != b'.');
                valid.then(|| raw.to_owned())
            }
        }
    }
}

pub async fn admit(pool: &PgPool, operation: AuthOperation) -> Result<(), AdmissionError> {
    charge(pool, operation, None).await
}

pub async fn admit_subject(
    pool: &PgPool,
    operation: AuthOperation,
    subject: &str,
) -> Result<(), AdmissionError> {
    let canonical = operation
        .subject(subject)
        .ok_or(AdmissionError::Unavailable)?;
    charge(pool, operation, Some(&canonical)).await
}

async fn charge(
    pool: &PgPool,
    operation: AuthOperation,
    subject: Option<&str>,
) -> Result<(), AdmissionError> {
    match sqlx::query_scalar("SELECT identity.admit_public_request($1,$2)")
        .bind(operation.name())
        .bind(subject)
        .fetch_one(pool)
        .await
    {
        Ok(true) => Ok(()),
        Ok(false) => Err(AdmissionError::Limited),
        Err(_) => Err(AdmissionError::Unavailable),
    }
}

#[cfg(test)]
mod tests {
    use super::AuthOperation::*;

    #[test]
    fn bounds_and_normalizes_subjects_before_admission() {
        assert_eq!(UsernameAvailability.subject("@AdA").as_deref(), Some("ada"));
        assert!(UsernameAvailability.subject("a..b").is_none());
        assert!(ForgotPassword.subject(&"x".repeat(513)).is_none());
        assert!(ForgotPassword.subject("missing-at").is_none());
        assert!(ResetPassword.subject("selector.secret").is_none());
        assert!(ResetPassword.subject("").is_none());
        assert_eq!(
            ResetPassword.subject("Case_Sensitive").as_deref(),
            Some("Case_Sensitive")
        );
    }
}
