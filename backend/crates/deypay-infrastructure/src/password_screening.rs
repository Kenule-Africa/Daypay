use deypay_application::{passwords::CompromisedPasswords, registration::RegistrationError};
use sha2::{Digest, Sha256};
use std::{collections::HashSet, time::SystemTime};

/// An operator supplies the trusted checksum and validity deadline separately from the dataset.
pub struct LocalCompromisedPasswords {
    hashes: HashSet<[u8; 32]>,
    valid_until: SystemTime,
}

impl LocalCompromisedPasswords {
    pub fn load(
        data: &[u8],
        expected_sha256: [u8; 32],
        valid_until: SystemTime,
    ) -> Result<Self, RegistrationError> {
        if Sha256::digest(data)[..] != expected_sha256 || valid_until <= SystemTime::now() {
            return Err(RegistrationError::Unavailable);
        }
        let text = std::str::from_utf8(data).map_err(|_| RegistrationError::Unavailable)?;
        let mut hashes = HashSet::new();
        for line in text.lines() {
            if line.len() != 64 || !line.bytes().all(|b| b.is_ascii_hexdigit()) {
                return Err(RegistrationError::Unavailable);
            }
            let mut digest = [0; 32];
            for (index, byte) in digest.iter_mut().enumerate() {
                *byte = u8::from_str_radix(&line[index * 2..index * 2 + 2], 16)
                    .map_err(|_| RegistrationError::Unavailable)?;
            }
            hashes.insert(digest);
        }
        if hashes.is_empty() {
            return Err(RegistrationError::Unavailable);
        }
        Ok(Self {
            hashes,
            valid_until,
        })
    }
}

impl CompromisedPasswords for LocalCompromisedPasswords {
    fn contains(&self, digest: &[u8; 32]) -> Result<bool, RegistrationError> {
        if SystemTime::now() >= self.valid_until {
            return Err(RegistrationError::Unavailable);
        }
        Ok(self.hashes.contains(digest))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{Duration, UNIX_EPOCH};

    #[test]
    fn dataset_requires_trusted_checksum_valid_rows_and_unexpired_release() {
        let until = SystemTime::now() + Duration::from_secs(3600);
        let digest: [u8; 32] = Sha256::digest(b"BlockedPassword1!").into();
        let data = format!("{:x}\n", Sha256::digest(b"BlockedPassword1!"));
        let checksum = Sha256::digest(data.as_bytes()).into();
        let mut screen = LocalCompromisedPasswords::load(data.as_bytes(), checksum, until).unwrap();
        assert_eq!(screen.contains(&digest), Ok(true));
        assert_eq!(screen.contains(&[0; 32]), Ok(false));
        for invalid in ["", "short", &"gg".repeat(32), &"+a".repeat(32)] {
            assert!(
                LocalCompromisedPasswords::load(
                    invalid.as_bytes(),
                    Sha256::digest(invalid).into(),
                    until
                )
                .is_err()
            );
        }
        assert!(LocalCompromisedPasswords::load(data.as_bytes(), [0; 32], until).is_err());
        assert!(LocalCompromisedPasswords::load(data.as_bytes(), checksum, UNIX_EPOCH).is_err());
        screen.valid_until = UNIX_EPOCH;
        assert_eq!(
            screen.contains(&digest),
            Err(RegistrationError::Unavailable)
        );
    }
}
