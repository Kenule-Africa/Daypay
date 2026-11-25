use deypay_application::{
    login::{LoginCredential, check_password},
    passwords::{CredentialError, CredentialVerifier, PasswordHasher, VerifiedPassword},
};
use deypay_infrastructure::password_hashing::Argon2idHasher;
use uuid::Uuid;

fn candidate(hash: String) -> LoginCredential {
    LoginCredential {
        customer_id: Uuid::from_u128(1),
        password_hash: hash,
    }
}

#[tokio::test]
async fn password_check_binds_the_checked_record_and_rejects_unusable_credentials() {
    let verifier = Argon2idHasher::new(32, 2, 1, 1).unwrap();
    let stored = verifier.hash("Synthetic1!").await.unwrap();
    let matched = check_password(&verifier, Some(candidate(stored.clone())), "Synthetic1!")
        .await
        .unwrap();
    assert_eq!(matched.snapshot().customer_id, Uuid::from_u128(1));
    assert!(matched.snapshot().password_hash == stored);
    for credential in [
        None,
        Some(candidate(stored.clone())),
        Some(candidate("corrupt".into())),
        Some(candidate(stored.replace("m=32", "m=4294967295"))),
    ] {
        assert!(matches!(
            check_password(&verifier, credential, "Different1!").await,
            Err(CredentialError::Rejected)
        ));
    }
}

struct FixedVerifier(Result<VerifiedPassword, CredentialError>);
impl CredentialVerifier for FixedVerifier {
    async fn verify(&self, _: &str, _: Option<&str>) -> Result<VerifiedPassword, CredentialError> {
        self.0
    }
}

#[tokio::test]
async fn no_verifier_outcome_can_manufacture_a_customer() {
    let yes = FixedVerifier(Ok(VerifiedPassword {
        needs_rehash: false,
    }));
    assert!(matches!(
        check_password(&yes, None, "fixture").await,
        Err(CredentialError::Rejected)
    ));
    for error in [CredentialError::Busy, CredentialError::Unavailable] {
        assert!(
            matches!(check_password(&FixedVerifier(Err(error)), Some(candidate("unused".into())), "fixture").await, Err(actual) if actual == error)
        );
    }
}
