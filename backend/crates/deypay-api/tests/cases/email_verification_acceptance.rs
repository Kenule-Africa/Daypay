//! IMP-P3-002 acceptance: registration queues verification, the emailed proof
//! confirms the address and advances the account, and the customer can then
//! sign in through the verifier without KYC. Delivery uses the mock adapter;
//! real staging delivery through Resend is a separate, recorded blocker.
use super::login_sessions::{RELEASE, auth_fixture};
use axum::{
    Router,
    body::{Body, to_bytes},
    http::{Request, StatusCode, header},
    routing::get,
};
use deypay_api::{
    email_verification::{Policy, routes},
    login::{AuthenticatedCustomer, BrowserAuth},
    sessions::BrowserOrigin,
};
use deypay_application::passwords::PasswordHasher;
use deypay_infrastructure::{
    MIGRATOR, email_delivery::MockDelivery, email_verification::VerificationWorker,
    password_hashing::Argon2idHasher, sessions::PgSessions,
};
use serde_json::json;
use sqlx::{PgPool, postgres::PgPoolOptions};
use std::sync::Arc;
use tower::ServiceExt;
use uuid::Uuid;

const ORIGIN: &str = "https://sandbox.example.test";
const BASE: &str = "http://127.0.0.1:8000/verify-email";
const PW: &str = "Verified1!pass";

async fn role(pool: &PgPool, statement: &'static str) -> sqlx::Result<PgPool> {
    PgPoolOptions::new()
        .max_connections(4)
        .after_connect(move |conn, _| {
            Box::pin(async move {
                sqlx::query(statement).execute(conn).await?;
                Ok(())
            })
        })
        .connect_with((*pool.connect_options()).clone())
        .await
}

async fn post(
    app: &Router,
    path: &str,
    body: String,
    cookie: Option<&str>,
) -> axum::response::Response {
    let mut req = Request::post(path)
        .header(header::CONTENT_TYPE, "application/json")
        .header(header::ORIGIN, ORIGIN);
    if let Some(cookie) = cookie {
        req = req.header(header::COOKIE, cookie);
    }
    app.clone()
        .oneshot(req.body(Body::from(body)).unwrap())
        .await
        .unwrap()
}

async fn status_of(app: &Router, cookie: &str) -> (StatusCode, String) {
    let res = app
        .clone()
        .oneshot(
            Request::get("/test/status")
                .header(header::COOKIE, cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let status = res.status();
    let body = to_bytes(res.into_body(), 1024).await.unwrap();
    (status, String::from_utf8(body.to_vec()).unwrap())
}

#[sqlx::test(migrator = "MIGRATOR")]
async fn verified_customers_sign_in_without_kyc_and_unverified_ones_cannot(
    pool: PgPool,
) -> sqlx::Result<()> {
    for script in [
        include_str!("../../../../../infrastructure/postgres/phase-two-grants.psql"),
        include_str!("../../../../../infrastructure/postgres/email-verification-api-grants.psql"),
        super::SANDBOX,
    ] {
        sqlx::raw_sql(script).execute(&pool).await?;
    }
    auth_fixture(&pool, 200, 100).await?;
    let name: String = sqlx::query_scalar("SELECT current_database()")
        .fetch_one(&pool)
        .await?;
    let worker_grants = include_str!(
        "../../../../../infrastructure/postgres/email-verification-worker-grants.psql"
    )
    .replace(":\"DBNAME\"", &format!("\"{name}\""))
    .replace(":\"worker_role\"", "pg_read_all_settings");
    sqlx::raw_sql(&worker_grants).execute(&pool).await?;
    let api_db = role(&pool, "SET ROLE deypay_app").await?;
    let worker_db = role(&pool, "SET ROLE pg_read_all_settings").await?;
    let verifier_db = role(&pool, "SET ROLE deypay_auth").await?;
    let verification = routes(Some(Policy {
        scope: "sandbox",
        per_minute: 120,
        cooldown_seconds: 60,
        resends_per_hour: 3,
        guesses: 3,
    }))
    .with_state(api_db);
    let auth = BrowserAuth::new(
        PgSessions::new(verifier_db, RELEASE.parse().unwrap(), [0x37; 32].into()),
        Argon2idHasher::new(32, 2, 1, 4).unwrap(),
        BrowserOrigin::parse(ORIGIN).unwrap(),
    );
    async fn account_status(customer: AuthenticatedCustomer) -> String {
        customer.account_status().to_owned()
    }
    let signin = auth
        .routes()
        .merge(auth.protect(Router::new().route("/test/status", get(account_status))));
    let login = |name: &'static str| {
        let body = json!({"identifier": format!("{name}@example.test"), "password": PW});
        post(&signin, "/api/v1/auth/login", body.to_string(), None)
    };

    // Registration queues exactly one verification email per new customer.
    let hash = Argon2idHasher::new(32, 2, 1, 4)
        .unwrap()
        .hash(PW)
        .await
        .unwrap();
    for name in ["ada", "bob"] {
        sqlx::query("SELECT identity.register_customer($1,'+2348012345678',$2,$3,'Test','Customer','NG','1.0','1.0',true,true)")
            .bind(format!("{name}@example.test")).bind(name).bind(&hash).execute(&pool).await?;
    }
    let queued: Vec<(String, String)> = sqlx::query_as(
        "SELECT u.email,e.event_type FROM integrations.outbox_events e JOIN identity.users u ON u.id=e.aggregate_id ORDER BY u.email",
    )
    .fetch_all(&pool)
    .await?;
    assert_eq!(
        queued,
        [
            (
                "ada@example.test".into(),
                "EMAIL_VERIFICATION_REQUESTED".into()
            ),
            (
                "bob@example.test".into(),
                "EMAIL_VERIFICATION_REQUESTED".into()
            )
        ]
    );
    // An unverified customer cannot sign in.
    assert_eq!(login("ada").await.status(), StatusCode::UNAUTHORIZED);

    // Each link carries a fresh selector and a distinct 256-bit secret.
    let inbox = Arc::new(MockDelivery::default());
    let worker = VerificationWorker::sandbox(inbox.clone(), [11; 32], BASE).unwrap();
    let mut links = std::collections::HashMap::new();
    for _ in 0..2 {
        assert!(worker.tick(&worker_db).await?);
        let (id, text) = inbox.last_message().unwrap();
        let token = text
            .lines()
            .find_map(|line| line.strip_prefix(&format!("{BASE}#")))
            .unwrap()
            .to_owned();
        let (selector, secret) = token.split_once('.').unwrap();
        assert_eq!(selector.parse::<Uuid>().unwrap(), id);
        assert!(
            secret.len() == 64
                && secret
                    .bytes()
                    .all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f'))
        );
        let owner: String = sqlx::query_scalar(
            "SELECT u.email FROM identity.verification_challenges c JOIN identity.users u ON u.id=c.user_id WHERE c.id=$1",
        )
        .bind(id)
        .fetch_one(&pool)
        .await?;
        links.insert(owner, token);
    }
    let ada_link = &links["ada@example.test"];
    let bob_link = &links["bob@example.test"];
    assert_ne!(
        ada_link.split_once('.').unwrap().1,
        bob_link.split_once('.').unwrap().1
    );

    // The proof confirms the address once and advances only the contact state.
    let verify = |token: &str| {
        post(
            &verification,
            "/api/v1/auth/verify-email",
            json!({ "token": token }).to_string(),
            None,
        )
    };
    assert_eq!(verify(ada_link).await.status(), StatusCode::OK);
    assert_eq!(
        verify(ada_link).await.status(),
        StatusCode::UNPROCESSABLE_ENTITY
    );
    let state: (String, bool, bool) = sqlx::query_as(
        "SELECT status,email_verified_at IS NOT NULL,phone_verified_at IS NULL FROM identity.users WHERE email='ada@example.test'",
    )
    .fetch_one(&pool)
    .await?;
    assert_eq!(state, ("CONTACT_VERIFIED".into(), true, true));

    // The verified customer signs in without any KYC record; the other cannot.
    let res = login("ada").await;
    assert_eq!(res.status(), StatusCode::OK);
    let cookie = res.headers()[header::SET_COOKIE]
        .to_str()
        .unwrap()
        .split_once("; ")
        .unwrap()
        .0
        .to_owned();
    assert_eq!(
        status_of(&signin, &cookie).await,
        (StatusCode::OK, "CONTACT_VERIFIED".to_owned())
    );
    assert_eq!(login("bob").await.status(), StatusCode::UNAUTHORIZED);
    let unrelated: bool = sqlx::query_scalar(
        "SELECT NOT EXISTS(SELECT FROM compliance.kyc_cases) AND NOT EXISTS(SELECT FROM wallets.wallet_profiles)
            AND NOT EXISTS(SELECT FROM payments.transactions)",
    )
    .fetch_one(&pool)
    .await?;
    assert!(unrelated);

    // A resend for an already verified address is acknowledged without a new email.
    let res = post(
        &verification,
        "/api/v1/auth/resend-verification",
        json!({"email": "ada@example.test"}).to_string(),
        None,
    )
    .await;
    assert_eq!(res.status(), StatusCode::ACCEPTED);
    let intents: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM integrations.outbox_events e JOIN identity.users u ON u.id=e.aggregate_id WHERE u.email='ada@example.test'",
    )
    .fetch_one(&pool)
    .await?;
    assert_eq!(intents, 1);
    Ok(())
}
