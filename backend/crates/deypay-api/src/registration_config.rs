use crate::registration::RegistrationSettings;
use deypay_application::passwords::{PasswordPolicy, ScreenedPasswords};
use deypay_common::config::Environment;
use deypay_infrastructure::{
    password_hashing::Argon2idHasher, password_screening::LocalCompromisedPasswords,
};
use std::{
    env, fs,
    num::NonZeroU32,
    sync::{Arc, OnceLock},
    time::{Duration, UNIX_EPOCH},
};

impl RegistrationSettings {
    pub fn from_env(environment: Environment) -> Result<Option<Self>, &'static str> {
        if !matches!(environment, Environment::Local | Environment::Test) {
            return Ok(None);
        }
        match env::var("DEYPAY_REGISTRATION_MODE").as_deref() {
            Err(env::VarError::NotPresent) | Ok("off") => return Ok(None),
            Ok("sandbox") => {}
            _ => return Err("Invalid registration mode"),
        }
        let screening = screening_from_env()?;
        // Local sandbox defaults only. Production has its own outstanding security gates.
        let hasher = sandbox_hasher()?;
        let attempts_per_minute =
            NonZeroU32::new(number("DEYPAY_REGISTRATION_ATTEMPTS_PER_MINUTE", 30)?)
                .ok_or("Invalid sandbox registration limit")?;
        Ok(Some(Self {
            passwords: Arc::new(ScreenedPasswords {
                policy: PasswordPolicy::PROVISIONAL,
                screening,
                hasher,
            }),
            attempts_per_minute,
        }))
    }
}

/// The configured compromised-password dataset. Registration and the recovery
/// verifier (IMP-P3-008) screen new passwords against the same dataset.
pub(crate) fn screening_from_env() -> Result<LocalCompromisedPasswords, &'static str> {
    let path = env::var_os("DEYPAY_SCREENING_FILE").ok_or("Missing sandbox screening file")?;
    let data = fs::read(path).map_err(|_| "Cannot read sandbox screening file")?;
    let checksum =
        env::var("DEYPAY_SCREENING_SHA256").map_err(|_| "Missing sandbox screening checksum")?;
    let digest = parse_digest(&checksum).ok_or("Invalid sandbox screening checksum")?;
    let expiry = env::var("DEYPAY_SCREENING_VALID_UNTIL_UNIX")
        .ok()
        .and_then(|v| v.parse().ok())
        .and_then(|seconds| UNIX_EPOCH.checked_add(Duration::from_secs(seconds)))
        .ok_or("Invalid sandbox screening expiry")?;
    LocalCompromisedPasswords::load(&data, digest, expiry)
        .map_err(|_| "Invalid or expired sandbox screening dataset")
}

pub(crate) fn sandbox_hasher() -> Result<Argon2idHasher, &'static str> {
    // Both endpoints must use one process-wide work budget.
    static HASHER: OnceLock<Result<Argon2idHasher, &'static str>> = OnceLock::new();
    HASHER
        .get_or_init(|| {
            Argon2idHasher::new(
                number("DEYPAY_ARGON2_MEMORY_KIB", 19456)?,
                number("DEYPAY_ARGON2_ITERATIONS", 2)?,
                number("DEYPAY_ARGON2_LANES", 1)?,
                number("DEYPAY_HASH_CONCURRENCY", 2)? as usize,
            )
            .map_err(|_| "Invalid sandbox Argon2id profile")
        })
        .clone()
}

fn number(key: &str, default: u32) -> Result<u32, &'static str> {
    match env::var(key) {
        Ok(v) => v
            .parse()
            .map_err(|_| "Invalid sandbox registration setting"),
        Err(env::VarError::NotPresent) => Ok(default),
        Err(_) => Err("Invalid sandbox registration setting"),
    }
}
fn parse_digest(s: &str) -> Option<[u8; 32]> {
    if s.len() != 64 || !s.bytes().all(|b| b.is_ascii_hexdigit()) {
        return None;
    }
    let mut bytes = [0; 32];
    for (i, b) in bytes.iter_mut().enumerate() {
        *b = u8::from_str_radix(&s[i * 2..i * 2 + 2], 16).ok()?;
    }
    Some(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn hosted_environments_keep_registration_closed() {
        for env in [Environment::Staging, Environment::Production] {
            assert!(RegistrationSettings::from_env(env).unwrap().is_none());
        }
        assert!(parse_digest(&"g".repeat(64)).is_none());
    }
}
