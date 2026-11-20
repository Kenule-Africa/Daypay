use sha2::{Digest, Sha256};
use uuid::Uuid;

/// Public challenge selector and one-way verifier suitable for a repository call.
pub struct EmailProof {
    pub selector: Uuid,
    pub digest: String,
}

impl EmailProof {
    pub fn parse(token: &str) -> Option<Self> {
        let (selector, secret) = token.split_once('.')?;
        if selector.len() != 36 {
            return None;
        }
        let mut bytes = [0; 32];
        hex::decode_to_slice(secret, &mut bytes).ok()?;
        Some(Self {
            selector: Uuid::parse_str(selector).ok()?,
            digest: Self::digest(&bytes),
        })
    }

    pub fn digest(secret: &[u8; 32]) -> String {
        hex::encode(Sha256::digest(secret))
    }
}
