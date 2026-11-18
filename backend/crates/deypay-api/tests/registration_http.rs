use axum::{
    body::{Body, to_bytes},
    http::{Request, StatusCode, header},
};
use deypay_api::registration::{AcceptedRegistration, RegistrationSettings, routes};
use deypay_application::{
    passwords::{CompromisedPasswords, PasswordPolicy, ScreenedPasswords},
    registration::RegistrationError,
};
use deypay_infrastructure::{MIGRATOR, password_hashing::Argon2idHasher};
use sqlx::PgPool;
use std::sync::Arc;
use tower::ServiceExt;

// Only the compromise verdict is a fixture. Hashing and persistence use the real adapters.
struct FixtureScreen(Result<bool, RegistrationError>);
impl CompromisedPasswords for FixtureScreen {
    fn contains(&self, _: &[u8; 32]) -> Result<bool, RegistrationError> {
        self.0
    }
}
fn settings(attempts: u32, compromised: Result<bool, RegistrationError>) -> RegistrationSettings {
    RegistrationSettings {
        attempts_per_minute: attempts.try_into().unwrap(),
        terms_version: "sandbox-terms".into(),
        privacy_version: "sandbox-privacy".into(),
        passwords: Arc::new(ScreenedPasswords {
            policy: PasswordPolicy::PROVISIONAL,
            screening: FixtureScreen(compromised),
            hasher: Argon2idHasher::new(32, 2, 1, 1).unwrap(),
        }),
    }
}
fn evidence(email: &str, username: &str) -> AcceptedRegistration {
    AcceptedRegistration {
        email: email.into(),
        username: username.into(),
        phone: "+2348012345678".into(),
        first_name: "Sandbox".into(),
        last_name: "Customer".into(),
        country_code: "NG".into(),
        terms_version: "sandbox-terms".into(),
        privacy_version: "sandbox-privacy".into(),
        accepted_at_unix: 1700000000,
    }
}
fn request(email: &str, username: &str, profile: Option<AcceptedRegistration>) -> Request<Body> {
    let body = serde_json::json!({"email":email,"username":username,"password":"A1!xxx","phone":"+2348012345678"});
    let mut request = Request::post("/api/v1/auth/register")
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(body.to_string()))
        .unwrap();
    if let Some(profile) = profile {
        request.extensions_mut().insert(Arc::new(profile));
    }
    request
}
#[sqlx::test(migrator = "MIGRATOR")]
async fn sandbox_signup_stores_a_real_configured_argon2id_hash(pool: PgPool) -> sqlx::Result<()> {
    let config = settings(1, Ok(false));
    let email = "fixture@example.test";
    let router = routes(Some(config)).with_state(pool.clone());
    let result = router
        .oneshot(request(email, "sandbox", Some(evidence(email, "sandbox"))))
        .await
        .unwrap();
    assert_eq!(result.status(), StatusCode::ACCEPTED);
    let hash: String =
        sqlx::query_scalar("SELECT password_hash FROM identity.password_credentials")
            .fetch_one(&pool)
            .await?;
    assert!(hash.starts_with("$argon2id$v=19$m=32,t=2,p=1$"));
    assert!(!hash.contains("A1!xxx"));
    Ok(())
}

#[sqlx::test(migrator = "MIGRATOR")]
async fn uniform_signup_responses_cover_replay_and_canonical_conflicts(
    pool: PgPool,
) -> sqlx::Result<()> {
    sqlx::query("INSERT INTO customers.username_keys VALUES ('reserved-fixture',true)")
        .execute(&pool)
        .await?;
    let router = routes(Some(settings(5, Ok(false)))).with_state(pool.clone());
    for (email, username) in [
        ("one@example.test", "@Mixed"),
        ("one@example.test", "other"),
        ("two@example.test", "mixed"),
        ("reserved@example.test", "@Reserved-Fixture"),
    ] {
        let response = router
            .clone()
            .oneshot(request(email, username, Some(evidence(email, username))))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::ACCEPTED);
        assert_eq!(response.headers()[header::CACHE_CONTROL], "no-store");
        assert!(!response.headers().contains_key(header::SET_COOKIE));
        assert_eq!(
            to_bytes(response.into_body(), 1024).await.unwrap().as_ref(),
            br#"{"data":{}}"#
        );
    }
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM identity.users")
            .fetch_one(&pool)
            .await?,
        1
    );
    Ok(())
}
#[sqlx::test(migrator = "MIGRATOR")]
async fn missing_evidence_policy_failures_and_abuse_create_nothing(
    pool: PgPool,
) -> sqlx::Result<()> {
    let email = "sandbox@example.test";
    let name = "name";
    let closed = routes(None).with_state(pool.clone());
    let response = closed
        .oneshot(request(email, name, Some(evidence(email, name))))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
    let unavailable =
        routes(Some(settings(1, Err(RegistrationError::Unavailable)))).with_state(pool.clone());
    assert_eq!(
        unavailable
            .oneshot(request(email, name, Some(evidence(email, name))))
            .await
            .unwrap()
            .status(),
        StatusCode::SERVICE_UNAVAILABLE
    );
    let router = routes(Some(settings(4, Ok(true)))).with_state(pool.clone());
    for (profile, status) in [
        (None, StatusCode::SERVICE_UNAVAILABLE),
        (
            Some(evidence("wrong@example.test", name)),
            StatusCode::UNPROCESSABLE_ENTITY,
        ),
        (
            Some(evidence(email, name)),
            StatusCode::UNPROCESSABLE_ENTITY,
        ),
    ] {
        let response = router
            .clone()
            .oneshot(request(email, name, profile))
            .await
            .unwrap();
        assert_eq!(response.status(), status);
        let bytes = to_bytes(response.into_body(), 1024).await.unwrap();
        let body = std::str::from_utf8(&bytes).unwrap();
        for secret in [email, "wrong@example.test", "A1!xxx", "sandbox-terms"] {
            assert!(!body.contains(secret));
        }
    }
    let mut malformed = request(email, name, Some(evidence(email, name)));
    *malformed.body_mut() = Body::from(
        r#"{"email":"x","phone":"x","password":"A1!xxx","username":"x","first_name":"client-asserted"}"#,
    );
    assert_eq!(
        router.clone().oneshot(malformed).await.unwrap().status(),
        StatusCode::BAD_REQUEST
    );
    assert_eq!(
        router
            .oneshot(request(email, name, None))
            .await
            .unwrap()
            .status(),
        StatusCode::TOO_MANY_REQUESTS
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM identity.users")
            .fetch_one(&pool)
            .await?,
        0
    );
    Ok(())
}
