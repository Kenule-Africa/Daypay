//! Password recovery (IMP-P3-008).
//!
//! Owner decisions of 2026-11-25: the recovery proof is a link sent to the
//! registered, already verified email address. Only accounts in the active
//! release's allowed login states receive one. A link is single use, expires
//! after a short lifetime and is superseded by a newer request. A successful
//! reset revokes every session and requires a fresh login (SEC-REC-004).
//! Requests never reveal whether an account exists (SEC-AUTH-008).
//!
//! The IMP-P3-012 admission contract applies: the caller charges the
//! deployment budget before reading the request; these use cases charge the
//! subject budget after validation and before any lookup, token check or
//! password work.

use crate::{
    email_verification::EmailProof,
    passwords::CredentialError,
    registration::{PasswordEnrollment, RegistrationError, valid_email},
};
use std::future::Future;
use uuid::Uuid;
use zeroize::Zeroizing;

/// The non-secret subject charged against the per-subject admission budget.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RecoverySubject<'a> {
    /// The canonical registration email of a forgot-password request.
    Email(&'a str),
    /// The public selector of a reset link, never its secret.
    Link(Uuid),
}

/// Recovery persistence. Implementations run in the isolated verifier and
/// receive an address, a link digest or an Argon2id hash; never a password
/// or a link secret.
pub trait RecoveryStore: Send + Sync {
    /// Charges the subject's admission budget, whether or not an account matches.
    /// `Busy` means the budget is exhausted.
    fn admit(
        &self,
        subject: RecoverySubject<'_>,
    ) -> impl Future<Output = Result<(), CredentialError>> + Send;
    /// Queue a reset email when the address belongs to an eligible account.
    /// The outcome is identical whether or not an email is queued.
    fn request(&self, email: &str) -> impl Future<Output = Result<(), CredentialError>> + Send;
    /// Redeem a link, replace the credential and revoke every session.
    /// `Ok(false)` covers unknown, expired, superseded, used and exhausted links.
    fn reset(
        &self,
        link: &EmailProof,
        password_hash: &str,
    ) -> impl Future<Output = Result<bool, CredentialError>> + Send;
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RecoveryError {
    /// The address is not a valid email address.
    InvalidRequest,
    /// The link is malformed, unknown, expired, superseded, used or exhausted.
    InvalidLink,
    /// The new password fails the password policy or compromised-password screening.
    PasswordRejected,
    Busy,
    Unavailable,
}

/// Starts recovery for `email`. Eligible or not, and within the subject budget
/// or not, the caller sees the same result.
pub async fn request_reset(store: &impl RecoveryStore, email: &str) -> Result<(), RecoveryError> {
    let email = email.trim();
    if !valid_email(email) {
        return Err(RecoveryError::InvalidRequest);
    }
    match store.admit(RecoverySubject::Email(email)).await {
        Ok(()) => {}
        // IMP-P3-012: an exhausted address budget gets the ordinary
        // acknowledgement and sends no mail.
        Err(CredentialError::Busy) => return Ok(()),
        Err(e) => return Err(unavailable(e)),
    }
    store.request(email).await.map_err(unavailable)
}

/// Replaces the password of the link's account. The new password passes the
/// registration policy and screening and is hashed before the store sees it.
pub async fn reset_password(
    store: &impl RecoveryStore,
    passwords: &dyn PasswordEnrollment,
    token: &str,
    password: &str,
) -> Result<(), RecoveryError> {
    let link = EmailProof::parse(token).ok_or(RecoveryError::InvalidLink)?;
    store
        .admit(RecoverySubject::Link(link.selector))
        .await
        .map_err(unavailable)?;
    let hash = Zeroizing::new(passwords.enroll(password).await.map_err(|e| match e {
        RegistrationError::InvalidInput => RecoveryError::PasswordRejected,
        RegistrationError::Busy => RecoveryError::Busy,
        RegistrationError::Unavailable => RecoveryError::Unavailable,
    })?);
    match store.reset(&link, &hash).await {
        Ok(true) => Ok(()),
        Ok(false) => Err(RecoveryError::InvalidLink),
        Err(e) => Err(unavailable(e)),
    }
}

fn unavailable(error: CredentialError) -> RecoveryError {
    match error {
        CredentialError::Busy => RecoveryError::Busy,
        CredentialError::Rejected | CredentialError::Unavailable => RecoveryError::Unavailable,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::registration::EnrollmentFuture;
    use std::pin::pin;
    use std::sync::Mutex;
    use std::task::{Context, Poll, Waker};

    /// The test doubles complete immediately, so one poll is enough.
    fn run<F: Future>(future: F) -> F::Output {
        match pin!(future).poll(&mut Context::from_waker(Waker::noop())) {
            Poll::Ready(output) => output,
            Poll::Pending => panic!("test doubles complete synchronously"),
        }
    }

    #[derive(Default)]
    struct Store {
        log: Mutex<Vec<String>>,
        admission: Option<CredentialError>,
        outcome: Option<Result<bool, CredentialError>>,
    }
    impl RecoveryStore for Store {
        async fn admit(&self, subject: RecoverySubject<'_>) -> Result<(), CredentialError> {
            self.log.lock().unwrap().push(format!("admit {subject:?}"));
            self.admission.map_or(Ok(()), Err)
        }
        async fn request(&self, email: &str) -> Result<(), CredentialError> {
            self.log.lock().unwrap().push(format!("request {email}"));
            Ok(())
        }
        async fn reset(&self, link: &EmailProof, hash: &str) -> Result<bool, CredentialError> {
            self.log
                .lock()
                .unwrap()
                .push(format!("reset {} {hash}", link.digest));
            self.outcome.unwrap_or(Ok(true))
        }
    }
    impl Store {
        fn log(&self) -> Vec<String> {
            self.log.lock().unwrap().clone()
        }
    }

    struct Enrollment(Result<&'static str, RegistrationError>, Mutex<usize>);
    impl PasswordEnrollment for Enrollment {
        fn enroll<'a>(&'a self, _: &'a str) -> EnrollmentFuture<'a> {
            *self.1.lock().unwrap() += 1;
            let result = self.0.map(str::to_owned);
            Box::pin(async move { result })
        }
    }
    fn enrollment(result: Result<&'static str, RegistrationError>) -> Enrollment {
        Enrollment(result, Mutex::new(0))
    }

    const SELECTOR: &str = "0b7f6a52-3d0e-4c39-9f5d-4c7a2b1e9d10";
    const TOKEN: &str = "0b7f6a52-3d0e-4c39-9f5d-4c7a2b1e9d10.\
        00112233445566778899aabbccddeeff00112233445566778899aabbccddeeff";

    #[test]
    fn requests_are_validated_then_charged_then_queued() {
        run(async {
            let store = Store::default();
            assert_eq!(
                request_reset(&store, "not-an-address").await,
                Err(RecoveryError::InvalidRequest)
            );
            assert!(store.log().is_empty());
            assert_eq!(request_reset(&store, "  ada@example.test ").await, Ok(()));
            assert_eq!(
                store.log(),
                [
                    "admit Email(\"ada@example.test\")",
                    "request ada@example.test"
                ]
            );
        })
    }

    #[test]
    fn an_exhausted_address_budget_is_acknowledged_without_mail() {
        run(async {
            let busy = Store {
                admission: Some(CredentialError::Busy),
                ..Store::default()
            };
            assert_eq!(request_reset(&busy, "ada@example.test").await, Ok(()));
            assert_eq!(busy.log(), ["admit Email(\"ada@example.test\")"]);
            let down = Store {
                admission: Some(CredentialError::Unavailable),
                ..Store::default()
            };
            assert_eq!(
                request_reset(&down, "ada@example.test").await,
                Err(RecoveryError::Unavailable)
            );
            assert_eq!(down.log().len(), 1);
        })
    }

    #[test]
    fn malformed_links_are_refused_before_admission_or_password_work() {
        run(async {
            let store = Store::default();
            let passwords = enrollment(Ok("$argon2id$hash"));
            for token in ["", "abc", "not-a-uuid.00", &TOKEN.replace('.', "")] {
                assert_eq!(
                    reset_password(&store, &passwords, token, "Secret1!").await,
                    Err(RecoveryError::InvalidLink)
                );
            }
            assert_eq!(*passwords.1.lock().unwrap(), 0);
            assert!(store.log().is_empty());
        })
    }

    #[test]
    fn the_selector_is_charged_before_any_password_work() {
        run(async {
            for (admission, expected) in [
                (CredentialError::Busy, RecoveryError::Busy),
                (CredentialError::Unavailable, RecoveryError::Unavailable),
            ] {
                let store = Store {
                    admission: Some(admission),
                    ..Store::default()
                };
                let passwords = enrollment(Ok("$argon2id$hash"));
                assert_eq!(
                    reset_password(&store, &passwords, TOKEN, "Secret1!").await,
                    Err(expected)
                );
                assert_eq!(store.log(), [format!("admit Link({SELECTOR})")]);
                assert_eq!(*passwords.1.lock().unwrap(), 0);
            }
        })
    }

    #[test]
    fn only_an_accepted_password_hash_reaches_the_store() {
        run(async {
            for (error, expected) in [
                (
                    RegistrationError::InvalidInput,
                    RecoveryError::PasswordRejected,
                ),
                (RegistrationError::Busy, RecoveryError::Busy),
                (RegistrationError::Unavailable, RecoveryError::Unavailable),
            ] {
                let store = Store::default();
                assert_eq!(
                    reset_password(&store, &enrollment(Err(error)), TOKEN, "weak").await,
                    Err(expected)
                );
                assert_eq!(store.log().len(), 1);
            }
            let store = Store::default();
            assert_eq!(
                reset_password(&store, &enrollment(Ok("$argon2id$new")), TOKEN, "Secret1!").await,
                Ok(())
            );
            let link = EmailProof::parse(TOKEN).unwrap();
            assert_eq!(
                store.log(),
                [
                    format!("admit Link({SELECTOR})"),
                    format!("reset {} $argon2id$new", link.digest)
                ]
            );
        })
    }

    #[test]
    fn refused_links_and_store_failures_are_reported_without_detail() {
        run(async {
            for (outcome, expected) in [
                (Ok(false), RecoveryError::InvalidLink),
                (Err(CredentialError::Busy), RecoveryError::Busy),
                (
                    Err(CredentialError::Unavailable),
                    RecoveryError::Unavailable,
                ),
                (Err(CredentialError::Rejected), RecoveryError::Unavailable),
            ] {
                let store = Store {
                    outcome: Some(outcome),
                    ..Store::default()
                };
                assert_eq!(
                    reset_password(&store, &enrollment(Ok("$argon2id$x")), TOKEN, "Secret1!").await,
                    Err(expected)
                );
            }
        })
    }
}
