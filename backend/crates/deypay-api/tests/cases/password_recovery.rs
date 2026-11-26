//! IMP-P3-008 password recovery through the real verifier routes, the IMP-P3-012
//! admission, the reset-email worker and PostgreSQL role boundaries.
use super::login_sessions::{RELEASE, auth_fixture};
use axum::{
    Router,
    body::{Body, to_bytes},
    http::{Request, StatusCode, header},
    response::Response,
};
use deypay_api::{
    login::{BrowserAuth, VerifierAuth, recovery::RecoverySettings},
    sessions::{BrowserOrigin, expired_session_cookie},
};
use deypay_application::passwords::PasswordHasher;
use deypay_infrastructure::{
    MIGRATOR,
    email_delivery::MockDelivery,
    password_hashing::Argon2idHasher,
    password_recovery::RecoveryWorker,
    password_screening::LocalCompromisedPasswords,
    sessions::{PgSessions, recovery::RecoveryLimits},
};
use serde_json::{Value, json};
use sqlx::{PgPool, postgres::PgPoolOptions};
use std::sync::Arc;
use tower::ServiceExt;
use uuid::Uuid;

const ORIGIN: &str = "https://sandbox.example.test";
const BASE: &str = "http://127.0.0.1:8000/reset-password";
const FORGOT: &str = "/api/v1/auth/forgot-password";
const RESET: &str = "/api/v1/auth/reset-password";
const LOGIN: &str = "/api/v1/auth/login";
const REFRESH: &str = "/api/v1/auth/refresh";
const OLD: &str = "Original1!pass";
const NEW: &str = "Replaced2@pass";
const BLOCKED: &str = "Breached3#pass";
const GUESSES: i32 = 3;

fn hasher() -> Argon2idHasher {
    Argon2idHasher::new(32, 2, 1, 4).unwrap()
}

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

/// A screening dataset containing only `BLOCKED`, digested by PostgreSQL.
async fn screening(pool: &PgPool) -> sqlx::Result<LocalCompromisedPasswords> {
    let data: String = sqlx::query_scalar("SELECT encode(sha256(convert_to($1,'UTF8')),'hex')")
        .bind(BLOCKED)
        .fetch_one(pool)
        .await?;
    let checksum: Vec<u8> = sqlx::query_scalar("SELECT sha256(convert_to($1,'UTF8'))")
        .bind(&data)
        .fetch_one(pool)
        .await?;
    let valid_until = std::time::SystemTime::now() + std::time::Duration::from_secs(3600);
    Ok(
        LocalCompromisedPasswords::load(data.as_bytes(), checksum.try_into().unwrap(), valid_until)
            .unwrap(),
    )
}

struct Fixture {
    app: Router,
    auth: Arc<VerifierAuth>,
    worker: RecoveryWorker,
    inbox: Arc<MockDelivery>,
    worker_db: PgPool,
    verifier_db: PgPool,
}

async fn fixture(pool: &PgPool) -> sqlx::Result<Fixture> {
    for script in [
        include_str!("../../../../../infrastructure/postgres/phase-two-grants.psql"),
        super::SANDBOX,
    ] {
        sqlx::raw_sql(script).execute(pool).await?;
    }
    auth_fixture(pool, 200, 100).await?;
    sqlx::query(
        "INSERT INTO identity.admission_policy(operation,total_limit,subject_limit)
         VALUES ('forgot_password',100,20),('reset_password',100,20),('username_availability',100,20)",
    )
    .execute(pool)
    .await?;
    super::pin_admission_windows(pool).await?;
    let name: String = sqlx::query_scalar("SELECT current_database()")
        .fetch_one(pool)
        .await?;
    let grants =
        include_str!("../../../../../infrastructure/postgres/password-recovery-worker-grants.psql")
            .replace(":\"DBNAME\"", &format!("\"{name}\""))
            .replace(":\"worker_role\"", "pg_read_all_settings");
    sqlx::raw_sql(&grants).execute(pool).await?;
    let verifier_db = role(pool, "SET ROLE deypay_auth").await?;
    let worker_db = role(pool, "SET ROLE pg_read_all_settings").await?;
    let auth = BrowserAuth::new(
        PgSessions::new(
            verifier_db.clone(),
            RELEASE.parse().unwrap(),
            [0x37; 32].into(),
        ),
        hasher(),
        BrowserOrigin::parse(ORIGIN).unwrap(),
    );
    let limits = RecoveryLimits {
        cooldown_seconds: 60,
        requests_per_hour: 3,
        guesses: GUESSES,
    };
    let settings = RecoverySettings::new(limits, screening(pool).await?);
    let app = auth.routes().merge(auth.recovery_routes(Some(settings)));
    let inbox = Arc::new(MockDelivery::default());
    let worker = RecoveryWorker::sandbox(inbox.clone(), [23; 32], BASE).unwrap();
    Ok(Fixture {
        app,
        auth,
        worker,
        inbox,
        worker_db,
        verifier_db,
    })
}

/// A registered customer whose email is verified (CONTACT_VERIFIED).
async fn seed(pool: &PgPool, name: &str) -> sqlx::Result<Uuid> {
    let email = format!("{name}@example.test");
    let hash = hasher().hash(OLD).await.unwrap();
    sqlx::query("SELECT identity.register_customer($1,'+2348012345678',$2,$3,'Test','Customer','NG','1.0','1.0',true,true)")
        .bind(&email).bind(name).bind(hash).execute(pool).await?;
    sqlx::query_scalar(
        "UPDATE identity.users SET email_verified_at=clock_timestamp() WHERE email=$1 RETURNING id",
    )
    .bind(email)
    .fetch_one(pool)
    .await
}

fn request(path: &str, body: impl Into<Body>, origin: Option<&str>) -> Request<Body> {
    let mut req = Request::post(path).header(header::CONTENT_TYPE, "application/json");
    if let Some(origin) = origin {
        req = req.header(header::ORIGIN, origin);
    }
    req.body(body.into()).unwrap()
}

async fn call(app: &Router, req: Request<Body>) -> Response {
    let res = app.clone().oneshot(req).await.unwrap();
    assert_eq!(res.headers()[header::CACHE_CONTROL], "no-store");
    res
}

async fn json_body(res: Response) -> Value {
    let bytes = to_bytes(res.into_body(), 64 * 1024).await.unwrap();
    serde_json::from_slice(&bytes).unwrap_or(Value::Null)
}

async fn forgot(app: &Router, email: &str) -> (StatusCode, Value) {
    let res = call(
        app,
        request(FORGOT, json!({ "email": email }).to_string(), Some(ORIGIN)),
    )
    .await;
    assert!(res.headers().get(header::SET_COOKIE).is_none());
    (res.status(), json_body(res).await)
}

async fn reset(app: &Router, token: &str, password: &str) -> Response {
    let body = json!({ "token": token, "password": password }).to_string();
    call(app, request(RESET, body, Some(ORIGIN))).await
}

async fn rejected(res: Response) -> &'static str {
    assert_eq!(res.status(), StatusCode::UNPROCESSABLE_ENTITY);
    let body = json_body(res).await;
    assert_eq!(body["error"]["code"], "VALIDATION_ERROR");
    match body["error"]["details"]["reason"].as_str() {
        Some("invalid_link") => "invalid_link",
        Some("password_rejected") => "password_rejected",
        other => panic!("unexpected reason {other:?}"),
    }
}

async fn login(app: &Router, name: &str, password: &str) -> Response {
    let body = json!({"identifier": format!("{name}@example.test"), "password": password});
    call(app, request(LOGIN, body.to_string(), Some(ORIGIN))).await
}

fn cookie(res: &Response) -> String {
    assert_eq!(res.status(), StatusCode::OK);
    let value = res.headers()[header::SET_COOKIE].to_str().unwrap();
    value.split_once("; ").unwrap().0.to_owned()
}

/// Runs the worker until the reset email for the newest intent is sent.
async fn deliver(f: &Fixture) -> sqlx::Result<String> {
    let before = f.inbox.last_message().map(|(id, _)| id);
    for _ in 0..5 {
        f.worker.tick(&f.worker_db).await?;
        if let Some((id, text)) = f.inbox.last_message()
            && Some(id) != before
        {
            let token = text
                .lines()
                .find_map(|line| line.strip_prefix(&format!("{BASE}#")))
                .expect("the email carries the reset link");
            assert!(token.starts_with(&id.to_string()));
            return Ok(token.to_owned());
        }
    }
    panic!("no reset email was sent");
}

async fn age_requests(pool: &PgPool) -> sqlx::Result<()> {
    sqlx::query("UPDATE integrations.outbox_events SET created_at=created_at-interval '61 seconds' WHERE event_type='PASSWORD_RESET_REQUESTED'")
        .execute(pool)
        .await?;
    Ok(())
}

async fn intents(pool: &PgPool) -> sqlx::Result<Vec<Uuid>> {
    sqlx::query_scalar("SELECT aggregate_id FROM integrations.outbox_events WHERE event_type='PASSWORD_RESET_REQUESTED' ORDER BY created_at")
        .fetch_all(pool)
        .await
}

async fn audit(pool: &PgPool, customer: Uuid) -> sqlx::Result<Vec<(String, Option<String>)>> {
    sqlx::query_as(
        "SELECT action,reason FROM audit.audit_events
         WHERE (resource_id=$1 OR actor_id=$1) AND (action LIKE 'PASSWORD_RESET%' OR action='SESSION_REVOKED')
         ORDER BY created_at,action",
    )
    .bind(customer)
    .fetch_all(pool)
    .await
}

#[sqlx::test(migrator = "MIGRATOR")]
async fn a_reset_link_replaces_the_password_and_ends_every_session(
    pool: PgPool,
) -> sqlx::Result<()> {
    let f = fixture(&pool).await?;
    let ada = seed(&pool, "ada").await?;
    let first = cookie(&login(&f.app, "ada", OLD).await);
    let _second = cookie(&login(&f.app, "ada", OLD).await);

    assert_eq!(
        forgot(&f.app, " ADA@Example.test ").await,
        (StatusCode::ACCEPTED, json!({"data": {}}))
    );
    let token = deliver(&f).await?;
    let (selector, secret) = token.split_once('.').unwrap();
    assert_eq!(secret.len(), 64);
    // Only the digest, expiry and delivery fingerprint are stored, never the secret.
    let stored: String = sqlx::query_scalar(
        "SELECT concat_ws(' ',c.code_hash,j.payload,e.payload,
            (SELECT string_agg(concat_ws(' ',metadata,reason),' ') FROM audit.audit_events))
         FROM identity.verification_challenges c JOIN integrations.jobs j ON j.id=c.id
         JOIN integrations.outbox_events e ON e.id=j.id WHERE c.challenge_type='PASSWORD_RESET'",
    )
    .fetch_one(&pool)
    .await?;
    assert!(!stored.contains(secret));

    let res = reset(&f.app, &token, NEW).await;
    assert_eq!(res.status(), StatusCode::OK);
    assert_eq!(res.headers()[header::SET_COOKIE], expired_session_cookie());
    assert_eq!(json_body(res).await, json!({"data": {}}));

    // SEC-REC-004: every session ends and the old cookie cannot be refreshed.
    let live: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM identity.sessions WHERE user_id=$1 AND revoked_at IS NULL",
    )
    .bind(ada)
    .fetch_one(&pool)
    .await?;
    assert_eq!(live, 0);
    let mut refresh = request(REFRESH, Body::empty(), Some(ORIGIN));
    refresh
        .headers_mut()
        .insert(header::COOKIE, first.parse().unwrap());
    assert_eq!(
        call(&f.app, refresh).await.status(),
        StatusCode::UNAUTHORIZED
    );
    // Only the new password signs in, and the link works once.
    assert_eq!(
        login(&f.app, "ada", OLD).await.status(),
        StatusCode::UNAUTHORIZED
    );
    cookie(&login(&f.app, "ada", NEW).await);
    assert_eq!(
        rejected(reset(&f.app, &token, "Another4$pass").await).await,
        "invalid_link"
    );

    let events = audit(&pool, ada).await?;
    let actions: Vec<&str> = events.iter().map(|(a, _)| a.as_str()).collect();
    assert!(actions.contains(&"PASSWORD_RESET_REQUESTED"));
    assert!(actions.contains(&"PASSWORD_RESET_COMPLETED"));
    let revoked: Vec<_> = events
        .iter()
        .filter(|(a, _)| a == "SESSION_REVOKED")
        .map(|(_, reason)| reason.as_deref())
        .collect();
    assert_eq!(revoked, [Some("PASSWORD_RESET"), Some("PASSWORD_RESET")]);
    let delivered: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM audit.audit_events WHERE action='PASSWORD_RESET_EMAIL_ACCEPTED' AND resource_id=$1",
    )
    .bind(selector.parse::<Uuid>().unwrap())
    .fetch_one(&pool)
    .await?;
    assert_eq!(delivered, 1);
    Ok(())
}

#[sqlx::test(migrator = "MIGRATOR")]
async fn forgot_password_answers_alike_whether_or_not_an_account_qualifies(
    pool: PgPool,
) -> sqlx::Result<()> {
    let f = fixture(&pool).await?;
    let ada = seed(&pool, "ada").await?;
    // Unverified email, and a verified account outside the release's states.
    sqlx::query("SELECT identity.register_customer('bob@example.test','+2348012345679','bob','$argon2id$test-only','Test','Customer','NG','1.0','1.0',true,true)")
        .execute(&pool).await?;
    let carol = seed(&pool, "carol").await?;
    sqlx::query("UPDATE identity.users SET status='SUSPENDED' WHERE id=$1")
        .bind(carol)
        .execute(&pool)
        .await?;
    for email in [
        "ada@example.test",
        "nobody@example.test",
        "bob@example.test",
        "carol@example.test",
        // The per-account cooldown also answers alike.
        "ada@example.test",
    ] {
        assert_eq!(
            forgot(&f.app, email).await,
            (StatusCode::ACCEPTED, json!({"data": {}})),
            "{email}"
        );
    }
    assert_eq!(intents(&pool).await?, [ada]);

    // Transport and validation failures.
    let (status, body) = forgot(&f.app, "not-an-address").await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(body["error"]["details"], json!({}));
    for (body, origin, expected) in [
        (
            json!({"email": "ada@example.test", "extra": 1}).to_string(),
            Some(ORIGIN),
            StatusCode::BAD_REQUEST,
        ),
        ("not json".to_owned(), Some(ORIGIN), StatusCode::BAD_REQUEST),
        (
            json!({"email": "ada@example.test"}).to_string(),
            None,
            StatusCode::FORBIDDEN,
        ),
        (
            json!({"email": "ada@example.test"}).to_string(),
            Some("https://evil.example.test"),
            StatusCode::FORBIDDEN,
        ),
    ] {
        let res = call(&f.app, request(FORGOT, body, origin)).await;
        assert_eq!(res.status(), expected);
    }
    let mut plain = request(FORGOT, r#"{"email":"ada@example.test"}"#, Some(ORIGIN));
    plain
        .headers_mut()
        .insert(header::CONTENT_TYPE, "text/plain".parse().unwrap());
    assert_eq!(call(&f.app, plain).await.status(), StatusCode::BAD_REQUEST);
    assert_eq!(intents(&pool).await?, [ada]);
    Ok(())
}

#[sqlx::test(migrator = "MIGRATOR")]
async fn links_are_superseded_guess_limited_expiring_and_capped_per_hour(
    pool: PgPool,
) -> sqlx::Result<()> {
    let f = fixture(&pool).await?;
    let ada = seed(&pool, "ada").await?;
    forgot(&f.app, "ada@example.test").await;
    let superseded = deliver(&f).await?;
    age_requests(&pool).await?;
    forgot(&f.app, "ada@example.test").await;
    let current = deliver(&f).await?;
    assert_ne!(superseded, current);
    assert_eq!(
        rejected(reset(&f.app, &superseded, NEW).await).await,
        "invalid_link"
    );

    // Wrong secrets spend the guess budget; the last one revokes the link.
    let selector = current.split_once('.').unwrap().0;
    let wrong = format!("{selector}.{}", "0".repeat(64));
    for _ in 0..GUESSES {
        assert_eq!(
            rejected(reset(&f.app, &wrong, NEW).await).await,
            "invalid_link"
        );
    }
    assert_eq!(
        rejected(reset(&f.app, &current, NEW).await).await,
        "invalid_link"
    );
    let exhausted: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM audit.audit_events WHERE action='PASSWORD_RESET_ATTEMPTS_EXHAUSTED' AND resource_id=$1",
    )
    .bind(ada)
    .fetch_one(&pool)
    .await?;
    assert_eq!(exhausted, 1);

    // A third link is issued normally.
    age_requests(&pool).await?;
    forgot(&f.app, "ada@example.test").await;
    deliver(&f).await?;
    // An expired link fails even with the right secret. Challenge timestamps
    // are immutable, so seed one that has already expired.
    let expired: Uuid = sqlx::query_scalar(
        "INSERT INTO identity.verification_challenges(user_id,challenge_type,destination,code_hash,created_at,expires_at)
         SELECT id,'PASSWORD_RESET',email,encode(sha256(decode(repeat('aa',32),'hex')),'hex'),
                now()-interval '2 hours',now()-interval '1 hour'
         FROM identity.users WHERE id=$1 RETURNING id",
    )
    .bind(ada)
    .fetch_one(&pool)
    .await?;
    let token = format!("{expired}.{}", "aa".repeat(32));
    assert_eq!(
        rejected(reset(&f.app, &token, NEW).await).await,
        "invalid_link"
    );
    // Refused for expiry before the secret was compared: no guess was spent.
    let guesses: i32 = sqlx::query_scalar(
        "SELECT attempt_count FROM identity.verification_challenges WHERE id=$1",
    )
    .bind(expired)
    .fetch_one(&pool)
    .await?;
    assert_eq!(guesses, 0);

    // Three emails per hour: a fourth request is acknowledged but not queued.
    age_requests(&pool).await?;
    assert_eq!(
        forgot(&f.app, "ada@example.test").await.0,
        StatusCode::ACCEPTED
    );
    assert_eq!(intents(&pool).await?.len(), 3);
    // The original password still works; nothing was reset.
    cookie(&login(&f.app, "ada", OLD).await);
    Ok(())
}

#[sqlx::test(migrator = "MIGRATOR")]
async fn one_link_resets_the_password_exactly_once_under_concurrency(
    pool: PgPool,
) -> sqlx::Result<()> {
    let f = fixture(&pool).await?;
    let ada = seed(&pool, "ada").await?;
    forgot(&f.app, "ada@example.test").await;
    let token = deliver(&f).await?;
    let other = "Racing5%pass";
    let (a, b) = tokio::join!(reset(&f.app, &token, NEW), reset(&f.app, &token, other));
    let mut statuses = [a.status(), b.status()];
    statuses.sort();
    assert_eq!(statuses, [StatusCode::OK, StatusCode::UNPROCESSABLE_ENTITY]);
    let winner = if a.status() == StatusCode::OK {
        NEW
    } else {
        other
    };
    let loser = if winner == NEW { other } else { NEW };
    cookie(&login(&f.app, "ada", winner).await);
    assert_eq!(
        login(&f.app, "ada", loser).await.status(),
        StatusCode::UNAUTHORIZED
    );
    let completed: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM audit.audit_events WHERE action='PASSWORD_RESET_COMPLETED' AND resource_id=$1",
    )
    .bind(ada)
    .fetch_one(&pool)
    .await?;
    assert_eq!(completed, 1);
    Ok(())
}

#[sqlx::test(migrator = "MIGRATOR")]
async fn rejected_passwords_and_malformed_requests_do_not_spend_the_link(
    pool: PgPool,
) -> sqlx::Result<()> {
    let f = fixture(&pool).await?;
    seed(&pool, "ada").await?;
    forgot(&f.app, "ada@example.test").await;
    let token = deliver(&f).await?;
    // The registration policy and compromised-password screening apply.
    for password in ["short", "nouppercase1!", BLOCKED, ""] {
        assert_eq!(
            rejected(reset(&f.app, &token, password).await).await,
            "password_rejected",
            "{password}"
        );
    }
    for token in ["", "abc", "not-a-uuid.00", &token.replace('.', "")] {
        assert_eq!(
            rejected(reset(&f.app, token, NEW).await).await,
            "invalid_link"
        );
    }
    for body in [
        json!({"token": token, "password": NEW, "email": "ada@example.test"}).to_string(),
        json!({"token": token}).to_string(),
        json!({"token": token, "password": null}).to_string(),
        "x".repeat(70 * 1024),
    ] {
        let res = call(&f.app, request(RESET, body, Some(ORIGIN))).await;
        assert_eq!(res.status(), StatusCode::BAD_REQUEST);
    }
    let untouched: (i32, bool) = sqlx::query_as(
        "SELECT attempt_count,consumed_at IS NULL FROM identity.verification_challenges WHERE challenge_type='PASSWORD_RESET'",
    )
    .fetch_one(&pool)
    .await?;
    assert_eq!(untouched, (0, true));
    assert_eq!(reset(&f.app, &token, NEW).await.status(), StatusCode::OK);
    Ok(())
}

#[sqlx::test(migrator = "MIGRATOR")]
async fn recovery_follows_the_shared_admission_contract(pool: PgPool) -> sqlx::Result<()> {
    let f = fixture(&pool).await?;
    let ada = seed(&pool, "ada").await?;
    // An exhausted address budget is acknowledged without sending mail.
    sqlx::query(
        "UPDATE identity.admission_policy SET subject_limit=1 WHERE operation='forgot_password'",
    )
    .execute(&pool)
    .await?;
    super::pin_admission_windows(&pool).await?;
    assert_eq!(
        forgot(&f.app, "ada@example.test").await.0,
        StatusCode::ACCEPTED
    );
    age_requests(&pool).await?;
    assert_eq!(
        forgot(&f.app, "ADA@example.test").await,
        (StatusCode::ACCEPTED, json!({"data": {}}))
    );
    assert_eq!(intents(&pool).await?, [ada]);

    // A selector's budget is charged before any password work.
    let token = deliver(&f).await?;
    sqlx::query(
        "UPDATE identity.admission_policy SET subject_limit=1 WHERE operation='reset_password'",
    )
    .execute(&pool)
    .await?;
    super::pin_admission_windows(&pool).await?;
    assert_eq!(
        rejected(reset(&f.app, &token, "short").await).await,
        "password_rejected"
    );
    let limited = reset(&f.app, &token, NEW).await;
    assert_eq!(limited.status(), StatusCode::TOO_MANY_REQUESTS);
    assert_eq!(json_body(limited).await["error"]["code"], "RATE_LIMITED");

    // The deployment budget applies before the body is read.
    sqlx::query("UPDATE identity.admission_policy SET total_limit=1,subject_limit=1 WHERE operation='reset_password'")
        .execute(&pool)
        .await?;
    super::pin_admission_windows(&pool).await?;
    let first = call(&f.app, request(RESET, "garbage", Some(ORIGIN))).await;
    assert_eq!(first.status(), StatusCode::BAD_REQUEST);
    let second = call(&f.app, request(RESET, "garbage", Some(ORIGIN))).await;
    assert_eq!(second.status(), StatusCode::TOO_MANY_REQUESTS);

    // Without an operator policy the operation is closed.
    sqlx::query("DELETE FROM identity.admission_policy WHERE operation IN ('forgot_password','reset_password')")
        .execute(&pool)
        .await?;
    assert_eq!(
        forgot(&f.app, "ada@example.test").await.0,
        StatusCode::SERVICE_UNAVAILABLE
    );
    assert_eq!(
        reset(&f.app, &token, NEW).await.status(),
        StatusCode::SERVICE_UNAVAILABLE
    );
    // The link is still unused and the password unchanged.
    cookie(&login(&f.app, "ada", OLD).await);
    Ok(())
}

#[sqlx::test(migrator = "MIGRATOR")]
async fn recovery_stays_inside_the_verifier_boundary(pool: PgPool) -> sqlx::Result<()> {
    let f = fixture(&pool).await?;
    let ada = seed(&pool, "ada").await?;
    forgot(&f.app, "ada@example.test").await;
    let token = deliver(&f).await?;

    // An account that leaves the release's states cannot complete a reset.
    let before: String = sqlx::query_scalar(
        "SELECT password_hash FROM identity.password_credentials WHERE user_id=$1",
    )
    .bind(ada)
    .fetch_one(&pool)
    .await?;
    sqlx::query("UPDATE identity.users SET status='SUSPENDED' WHERE id=$1")
        .bind(ada)
        .execute(&pool)
        .await?;
    assert_eq!(
        rejected(reset(&f.app, &token, NEW).await).await,
        "invalid_link"
    );
    let after: String = sqlx::query_scalar(
        "SELECT password_hash FROM identity.password_credentials WHERE user_id=$1",
    )
    .bind(ada)
    .fetch_one(&pool)
    .await?;
    assert_eq!(before, after);

    // The general API and a verifier without recovery settings refuse both routes.
    let general = deypay_api::login::routes(None);
    let disabled = f.auth.recovery_routes(None);
    for app in [&general, &disabled] {
        for path in [FORGOT, RESET] {
            let res = call(app, request(path, "{}", Some(ORIGIN))).await;
            assert_eq!(res.status(), StatusCode::SERVICE_UNAVAILABLE);
        }
    }

    // Runtime roles reach recovery only through the signed verifier capabilities.
    let api_db = role(&pool, "SET ROLE deypay_app").await?;
    let helpers = [
        "SELECT identity.enqueue_password_reset(gen_random_uuid(),ARRAY['CONTACT_VERIFIED'],60,3)",
        "SELECT identity.redeem_password_reset(gen_random_uuid(),repeat('a',64),'$argon2id$x',3,ARRAY['CONTACT_VERIFIED'])",
    ];
    let capabilities = [
        "SELECT identity.auth_request_reset($1::uuid,'ada@example.test',60,3,0,'\\x00'::bytea)",
        "SELECT identity.auth_reset_password($1::uuid,gen_random_uuid(),repeat('a',64),'$argon2id$x',3,0,'\\x00'::bytea)",
    ];
    for db in [&api_db, &f.verifier_db, &f.worker_db] {
        for statement in helpers {
            assert!(
                sqlx::query(statement).execute(db).await.is_err(),
                "{statement}"
            );
        }
    }
    for db in [&api_db, &f.worker_db] {
        for statement in capabilities {
            let call = sqlx::query(statement).bind(RELEASE.parse::<Uuid>().unwrap());
            assert!(call.execute(db).await.is_err(), "{statement}");
        }
    }
    // The verifier login alone, without the signing key, is refused.
    let now: i64 = sqlx::query_scalar("SELECT extract(epoch FROM clock_timestamp())::bigint")
        .fetch_one(&pool)
        .await?;
    for statement in [
        "SELECT identity.auth_request_reset($1::uuid,'ada@example.test',60,3,$2,decode(repeat('00',32),'hex'))",
        "SELECT identity.auth_reset_password($1::uuid,gen_random_uuid(),repeat('a',64),'$argon2id$x',3,$2,decode(repeat('00',32),'hex'))",
    ] {
        let forged = sqlx::query(statement)
            .bind(RELEASE.parse::<Uuid>().unwrap())
            .bind(now);
        assert!(forged.execute(&f.verifier_db).await.is_err(), "{statement}");
    }
    let ok: bool = sqlx::query_scalar("SELECT identity.auth_acl_ok() AND identity.auth_owner_ok()")
        .fetch_one(&pool)
        .await?;
    assert!(ok);
    Ok(())
}
