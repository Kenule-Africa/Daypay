use axum::{
    Router,
    body::Body,
    http::{Request, StatusCode, header},
    response::Response,
    routing::get,
};
use deypay_api::{
    login::{AuthenticatedCustomer, CustomerAuth, LoginLimits, SandboxAuth},
    sessions::BrowserOrigin,
};
use deypay_application::{
    login::SessionLifetime,
    passwords::{CredentialError, CredentialVerifier, PasswordHasher, VerifiedPassword},
};
use deypay_common::config::Environment;
use deypay_infrastructure::{
    MIGRATOR,
    password_hashing::Argon2idHasher,
    sessions::{PgSessions, SessionToken},
};
use serde_json::json;
use sqlx::{PgPool, postgres::PgPoolOptions};
use tower::ServiceExt;
use uuid::Uuid;

const ORIGIN: &str = "https://sandbox.example.test";
const PW: &str = "Sandbox1!password";
const LOGIN: &str = "/api/v1/auth/login";
const REFRESH: &str = "/api/v1/auth/refresh";

fn hasher() -> Argon2idHasher {
    Argon2idHasher::new(32, 2, 1, 4).unwrap()
}

async fn runtime(pool: &PgPool) -> sqlx::Result<PgPool> {
    sqlx::raw_sql(include_str!(
        "../../../../../infrastructure/postgres/phase-two-grants.psql"
    ))
    .execute(pool)
    .await?;
    sqlx::raw_sql(include_str!(
        "../../../../../infrastructure/postgres/sandbox-auth-grants.psql"
    ))
    .execute(pool)
    .await?;
    sqlx::raw_sql(super::SANDBOX).execute(pool).await?;
    PgPoolOptions::new()
        .max_connections(5)
        .after_connect(|conn, _| {
            Box::pin(async move {
                sqlx::query("SET ROLE deypay_app").execute(conn).await?;
                Ok(())
            })
        })
        .connect_with((*pool.connect_options()).clone())
        .await
}

fn router<V: CredentialVerifier + Send + 'static>(
    pool: PgPool,
    verifier: V,
    per_id: u32,
) -> Router {
    let auth = SandboxAuth::new(
        Environment::Test,
        PgSessions(pool),
        verifier,
        BrowserOrigin::parse(ORIGIN).unwrap(),
        SessionLifetime::new(120, 600).unwrap(),
        LoginLimits::new(200.try_into().unwrap(), per_id.try_into().unwrap()),
    )
    .unwrap();
    async fn who(customer: AuthenticatedCustomer) -> String {
        customer.id().to_string()
    }
    auth.routes()
        .merge(auth.protect(Router::new().route("/test/principal", get(who).post(who))))
        .route("/test/unprotected", get(who))
}

async fn seed(pool: &PgPool, name: &str) -> sqlx::Result<Uuid> {
    let email = format!("{name}@example.test");
    let hash = hasher().hash(PW).await.unwrap();
    sqlx::query("SELECT identity.register_customer($1,'+2348012345678',$2,$3,'Test','Customer','NG','1.0','1.0',true,true)")
        .bind(&email).bind(name).bind(hash).execute(pool).await?;
    sqlx::query_scalar(
        "UPDATE identity.users SET email_verified_at=clock_timestamp() WHERE email=$1 RETURNING id",
    )
    .bind(email)
    .fetch_one(pool)
    .await
}

async fn call(app: &Router, req: Request<Body>) -> Response {
    app.clone().oneshot(req).await.unwrap()
}
fn post(path: &str, body: String, cookie: Option<&str>) -> Request<Body> {
    let mut req = Request::post(path)
        .header(header::CONTENT_TYPE, "application/json")
        .header(header::ORIGIN, ORIGIN);
    if let Some(cookie) = cookie {
        req = req.header(header::COOKIE, cookie);
    }
    req.body(Body::from(body)).unwrap()
}
async fn login(app: &Router, email: &str, password: &str, cookie: Option<&str>) -> Response {
    call(
        app,
        post(
            LOGIN,
            json!({"identifier":email,"password":password}).to_string(),
            cookie,
        ),
    )
    .await
}
async fn principal(app: &Router, cookie: Option<&str>) -> Response {
    let mut req = Request::get("/test/principal").header("x-customer-id", Uuid::nil().to_string());
    if let Some(cookie) = cookie {
        req = req.header(header::COOKIE, cookie);
    }
    call(app, req.body(Body::empty()).unwrap()).await
}
fn pair(res: &Response) -> String {
    assert_eq!(res.status(), StatusCode::OK);
    let (cookie, attributes) = res.headers()[header::SET_COOKIE]
        .to_str()
        .unwrap()
        .split_once("; ")
        .unwrap();
    assert_eq!(attributes, "Path=/; Secure; HttpOnly; SameSite=Lax");
    cookie.to_owned()
}
async fn rejected(res: Response) -> Vec<u8> {
    assert_eq!(res.status(), StatusCode::UNAUTHORIZED);
    assert!(res.headers().get(header::SET_COOKIE).is_none());
    assert_eq!(res.headers()[header::CACHE_CONTROL], "no-store");
    super::bytes(res).await.to_vec()
}

#[sqlx::test(migrator = "MIGRATOR")]
async fn login_binds_only_the_verified_customer_and_rotates_an_existing_cookie(
    pool: PgPool,
) -> sqlx::Result<()> {
    let api = runtime(&pool).await?;
    assert!(
        sqlx::query("SELECT password_hash FROM identity.password_credentials")
            .execute(&api)
            .await
            .is_err()
    );
    assert!(
        sqlx::query("SELECT token_hash FROM identity.sessions")
            .execute(&api)
            .await
            .is_err()
    );
    assert!(
        sqlx::query("UPDATE identity.sessions SET revoked_at=now()")
            .execute(&api)
            .await
            .is_err()
    );
    let app = router(api, hasher(), 100);
    let ada = seed(&pool, "ada").await?;
    let ben = seed(&pool, "ben").await?;
    let first = login(&app, " Ada@EXAMPLE.TEST ", PW, None).await;
    let a = pair(&first);
    assert_eq!(super::bytes(first).await.as_ref(), br#"{"data":{}}"#);
    assert_eq!(
        super::bytes(principal(&app, Some(&a)).await).await.as_ref(),
        ada.to_string().as_bytes()
    );
    let raw = a.split_once('=').unwrap().1;
    let stored: String =
        sqlx::query_scalar("SELECT token_hash FROM identity.sessions WHERE user_id=$1")
            .bind(ada)
            .fetch_one(&pool)
            .await?;
    assert!(stored != raw && stored == SessionToken::from_cookie(raw).unwrap().digest());
    let b = pair(&login(&app, "ben@example.test", PW, Some(&a)).await);
    assert!(a != b);
    rejected(principal(&app, Some(&a)).await).await;
    assert_eq!(
        super::bytes(principal(&app, Some(&b)).await).await.as_ref(),
        ben.to_string().as_bytes()
    );
    rejected(principal(&app, None).await).await;
    rejected(principal(&app, Some("__Host-deypay_session=malformed")).await).await;
    let unknown = format!(
        "__Host-deypay_session={}",
        SessionToken::generate().unwrap().cookie_value().as_str()
    );
    rejected(principal(&app, Some(&unknown)).await).await;
    rejected(
        call(
            &app,
            Request::get("/test/unprotected")
                .header(header::COOKIE, &b)
                .header("x-customer-id", ada.to_string())
                .body(Body::empty())
                .unwrap(),
        )
        .await,
    )
    .await;
    let unsafe_req = Request::post("/test/principal")
        .header(header::COOKIE, &b)
        .body(Body::empty())
        .unwrap();
    assert_eq!(call(&app, unsafe_req).await.status(), StatusCode::FORBIDDEN);
    assert!(sqlx::query_scalar::<_,bool>("SELECT (SELECT count(*) FROM audit.audit_events WHERE action='LOGIN_SUCCEEDED')=2
        AND NOT EXISTS(SELECT FROM compliance.kyc_cases) AND NOT EXISTS(SELECT FROM wallets.wallet_profiles)
        AND NOT EXISTS(SELECT FROM payments.transactions) AND NOT EXISTS(SELECT FROM audit.audit_events WHERE
        strpos(to_jsonb(audit_events)::text,$1)>0 OR strpos(to_jsonb(audit_events)::text,$2)>0 OR strpos(to_jsonb(audit_events)::text,'$argon2id$')>0)")
        .bind(raw).bind(PW).fetch_one(&pool).await?);
    Ok(())
}

#[sqlx::test(migrator = "MIGRATOR")]
async fn credential_and_eligibility_failures_are_indistinguishable(
    pool: PgPool,
) -> sqlx::Result<()> {
    let app = router(runtime(&pool).await?, hasher(), 100);
    let id = seed(&pool, "ada").await?;
    let expected = rejected(login(&app, "unknown@example.test", PW, None).await).await;
    assert!(expected == rejected(login(&app, "ada@example.test", "Incorrect1!", None).await).await);
    for state in [
        "REGISTERED",
        "CONTACT_UNVERIFIED",
        "KYC_REQUIRED",
        "KYC_PENDING",
        "KYC_APPROVED",
        "KYC_REJECTED",
        "RESTRICTED",
        "SUSPENDED",
        "CLOSED",
    ] {
        sqlx::query("UPDATE identity.users SET status=$1,closed_at=CASE WHEN $1='CLOSED' THEN now() END WHERE id=$2")
            .bind(state).bind(id).execute(&pool).await?;
        assert!(expected == rejected(login(&app, "ada@example.test", PW, None).await).await);
    }
    sqlx::query("UPDATE identity.users SET status='CONTACT_VERIFIED',closed_at=NULL WHERE id=$1")
        .bind(id)
        .execute(&pool)
        .await?;
    for hash in [
        "$argon2id$corrupt",
        &hasher()
            .hash(PW)
            .await
            .unwrap()
            .replace("m=32", "m=4294967295"),
    ] {
        sqlx::query("UPDATE identity.password_credentials SET password_hash=$1 WHERE user_id=$2")
            .bind(hash)
            .bind(id)
            .execute(&pool)
            .await?;
        assert!(expected == rejected(login(&app, "ada@example.test", PW, None).await).await);
    }
    sqlx::query("DELETE FROM identity.password_credentials WHERE user_id=$1")
        .bind(id)
        .execute(&pool)
        .await?;
    assert!(expected == rejected(login(&app, "ada@example.test", PW, None).await).await);
    assert_eq!(
        call(
            &app,
            post(
                LOGIN,
                r#"{"identifier":"ada@example.test","customer_id":"forged"}"#.into(),
                None
            )
        )
        .await
        .status(),
        StatusCode::BAD_REQUEST
    );
    assert!(sqlx::query_scalar::<_,bool>("SELECT NOT EXISTS(SELECT FROM identity.sessions) AND EXISTS(SELECT FROM audit.audit_events WHERE action='LOGIN_REJECTED')").fetch_one(&pool).await?);
    Ok(())
}

#[sqlx::test(migrator = "MIGRATOR")]
async fn concurrent_refresh_is_single_use_and_preserves_absolute_deadline(
    pool: PgPool,
) -> sqlx::Result<()> {
    let app = router(runtime(&pool).await?, hasher(), 100);
    let id = seed(&pool, "ada").await?;
    let cookie = pair(&login(&app, "ada@example.test", PW, None).await);
    let deadline: String =
        sqlx::query_scalar("SELECT absolute_expires_at::text FROM identity.sessions")
            .fetch_one(&pool)
            .await?;
    let (a, b) = tokio::join!(
        call(&app, post(REFRESH, String::new(), Some(&cookie))),
        call(&app, post(REFRESH, String::new(), Some(&cookie)))
    );
    let (ok, denied) = if a.status() == StatusCode::OK {
        (a, b)
    } else {
        (b, a)
    };
    let rotated = pair(&ok);
    rejected(denied).await;
    rejected(principal(&app, Some(&cookie)).await).await;
    assert!(rotated != cookie);
    assert!(sqlx::query_scalar::<_,bool>("SELECT absolute_expires_at::text=$1 AND expires_at<=absolute_expires_at FROM identity.sessions")
        .bind(deadline).fetch_one(&pool).await?);
    let (a, b) = tokio::join!(
        principal(&app, Some(&rotated)),
        principal(&app, Some(&rotated))
    );
    assert_eq!((a.status(), b.status()), (StatusCode::OK, StatusCode::OK));
    sqlx::query("UPDATE identity.users SET status='SUSPENDED' WHERE id=$1")
        .bind(id)
        .execute(&pool)
        .await?;
    rejected(principal(&app, Some(&rotated)).await).await;
    sqlx::query("UPDATE identity.users SET status='CONTACT_VERIFIED' WHERE id=$1")
        .bind(id)
        .execute(&pool)
        .await?;
    rejected(principal(&app, Some(&rotated)).await).await;
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT count(*) FROM audit.audit_events WHERE action='SESSION_REFRESHED'"
        )
        .fetch_one(&pool)
        .await?,
        1
    );
    Ok(())
}

#[sqlx::test(migrator = "MIGRATOR")]
async fn idle_absolute_and_revoked_sessions_cannot_be_used_or_refreshed(
    pool: PgPool,
) -> sqlx::Result<()> {
    let app = router(runtime(&pool).await?, hasher(), 100);
    let id = seed(&pool, "ada").await?;
    for (expired_idle, expired_absolute, revoked) in [
        (true, false, false),
        (true, true, false),
        (false, false, true),
    ] {
        let token = SessionToken::generate().unwrap();
        sqlx::query("INSERT INTO identity.sessions(user_id,token_hash,created_at,last_activity_at,expires_at,absolute_expires_at,revoked_at)
            SELECT $1,$2,now()-interval '1 hour',now()-interval '1 hour',
            now()+CASE WHEN $3 THEN interval '-1 second' ELSE interval '1 minute' END,
            now()+CASE WHEN $4 THEN interval '-1 second' ELSE interval '1 day' END,CASE WHEN $5 THEN now() END")
            .bind(id).bind(token.digest()).bind(expired_idle).bind(expired_absolute).bind(revoked).execute(&pool).await?;
        let cookie = format!("__Host-deypay_session={}", token.cookie_value().as_str());
        rejected(principal(&app, Some(&cookie)).await).await;
        rejected(call(&app, post(REFRESH, String::new(), Some(&cookie))).await).await;
    }
    Ok(())
}

#[sqlx::test(migrator = "MIGRATOR")]
async fn audit_issuance_and_lookup_failures_never_issue_a_cookie(pool: PgPool) -> sqlx::Result<()> {
    let api = runtime(&pool).await?;
    let app = router(api.clone(), hasher(), 100);
    seed(&pool, "ada").await?;
    let prior = pair(&login(&app, "ada@example.test", PW, None).await);
    sqlx::raw_sql("CREATE FUNCTION audit.fail_test_login() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN RAISE EXCEPTION 'synthetic outage'; END $$;
        CREATE TRIGGER test_login_failure BEFORE INSERT ON audit.audit_events FOR EACH ROW
        WHEN (NEW.action IN ('LOGIN_SUCCEEDED','SESSION_REFRESHED')) EXECUTE FUNCTION audit.fail_test_login();").execute(&pool).await?;
    for response in [
        login(&app, "ada@example.test", PW, Some(&prior)).await,
        call(&app, post(REFRESH, String::new(), Some(&prior))).await,
    ] {
        assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
        assert!(response.headers().get(header::SET_COOKIE).is_none());
    }
    assert_eq!(principal(&app, Some(&prior)).await.status(), StatusCode::OK);
    assert!(
        sqlx::query_scalar::<_, bool>(
            "SELECT count(*)=1 AND bool_and(revoked_at IS NULL) FROM identity.sessions"
        )
        .fetch_one(&pool)
        .await?
    );
    sqlx::query(
        "REVOKE EXECUTE ON FUNCTION identity.sandbox_login_credential(text) FROM deypay_app",
    )
    .execute(&pool)
    .await?;
    let unavailable = login(&app, "ada@example.test", PW, None).await;
    assert_eq!(unavailable.status(), StatusCode::SERVICE_UNAVAILABLE);
    assert!(unavailable.headers().get(header::SET_COOKIE).is_none());
    let error = String::from_utf8(super::bytes(unavailable).await.to_vec()).unwrap();
    assert!(
        !error.contains(PW)
            && !error.contains("argon2")
            && !error.contains("identity.")
            && !error.contains("ada@")
    );
    api.close().await;
    assert_eq!(
        principal(&app, Some(&prior)).await.status(),
        StatusCode::SERVICE_UNAVAILABLE
    );
    Ok(())
}

struct FailedVerifier(CredentialError);
impl CredentialVerifier for FailedVerifier {
    async fn verify(&self, _: &str, _: Option<&str>) -> Result<VerifiedPassword, CredentialError> {
        Err(self.0)
    }
}

#[sqlx::test(migrator = "MIGRATOR")]
async fn login_admission_and_verifier_errors_fail_closed(pool: PgPool) -> sqlx::Result<()> {
    let api = runtime(&pool).await?;
    for (error, expected) in [
        (CredentialError::Busy, StatusCode::TOO_MANY_REQUESTS),
        (
            CredentialError::Unavailable,
            StatusCode::SERVICE_UNAVAILABLE,
        ),
    ] {
        let app = router(api.clone(), FailedVerifier(error), 20);
        let response = login(&app, "missing@example.test", PW, None).await;
        assert_eq!(response.status(), expected);
        assert!(response.headers().get(header::SET_COOKIE).is_none());
    }
    let app = router(api.clone(), hasher(), 1);
    rejected(login(&app, "missing@example.test", PW, None).await).await;
    assert_eq!(
        login(&app, " MISSING@example.test ", PW, None)
            .await
            .status(),
        StatusCode::TOO_MANY_REQUESTS
    );
    for env in [Environment::Staging, Environment::Production] {
        assert!(CustomerAuth::from_env(env, api.clone()).unwrap().is_none());
    }
    assert_eq!(
        login(&deypay_api::login::routes(None), "a@example.test", PW, None)
            .await
            .status(),
        StatusCode::SERVICE_UNAVAILABLE
    );
    Ok(())
}
#[sqlx::test(migrator = "MIGRATOR")]
async fn stale_password_check_and_concurrent_revocation_fail_closed(
    pool: PgPool,
) -> sqlx::Result<()> {
    use deypay_application::login::{LoginStore, check_password};
    let api = runtime(&pool).await?;
    let app = router(api.clone(), hasher(), 100);
    let id = seed(&pool, "ada").await?;
    let prior = pair(&login(&app, "ada@example.test", PW, None).await);
    let store = PgSessions(api);
    let record = store.credential("ada@example.test").await.unwrap();
    let checked = check_password(&hasher(), record, PW).await.unwrap();
    sqlx::query("UPDATE identity.password_credentials SET password_hash=$2 WHERE user_id=$1")
        .bind(id)
        .bind(hasher().hash("Different1!").await.unwrap())
        .execute(&pool)
        .await?;
    let token = SessionToken::generate().unwrap();
    assert!(matches!(
        store
            .issue(
                checked,
                "ada@example.test",
                &token.digest(),
                None,
                SessionLifetime::new(120, 600).unwrap()
            )
            .await,
        Err(CredentialError::Rejected)
    ));
    let mut tx = pool.begin().await?;
    sqlx::query("UPDATE identity.sessions SET revoked_at=clock_timestamp() WHERE user_id=$1")
        .bind(id)
        .execute(&mut *tx)
        .await?;
    let mut pending = std::pin::pin!(principal(&app, Some(&prior)));
    assert!(
        tokio::time::timeout(std::time::Duration::from_millis(50), &mut pending)
            .await
            .is_err()
    );
    tx.commit().await?;
    rejected(pending.await).await;
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM identity.sessions")
            .fetch_one(&pool)
            .await?,
        1
    );
    Ok(())
}
