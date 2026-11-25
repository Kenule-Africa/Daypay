use axum::{
    body::{Body, to_bytes},
    http::{Request, StatusCode, header},
};
use deypay_api::registration::{RegistrationSettings, routes};
use deypay_application::{
    passwords::{
        CompromisedPasswords, CredentialError, CredentialVerifier, PasswordPolicy,
        ScreenedPasswords, VerifiedPassword,
    },
    registration::RegistrationError,
};
use deypay_common::config::Environment;
use deypay_infrastructure::{MIGRATOR, password_hashing::Argon2idHasher};
use sqlx::PgPool;
use std::sync::Arc;
use tower::ServiceExt;

#[path = "cases/login_sessions.rs"]
mod login_sessions;

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
        passwords: Arc::new(ScreenedPasswords {
            policy: PasswordPolicy::PROVISIONAL,
            screening: FixtureScreen(compromised),
            hasher: Argon2idHasher::new(32, 2, 1, 1).unwrap(),
        }),
    }
}
const SANDBOX: &str = include_str!("../../../../infrastructure/postgres/registration-sandbox.psql");

#[sqlx::test(migrator = "MIGRATOR")]
async fn executable_accepts_sandbox_requests_and_refuses_production(
    pool: PgPool,
) -> sqlx::Result<()> {
    for script in [
        include_str!("../../../../infrastructure/postgres/phase-two-grants.psql"),
        include_str!("../../../../infrastructure/postgres/registration-grants.psql"),
        include_str!("../../../../infrastructure/postgres/email-verification-api-grants.psql"),
        SANDBOX,
    ] {
        sqlx::raw_sql(script).execute(&pool).await?;
    }
    login_sessions::auth_fixture(&pool, 200, 100).await?;
    let name: String = sqlx::query_scalar("SELECT current_database()")
        .fetch_one(&pool)
        .await?;
    let script = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../../scripts/registration-sandbox-smoke.mjs");
    let status = tokio::task::spawn_blocking(move || {
        std::process::Command::new("node")
            .arg(script)
            .arg(env!("CARGO_BIN_EXE_deypay-api"))
            .arg(name)
            .arg(env!("CARGO_BIN_EXE_deypay-auth"))
            .status()
    })
    .await
    .unwrap()
    .unwrap();
    assert!(status.success());
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT count(*) FROM customers.registration_consent_evidence"
        )
        .fetch_one(&pool)
        .await?,
        1
    );
    Ok(())
}
fn submission(email: &str, username: &str) -> serde_json::Value {
    serde_json::json!({"first_name":"Ada","last_name":"Okafor","email":email,"phone":"+2348012345678",
        "country_code":"NG","username":username,"password":"A1!xxx","terms_version":"1.0","privacy_version":"1.0",
        "accept_terms":true,"acknowledge_privacy_notice":true})
}
fn post(body: serde_json::Value) -> Request<Body> {
    Request::post("/api/v1/auth/register")
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(body.to_string()))
        .unwrap()
}
async fn app(
    pool: &PgPool,
    attempts: u32,
    compromised: Result<bool, RegistrationError>,
) -> axum::Router {
    sqlx::raw_sql(SANDBOX).execute(pool).await.unwrap();
    routes(Environment::Test, Some(settings(attempts, compromised))).with_state(pool.clone())
}
async fn get(app: &axum::Router, path: &str) -> axum::response::Response {
    app.clone()
        .oneshot(Request::get(path).body(Body::empty()).unwrap())
        .await
        .unwrap()
}
async fn bytes(response: axum::response::Response) -> axum::body::Bytes {
    to_bytes(response.into_body(), 1024 * 1024).await.unwrap()
}

#[sqlx::test(migrator = "MIGRATOR")]
async fn public_signup_is_unverified_and_non_enumerating(pool: PgPool) -> sqlx::Result<()> {
    let router = app(&pool, 5, Ok(false)).await;
    sqlx::query("INSERT INTO customers.username_keys VALUES ('reserved_fixture',true)")
        .execute(&pool)
        .await?;
    for (email, name) in [
        ("one@example.test", "@Mixed"),
        ("one@example.test", "other"),
        ("two@example.test", "mixed"),
        ("three@example.test", "@Reserved_Fixture"),
    ] {
        let response = router
            .clone()
            .oneshot(post(submission(email, name)))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::ACCEPTED);
        assert_eq!(response.headers()[header::CACHE_CONTROL], "no-store");
        assert!(!response.headers().contains_key(header::SET_COOKIE));
        assert_eq!(bytes(response).await.as_ref(), br#"{"data":{}}"#);
    }
    let (hash,status,first,last,country):(String,String,String,String,String)=sqlx::query_as(
        "SELECT password_hash,status,first_name,last_name,country_code FROM identity.users u
        JOIN identity.password_credentials pw ON pw.user_id=u.id JOIN customers.profiles p ON p.user_id=u.id"
    ).fetch_one(&pool).await?;
    assert!(hash.starts_with("$argon2id$v=19$m=32,t=2,p=1$"));
    let verifier = Argon2idHasher::new(64, 2, 1, 1).unwrap();
    assert_eq!(
        verifier.verify("A1!xxx", Some(&hash)).await,
        Ok(VerifiedPassword { needs_rehash: true })
    );
    assert_eq!(
        verifier.verify("A1!xxy", Some(&hash)).await,
        Err(CredentialError::Rejected)
    );
    assert_eq!(
        (
            status.as_str(),
            first.as_str(),
            last.as_str(),
            country.as_str()
        ),
        ("CONTACT_UNVERIFIED", "Ada", "Okafor", "NG")
    );
    assert!(
        sqlx::query_scalar::<_, bool>(
            "SELECT phone_e164='+2348012345678' AND phone_verified_at IS NULL
             AND ARRAY(SELECT event_type FROM integrations.outbox_events WHERE aggregate_id=u.id)
                 = ARRAY['EMAIL_VERIFICATION_REQUESTED'] FROM identity.users u",
        )
        .fetch_one(&pool)
        .await?
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT count(*) FROM customers.registration_consent_evidence"
        )
        .fetch_one(&pool)
        .await?,
        1
    );
    Ok(())
}
#[sqlx::test(migrator = "MIGRATOR")]
async fn unapproved_username_formats_are_validation_failures(pool: PgPool) -> sqlx::Result<()> {
    let router = app(&pool, 32, Ok(false)).await;
    // Incompressible: on main this reached the index and returned 503.
    let oversized: String = (0..200u32)
        .map(|i| format!("{:016x}", u64::from(i).wrapping_mul(0x9e37_79b9_7f4a_7c15)))
        .collect();
    for (i, name) in [
        "mich\u{430}el",
        "rex\u{200b}zy",
        "jose\u{301}",
        "\u{202e}yzxer",
        "ab",
        "ada-okafor",
        oversized.as_str(),
    ]
    .into_iter()
    .enumerate()
    {
        let response = router
            .clone()
            .oneshot(post(submission(&format!("r{i}@example.test"), name)))
            .await
            .unwrap();
        assert_eq!(
            response.status(),
            StatusCode::UNPROCESSABLE_ENTITY,
            "{name:?}"
        );
        assert_eq!(response.headers()[header::CACHE_CONTROL], "no-store");
        assert_eq!(
            bytes(response).await.as_ref(),
            br#"{"error":{"code":"VALIDATION_ERROR","message":"Invalid registration request","details":{}}}"#
        );
    }
    let users: i64 = sqlx::query_scalar("SELECT count(*) FROM identity.users")
        .fetch_one(&pool)
        .await?;
    assert_eq!(users, 0);
    Ok(())
}
#[sqlx::test(migrator = "MIGRATOR")]
async fn required_fields_and_acknowledgements_are_enforced(pool: PgPool) -> sqlx::Result<()> {
    let router = app(&pool, 32, Ok(false)).await;
    let original = submission("ada@example.test", "ada");
    for field in [
        "first_name",
        "last_name",
        "country_code",
        "phone",
        "accept_terms",
        "acknowledge_privacy_notice",
        "terms_version",
        "privacy_version",
    ] {
        let mut body = original.clone();
        body.as_object_mut().unwrap().remove(field);
        assert_eq!(
            router.clone().oneshot(post(body)).await.unwrap().status(),
            StatusCode::BAD_REQUEST,
            "{field}"
        );
    }
    for (field, val) in [
        ("accept_terms", false.into()),
        ("acknowledge_privacy_notice", false.into()),
        ("terms_version", "0.9".into()),
        ("privacy_version", "future".into()),
        ("first_name", " ".into()),
        ("last_name", "".into()),
        ("country_code", "ng".into()),
        ("phone", "08012345678".into()),
        ("password", "weak".into()),
    ] {
        let mut body = original.clone();
        body[field] = val;
        let response = router.clone().oneshot(post(body)).await.unwrap();
        assert_eq!(
            response.status(),
            StatusCode::UNPROCESSABLE_ENTITY,
            "{field}"
        );
        let body = bytes(response).await;
        for secret in ["ada@example.test", "weak", "A1!xxx"] {
            assert!(!std::str::from_utf8(&body).unwrap().contains(secret));
        }
    }
    for field in ["accepted_at", "accepted_at_unix", "channel", "customer_id"] {
        let mut body = original.clone();
        body[field] = "forged".into();
        assert_eq!(
            router.clone().oneshot(post(body)).await.unwrap().status(),
            StatusCode::BAD_REQUEST
        );
    }
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM identity.users")
            .fetch_one(&pool)
            .await?,
        0
    );
    Ok(())
}
#[sqlx::test(migrator = "MIGRATOR")]
async fn registration_activation_screening_and_abuse_gates(pool: PgPool) -> sqlx::Result<()> {
    let request = || post(submission("ada@example.test", "ada"));
    let inactive =
        routes(Environment::Local, Some(settings(5, Ok(false)))).with_state(pool.clone());
    assert_eq!(
        inactive.oneshot(request()).await.unwrap().status(),
        StatusCode::SERVICE_UNAVAILABLE
    );
    for (verdict, status) in [
        (Ok(true), StatusCode::UNPROCESSABLE_ENTITY),
        (
            Err(RegistrationError::Unavailable),
            StatusCode::SERVICE_UNAVAILABLE,
        ),
    ] {
        let enabled = app(&pool, 1, verdict).await;
        assert_eq!(
            enabled.clone().oneshot(request()).await.unwrap().status(),
            status
        );
        assert_eq!(
            enabled.oneshot(request()).await.unwrap().status(),
            StatusCode::TOO_MANY_REQUESTS
        );
    }
    let production =
        routes(Environment::Production, Some(settings(5, Ok(false)))).with_state(pool.clone());
    struct UnavailableHasher;
    impl deypay_application::passwords::PasswordHasher for UnavailableHasher {
        async fn hash(&self, _: &str) -> Result<String, RegistrationError> {
            Err(RegistrationError::Unavailable)
        }
    }
    let mut config = settings(1, Ok(false));
    config.passwords = Arc::new(ScreenedPasswords {
        policy: PasswordPolicy::PROVISIONAL,
        screening: FixtureScreen(Ok(false)),
        hasher: UnavailableHasher,
    });
    let failed_hash = routes(Environment::Test, Some(config)).with_state(pool.clone());
    assert_eq!(
        failed_hash.oneshot(request()).await.unwrap().status(),
        StatusCode::SERVICE_UNAVAILABLE
    );
    assert_eq!(
        production
            .clone()
            .oneshot(request())
            .await
            .unwrap()
            .status(),
        StatusCode::SERVICE_UNAVAILABLE
    );
    let list: serde_json::Value =
        serde_json::from_slice(&bytes(get(&production, "/api/v1/legal/documents").await).await)
            .unwrap();
    assert_eq!(list["data"]["documents"], serde_json::json!([]));
    assert_eq!(
        get(&production, "/api/v1/legal/documents/DEP-LEG-001/1.0")
            .await
            .status(),
        StatusCode::NOT_FOUND
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM identity.users")
            .fetch_one(&pool)
            .await?,
        0
    );
    Ok(())
}

#[sqlx::test(migrator = "MIGRATOR")]
async fn publication_requires_final_revisions_and_does_not_open_production_registration(
    pool: PgPool,
) -> sqlx::Result<()> {
    // Entirely synthetic final versions on this disposable database.
    sqlx::raw_sql("INSERT INTO customers.legal_document_revisions(document_id,version,edition,title,content)
        SELECT id,'test-final','final',id,convert_to('Synthetic publication only','UTF8') FROM
        (VALUES ('DEP-LEG-001'),('DEP-LEG-002'),('DEP-LEG-003')) d(id);
        INSERT INTO customers.registration_legal_releases(scope,terms_version,privacy_version,wording_version,approval_reference,published_at)
        VALUES ('published','test-final','test-final','test-final','synthetic-test-reference',now());
        INSERT INTO customers.current_registration_release SELECT scope,id FROM customers.registration_legal_releases WHERE scope='published'").execute(&pool).await?;
    let router = routes(Environment::Production, Some(settings(5, Ok(false)))).with_state(pool);
    let index: serde_json::Value =
        serde_json::from_slice(&bytes(get(&router, "/api/v1/legal/documents").await).await)
            .unwrap();
    assert_eq!(index["data"]["documents"].as_array().unwrap().len(), 3);
    for d in index["data"]["documents"].as_array().unwrap() {
        assert_eq!(d["status"], "published");
        assert_eq!(d["version"], "test-final");
    }
    assert_eq!(
        get(&router, "/api/v1/legal/documents/DEP-LEG-001/1.0")
            .await
            .status(),
        StatusCode::NOT_FOUND
    );
    assert_eq!(
        router
            .oneshot(post(submission("sandbox@example.test", "sandbox")))
            .await
            .unwrap()
            .status(),
        StatusCode::SERVICE_UNAVAILABLE
    );
    Ok(())
}

#[sqlx::test(migrator = "MIGRATOR")]
async fn catalog_links_serve_the_exact_immutable_legal_revisions(pool: PgPool) -> sqlx::Result<()> {
    let router = app(&pool, 5, Ok(false)).await;
    let response = get(&router, "/api/v1/legal/documents").await;
    assert_eq!(response.status(), StatusCode::OK);
    let catalog: serde_json::Value = serde_json::from_slice(&bytes(response).await).unwrap();
    assert_eq!(catalog["data"]["scope"], "sandbox");
    let documents = catalog["data"]["documents"].as_array().unwrap();
    assert_eq!(documents.len(), 3);
    let originals = [
        include_str!("../../../../docs/legal/DEYPAY TERMS OF SERVICE.md"),
        include_str!("../../../../docs/legal/DEYPAY PRIVACY NOTICE.md"),
        include_str!("../../../../docs/legal/DEYPAY CUSTOMER CONSENT & ACKNOWLEDGEMENTS.md"),
        include_str!("../../../../docs/legal/DEYPAY REGISTRATION CONSENT & ACCEPTANCE RECORD.md"),
    ];
    for (index, original) in originals.iter().enumerate() {
        let id = format!("DEP-LEG-00{}", index + 1);
        let source = original.replace("\r\n", "\n");
        let (content,digest):(Vec<u8>,String)=sqlx::query_as("SELECT content,sha256 FROM customers.legal_document_revisions WHERE document_id=$1 AND version='1.0'").bind(&id).fetch_one(&pool).await?;
        assert_eq!(content, source.as_bytes());
        if let Some(document) = documents.get(index) {
            assert_eq!(document["document_id"], id);
            assert_eq!(document["version"], "1.0");
            assert_eq!(document["sha256"], digest);
            assert_eq!(document["status"], "sandbox_draft");
            let response = get(&router, document["url"].as_str().unwrap()).await;
            assert_eq!(response.headers()["x-deypay-legal-status"], "sandbox_draft");
            assert_eq!(bytes(response).await.as_ref(), content.as_slice());
        }
    }
    assert_eq!(
        get(&router, "/api/v1/legal/documents/DEP-LEG-004/1.0")
            .await
            .status(),
        StatusCode::NOT_FOUND
    );
    Ok(())
}
