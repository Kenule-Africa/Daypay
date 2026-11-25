use argon2::{
    Algorithm, Argon2, Params, Version,
    password_hash::{PasswordHash, PasswordHasher as _, PasswordVerifier as _, SaltString},
};
use deypay_application::{
    passwords::{
        CredentialError, CredentialVerifier, MAX_PASSWORD_BYTES, PasswordHasher, VerifiedPassword,
    },
    registration::RegistrationError,
};
use std::sync::Arc;
use tokio::sync::Semaphore;
use zeroize::Zeroizing;

/// Costs and concurrency must be selected together for the deployment's memory budget.
#[derive(Clone)]
pub struct Argon2idHasher {
    params: Params,
    slots: Arc<Semaphore>,
}
impl Argon2idHasher {
    pub fn new(
        memory_kib: u32,
        iterations: u32,
        lanes: u32,
        max_concurrent: usize,
    ) -> Result<Self, RegistrationError> {
        if max_concurrent == 0 || max_concurrent > Semaphore::MAX_PERMITS {
            return Err(RegistrationError::Unavailable);
        }
        let params = Params::new(memory_kib, iterations, lanes, None)
            .map_err(|_| RegistrationError::Unavailable)?;
        Ok(Self {
            params,
            slots: Arc::new(Semaphore::new(max_concurrent)),
        })
    }
    async fn with_slot<T: Send + 'static>(
        &self,
        password: &str,
        work: impl FnOnce(&[u8], Params) -> Result<T, CredentialError> + Send + 'static,
    ) -> Result<T, CredentialError> {
        let permit = self
            .slots
            .clone()
            .try_acquire_owned()
            .map_err(|_| CredentialError::Busy)?;
        let secret = Zeroizing::new(password.to_owned());
        let params = self.params.clone();
        tokio::task::spawn_blocking(move || {
            // Cancellation must not release capacity while Argon2 is still running.
            let _permit = permit;
            work(secret.as_bytes(), params)
        })
        .await
        .map_err(|_| CredentialError::Unavailable)?
    }
}

impl PasswordHasher for Argon2idHasher {
    async fn hash(&self, password: &str) -> Result<String, RegistrationError> {
        if password.is_empty() || password.len() > MAX_PASSWORD_BYTES {
            return Err(RegistrationError::InvalidInput);
        }
        self.with_slot(password, |secret, params| {
            let mut bytes = [0u8; 16];
            getrandom::getrandom(&mut bytes).map_err(|_| CredentialError::Unavailable)?;
            let salt = SaltString::encode_b64(&bytes).map_err(|_| CredentialError::Unavailable)?;
            Argon2::new(Algorithm::Argon2id, Version::V0x13, params)
                .hash_password(secret, &salt)
                .map(|hash| hash.to_string())
                .map_err(|_| CredentialError::Unavailable)
        })
        .await
        .map_err(|error| match error {
            CredentialError::Rejected => RegistrationError::InvalidInput,
            CredentialError::Busy => RegistrationError::Busy,
            CredentialError::Unavailable => RegistrationError::Unavailable,
        })
    }
}

/// Accept the format produced by DEYPAY enrollment. Stored records cannot
/// raise a single cost above the operator's configured per-request budget.
fn supported_hash<'a>(phc: &'a str, target: &Params) -> Option<(PasswordHash<'a>, bool)> {
    let hash = PasswordHash::new(phc).ok()?;
    if hash.algorithm != Algorithm::Argon2id.ident()
        || hash.version != Some(Version::V0x13.into())
        || hash.params.iter().count() != 3
        || hash.hash?.len() != Params::DEFAULT_OUTPUT_LEN
        || hash.salt?.decode_b64(&mut [0; 16]).ok()?.len() != 16
    {
        return None;
    }
    let memory = hash.params.get_decimal("m")?;
    let passes = hash.params.get_decimal("t")?;
    let lanes = hash.params.get_decimal("p")?;
    Params::new(memory, passes, lanes, None).ok()?;
    if memory > target.m_cost() || passes > target.t_cost() || lanes > target.p_cost() {
        return None;
    }
    let needs_rehash =
        (memory, passes, lanes) != (target.m_cost(), target.t_cost(), target.p_cost());
    Some((hash, needs_rehash))
}

impl CredentialVerifier for Argon2idHasher {
    async fn verify(
        &self,
        password: &str,
        stored_phc: Option<&str>,
    ) -> Result<VerifiedPassword, CredentialError> {
        if password.is_empty() || password.len() > MAX_PASSWORD_BYTES {
            return Err(CredentialError::Rejected);
        }
        // A v19 record with 3 u32 costs, a 16-byte salt and a 32-byte digest
        // fits comfortably. Bound the copy before parsing untrusted stored text.
        let phc = stored_phc
            .filter(|s| s.len() <= 128)
            .map(|s| Zeroizing::new(s.to_owned()));
        self.with_slot(password, move |secret, target| {
            let supported = phc.as_ref().and_then(|s| supported_hash(s, &target));
            let engine = Argon2::new(Algorithm::Argon2id, Version::V0x13, target);
            let Some((hash, needs_rehash)) = supported else {
                // A missing or unusable record consumes the current profile's
                // work. This synthetic salt and discarded output never enroll a user.
                let mut discarded = Zeroizing::new([0; Params::DEFAULT_OUTPUT_LEN]);
                engine
                    .hash_password_into(secret, b"deypay-dummy-salt", discarded.as_mut())
                    .map_err(|_| CredentialError::Unavailable)?;
                return Err(CredentialError::Rejected);
            };
            match engine.verify_password(secret, &hash) {
                Ok(()) => Ok(VerifiedPassword { needs_rehash }),
                Err(argon2::password_hash::Error::Password) => Err(CredentialError::Rejected),
                Err(_) => Err(CredentialError::Unavailable),
            }
        })
        .await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn salts_are_fresh_costs_are_configured_and_long_secrets_are_not_truncated() {
        // Deliberately small test-only costs.
        let hasher = Argon2idHasher::new(32, 2, 1, 1).unwrap();
        let secret = format!("A1!{}", "x".repeat(160));
        let one = hasher.hash(&secret).await.unwrap();
        let two = hasher.hash(&secret).await.unwrap();
        assert!(one != two);
        let parsed = PasswordHash::new(&one).unwrap();
        assert!(one.starts_with("$argon2id$v=19$m=32,t=2,p=1$"));
        assert!(
            Argon2::default()
                .verify_password(secret.as_bytes(), &parsed)
                .is_ok()
        );
        assert!(
            Argon2::default()
                .verify_password(&secret.as_bytes()[..128], &parsed)
                .is_err()
        );
        let _held = hasher.slots.acquire().await.unwrap();
        assert!(matches!(
            hasher.hash(&secret).await,
            Err(RegistrationError::Busy)
        ));
        assert!(Argon2idHasher::new(1, 0, 0, 0).is_err());
    }

    #[tokio::test]
    async fn verifies_full_password_and_signals_only_successful_upgrades() {
        let original = Argon2idHasher::new(32, 1, 1, 1).unwrap();
        let target = Argon2idHasher::new(64, 2, 2, 1).unwrap();
        let secret = "雪".repeat(MAX_PASSWORD_BYTES / 3) + "!";
        assert_eq!(secret.len(), MAX_PASSWORD_BYTES);
        let stored = original.hash(&secret).await.unwrap();
        assert_eq!(
            original.verify(&secret, Some(&stored)).await,
            Ok(VerifiedPassword {
                needs_rehash: false
            })
        );
        assert_eq!(
            target.verify(&secret, Some(&stored)).await,
            Ok(VerifiedPassword { needs_rehash: true })
        );
        for wrong in [&secret[..secret.len() - 1], "", &(secret.clone() + "!")] {
            assert_eq!(
                target.verify(wrong, Some(&stored)).await,
                Err(CredentialError::Rejected)
            );
        }
        assert!(matches!(
            original.hash(&(secret + "!")).await,
            Err(RegistrationError::InvalidInput)
        ));
    }

    #[tokio::test]
    async fn absent_corrupt_and_unsupported_records_fail_closed() {
        let hasher = Argon2idHasher::new(32, 2, 1, 1).unwrap();
        let candidate = "synthetic password";
        let stored = hasher.hash(candidate).await.unwrap();
        let (salted, digest) = stored.rsplit_once('$').unwrap();
        let (params, _) = salted.rsplit_once('$').unwrap();
        assert_eq!(
            hasher.verify(candidate, None).await,
            Err(CredentialError::Rejected)
        );
        for invalid in [
            "not a PHC record".to_owned(),
            "x".repeat(129),
            stored.replace("argon2id", "argon2i"),
            stored.replace("v=19", "v=42"),
            stored.replace("v=19$", ""),
            stored.replace("m=32,t=2,p=1", "m=32,m=32,t=2"),
            stored.replace("m=32", "m=4294967295"),
            stored.replace("t=2", "t=1000000"),
            stored.replace("p=1", "p=2"),
            stored.replace("m=32", "m=1"),
            stored.replace("t=2,", ""),
            stored.replace("p=1", "p=1,data=AAAA"),
            salted.to_owned(),
            format!("{params}$bad!salt${digest}"),
        ] {
            assert_eq!(
                hasher.verify(candidate, Some(&invalid)).await,
                Err(CredentialError::Rejected)
            );
        }
        let other = hasher.hash("different synthetic password").await.unwrap();
        assert_eq!(
            hasher.verify(candidate, Some(&other)).await,
            Err(CredentialError::Rejected)
        );
    }

    #[tokio::test]
    async fn cancelled_work_keeps_enrollment_and_verification_within_one_budget() {
        let hasher = Arc::new(Argon2idHasher::new(32, 2, 1, 1).unwrap());
        let stored = hasher.hash("fixture").await.unwrap();
        let (started, running) = tokio::sync::oneshot::channel();
        let (release, resume) = std::sync::mpsc::channel();
        let busy = hasher.clone();
        let caller = tokio::spawn(async move {
            busy.with_slot("fixture", move |_, _| {
                started.send(()).unwrap();
                resume.recv().unwrap();
                Ok(())
            })
            .await
        });
        running.await.unwrap();
        caller.abort();
        assert!(caller.await.unwrap_err().is_cancelled());
        for record in [Some(stored.as_str()), None] {
            assert_eq!(
                hasher.verify("fixture", record).await,
                Err(CredentialError::Busy)
            );
        }
        assert!(matches!(
            hasher.hash("fixture").await,
            Err(RegistrationError::Busy)
        ));
        release.send(()).unwrap();
        drop(hasher.slots.acquire().await.unwrap());
        assert_eq!(
            hasher.verify("fixture", Some(&stored)).await,
            Ok(VerifiedPassword {
                needs_rehash: false
            })
        );
    }
}
