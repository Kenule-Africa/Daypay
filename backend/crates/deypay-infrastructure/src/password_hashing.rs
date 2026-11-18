use argon2::{
    Algorithm, Argon2, Params, Version,
    password_hash::{PasswordHasher as _, SaltString},
};
use deypay_application::{passwords::PasswordHasher, registration::RegistrationError};
use std::sync::Arc;
use tokio::sync::Semaphore;
use zeroize::Zeroizing;

/// Costs and concurrency must be selected together for the deployment's memory budget.
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
}

impl PasswordHasher for Argon2idHasher {
    async fn hash(&self, password: &str) -> Result<String, RegistrationError> {
        let permit = self
            .slots
            .clone()
            .try_acquire_owned()
            .map_err(|_| RegistrationError::Busy)?;
        let secret = Zeroizing::new(password.to_owned());
        let params = self.params.clone();
        tokio::task::spawn_blocking(move || {
            // Keep the permit while hashing, even if the request is cancelled.
            let _permit = permit;
            let mut bytes = [0u8; 16];
            getrandom::getrandom(&mut bytes).map_err(|_| RegistrationError::Unavailable)?;
            let salt =
                SaltString::encode_b64(&bytes).map_err(|_| RegistrationError::Unavailable)?;
            Argon2::new(Algorithm::Argon2id, Version::V0x13, params)
                .hash_password(secret.as_bytes(), &salt)
                .map(|hash| hash.to_string())
                .map_err(|_| RegistrationError::Unavailable)
        })
        .await
        .map_err(|_| RegistrationError::Unavailable)?
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use argon2::password_hash::{PasswordHash, PasswordVerifier};

    #[tokio::test]
    async fn salts_are_fresh_costs_are_configured_and_long_secrets_are_not_truncated() {
        // Deliberately small test-only costs.
        let hasher = Argon2idHasher::new(32, 2, 1, 1).unwrap();
        let secret = format!("A1!{}", "x".repeat(160));
        let one = hasher.hash(&secret).await.unwrap();
        let two = hasher.hash(&secret).await.unwrap();
        assert_ne!(one, two);
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
        assert_eq!(hasher.hash(&secret).await, Err(RegistrationError::Busy));
        assert!(Argon2idHasher::new(1, 0, 0, 0).is_err());
    }
}
