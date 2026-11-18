use crate::registration::{EnrollmentFuture, PasswordEnrollment, RegistrationError};
use sha2::{Digest, Sha256};
use std::future::Future;
use unicode_properties::{GeneralCategoryGroup, UnicodeGeneralCategory};

#[derive(Clone, Copy)]
pub struct PasswordPolicy {
    pub minimum_characters: usize,
    pub require_uppercase: bool,
    pub require_digit: bool,
    pub require_special: bool,
}

impl PasswordPolicy {
    /// Product-selected sandbox policy. Production security acceptance is pending.
    pub const PROVISIONAL: Self = Self {
        minimum_characters: 6,
        require_uppercase: true,
        require_digit: true,
        require_special: true,
    };

    pub fn accepts(&self, secret: &str) -> bool {
        self.minimum_characters > 0
            && secret.chars().count() >= self.minimum_characters
            && (!self.require_uppercase || secret.bytes().any(|b| b.is_ascii_uppercase()))
            && (!self.require_digit || secret.bytes().any(|b| b.is_ascii_digit()))
            && (!self.require_special
                || secret.chars().any(|c| {
                    matches!(
                        c.general_category_group(),
                        GeneralCategoryGroup::Punctuation | GeneralCategoryGroup::Symbol
                    )
                }))
    }
}

/// Local dataset adapters receive only a digest. Remote screening requires a separately reviewed adapter.
pub trait CompromisedPasswords: Sync {
    fn contains(&self, digest: &[u8; 32]) -> Result<bool, RegistrationError>;
}

pub trait PasswordHasher: Sync {
    fn hash(
        &self,
        password: &str,
    ) -> impl Future<Output = Result<String, RegistrationError>> + Send;
}

pub struct ScreenedPasswords<S, H> {
    pub policy: PasswordPolicy,
    pub screening: S,
    pub hasher: H,
}

impl<S: CompromisedPasswords, H: PasswordHasher> PasswordEnrollment for ScreenedPasswords<S, H> {
    fn enroll<'a>(&'a self, password: &'a str) -> EnrollmentFuture<'a> {
        Box::pin(async move {
            if !self.policy.accepts(password) {
                return Err(RegistrationError::InvalidInput);
            }
            let digest = Sha256::digest(password.as_bytes()).into();
            if self.screening.contains(&digest)? {
                return Err(RegistrationError::InvalidInput);
            }
            self.hasher.hash(password).await
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn provisional_boundaries_preserve_unicode_spaces_and_long_passwords() {
        let p = PasswordPolicy::PROVISIONAL;
        for password in [
            "A1!xx",
            "lowercase1!",
            "Uppercase!",
            "Uppercase1",
            "Α1!xxx",
            "A１!xxx",
            "A1xxxx\u{200b}",
        ] {
            assert!(!p.accepts(password));
        }
        for password in ["A1!XXX", "A1! 雪雪", "A1★XXX"] {
            assert!(p.accepts(password));
        }
        assert!(p.accepts(&format!("A1!{}", "雪".repeat(256))));
        assert!(
            !PasswordPolicy {
                minimum_characters: 7,
                ..p
            }
            .accepts("A1!XXX")
        );
    }
}
