use axum::{
    Router,
    body::Body,
    http::{Request, StatusCode, header},
    response::Response,
    routing::get,
};
use deypay_api::{
    login::{AuthenticatedCustomer, BrowserAuth},
    sessions::BrowserOrigin,
};
use deypay_application::passwords::{
    CredentialError, CredentialVerifier, PasswordHasher, VerifiedPassword,
};
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

pub(super) const RELEASE: &str = "ed000000-0000-4000-8000-000000000001";

pub(super) async fn auth_fixture(pool: &PgPool, total: i32, per_id: i32) -> sqlx::Result<()> {
    // The cluster bootstrap supplies both roles. Each fixture has isolated policy and counters.
    sqlx::raw_sql(include_str!(
        "../../../../../infrastructure/postgres/auth-grants.psql"
    ))
    .execute(pool)
    .await?;
    sqlx::query("INSERT INTO identity.auth_policy(id,enabled,environment,origin,security_ref,product_ref,audit_ref,signing_key,allowed_states,
        idle_seconds,absolute_seconds,total_limit,identifier_limit,memory_kib,passes,lanes,slots,assertion_seconds)
        VALUES ($1,true,'test',$2,'fixture-security','fixture-product','fixture-audit',decode(repeat('37',32),'hex'),
        ARRAY['CONTACT_VERIFIED','KYC_REQUIRED','KYC_PENDING','KYC_APPROVED'],120,600,$3,$4,32,2,1,1,30)")
        .bind(RELEASE.parse::<Uuid>().unwrap()).bind(ORIGIN).bind(total).bind(per_id).execute(pool).await?;
    Ok(())
}
async fn runtime(pool: &PgPool) -> sqlx::Result<PgPool> {
    runtime_with_limits(pool, 200, 100).await
}
async fn runtime_with_limits(pool: &PgPool, total: i32, per_id: i32) -> sqlx::Result<PgPool> {
    sqlx::raw_sql(include_str!(
        "../../../../../infrastructure/postgres/phase-two-grants.psql"
    ))
    .execute(pool)
    .await?;
    auth_fixture(pool, total, per_id).await?;
    sqlx::raw_sql(super::SANDBOX).execute(pool).await?;
    auth_pool(pool).await
}
async fn auth_pool(pool: &PgPool) -> sqlx::Result<PgPool> {
    PgPoolOptions::new()
        .max_connections(5)
        .after_connect(|conn, _| {
            Box::pin(async move {
                sqlx::query("SET ROLE deypay_auth").execute(conn).await?;
                Ok(())
            })
        })
        .connect_with((*pool.connect_options()).clone())
        .await
}
fn store(pool: PgPool) -> PgSessions {
    PgSessions::new(pool, RELEASE.parse().unwrap(), [0x37; 32].into())
}
fn router<V: CredentialVerifier + Send + 'static>(pool: PgPool, verifier: V) -> Router {
    let auth = BrowserAuth::new(store(pool), verifier, BrowserOrigin::parse(ORIGIN).unwrap());
    async fn who(customer: AuthenticatedCustomer) -> String {
        customer.id().to_string()
    }
    async fn status(customer: AuthenticatedCustomer) -> String {
        customer.account_status().to_owned()
    }
    auth.routes()
        .merge(
            auth.protect(
                Router::new()
                    .route("/test/principal", get(who).post(who))
                    .route("/test/status", get(status)),
            ),
        )
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
    let app = router(api, hasher());
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
    let app = router(runtime(&pool).await?, hasher());
    let id = seed(&pool, "ada").await?;
    let expected = rejected(login(&app, "unknown@example.test", PW, None).await).await;
    // A customer may originate from a published enrollment or an import; consent scope is not login authority.
    sqlx::query(
        "WITH outside AS (INSERT INTO identity.users(email,email_verified_at,status)
        VALUES ('outside@example.test',now(),'CONTACT_VERIFIED') RETURNING id)
        INSERT INTO identity.password_credentials(user_id,password_hash) SELECT id,$1 FROM outside",
    )
    .bind(hasher().hash(PW).await.unwrap())
    .execute(&pool)
    .await?;
    assert_eq!(
        login(&app, "outside@example.test", PW, None).await.status(),
        StatusCode::OK
    );
    assert!(expected == rejected(login(&app, "ada@example.test", "Incorrect1!", None).await).await);
    for state in [
        "REGISTERED",
        "CONTACT_UNVERIFIED",
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
        &format!("$argon2id${}", "x".repeat(65_536)),
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
    assert!(sqlx::query_scalar::<_,bool>("SELECT NOT EXISTS(SELECT FROM identity.sessions s JOIN identity.users u ON u.id=s.user_id WHERE u.email='ada@example.test') AND EXISTS(SELECT FROM audit.audit_events WHERE action='LOGIN_REJECTED')").fetch_one(&pool).await?);
    Ok(())
}

#[sqlx::test(migrator = "MIGRATOR")]
async fn concurrent_refresh_is_single_use_and_preserves_absolute_deadline(
    pool: PgPool,
) -> sqlx::Result<()> {
    let app = router(runtime(&pool).await?, hasher());
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
    let app = router(runtime(&pool).await?, hasher());
    let id = seed(&pool, "ada").await?;
    for (expired_idle, expired_absolute, revoked) in [
        (true, false, false),
        (true, true, false),
        (false, false, true),
    ] {
        let token = SessionToken::generate().unwrap();
        sqlx::query("INSERT INTO identity.sessions(auth_release,user_id,token_hash,created_at,last_activity_at,expires_at,absolute_expires_at,revoked_at)
            SELECT 'ed000000-0000-4000-8000-000000000001',$1,$2,now()-interval '1 hour',now()-interval '1 hour',
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
    assert!(store(api.clone()).ready().await);
    assert!(!store(pool.clone()).ready().await);
    let app = router(api.clone(), hasher());
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
        "REVOKE EXECUTE ON FUNCTION identity.auth_credential(uuid,text,bigint,bytea) FROM deypay_auth",
    )
    .execute(&pool)
    .await?;
    assert!(!store(api.clone()).ready().await);
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
    let api = runtime_with_limits(&pool, 200, 1).await?;
    for (error, expected) in [
        (CredentialError::Busy, StatusCode::TOO_MANY_REQUESTS),
        (
            CredentialError::Unavailable,
            StatusCode::SERVICE_UNAVAILABLE,
        ),
    ] {
        let app = router(api.clone(), FailedVerifier(error));
        let response = login(
            &app,
            &format!("case{}@example.test", expected.as_u16()),
            PW,
            None,
        )
        .await;
        assert_eq!(response.status(), expected);
        assert!(response.headers().get(header::SET_COOKIE).is_none());
    }

    let app = router(api.clone(), hasher());
    rejected(login(&app, "new@example.test", PW, None).await).await;
    assert_eq!(
        login(&app, " NEW@example.test ", PW, None).await.status(),
        StatusCode::TOO_MANY_REQUESTS
    );

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
    let app = router(api.clone(), hasher());
    let id = seed(&pool, "ada").await?;
    let prior = pair(&login(&app, "ada@example.test", PW, None).await);
    let store = store(api);
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
            .issue(checked, "ada@example.test", &token.digest(), None)
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

#[sqlx::test(migrator = "MIGRATOR")]
async fn a_blocked_refresh_cannot_cross_the_idle_deadline(pool: PgPool) -> sqlx::Result<()> {
    let app = router(runtime(&pool).await?, hasher());
    seed(&pool, "ada").await?;
    let cookie = pair(&login(&app, "ada@example.test", PW, None).await);
    let mut tx = pool.begin().await?;
    sqlx::query(
        "UPDATE identity.sessions SET expires_at=clock_timestamp()+interval '150 milliseconds'",
    )
    .execute(&mut *tx)
    .await?;
    let mut attempt = std::pin::pin!(call(&app, post(REFRESH, String::new(), Some(&cookie))));
    assert!(
        tokio::time::timeout(std::time::Duration::from_millis(30), &mut attempt)
            .await
            .is_err()
    );
    // Wait against the same authoritative clock used by the SQL validation routine.
    sqlx::query("SELECT pg_sleep(greatest(0,extract(epoch FROM expires_at-clock_timestamp()))::double precision + 0.01) FROM identity.sessions")
        .execute(&mut *tx).await?;
    tx.commit().await?;
    rejected(attempt.await).await;
    Ok(())
}

#[sqlx::test(migrator = "MIGRATOR")]
async fn verified_lifecycle_states_expose_only_the_authenticated_identity_and_status(
    pool: PgPool,
) -> sqlx::Result<()> {
    let app = router(runtime(&pool).await?, hasher());
    let customer = seed(&pool, "ada").await?;
    for status in [
        "CONTACT_VERIFIED",
        "KYC_REQUIRED",
        "KYC_PENDING",
        "KYC_APPROVED",
    ] {
        sqlx::query("UPDATE identity.users SET status=$2 WHERE id=$1")
            .bind(customer)
            .bind(status)
            .execute(&pool)
            .await?;
        let cookie = pair(&login(&app, "ada@example.test", PW, None).await);
        let request = Request::get("/test/status")
            .header(header::COOKIE, &cookie)
            .header("x-account-status", "CLOSED")
            .body(Body::empty())
            .unwrap();
        let response = call(&app, request).await;
        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(super::bytes(response).await.as_ref(), status.as_bytes());
    }
    assert!(sqlx::query_scalar::<_,bool>("SELECT NOT EXISTS (SELECT FROM compliance.kyc_cases)
        AND NOT EXISTS (SELECT FROM wallets.wallet_profiles) AND NOT EXISTS (SELECT FROM payments.transactions)")
        .fetch_one(&pool).await?);
    Ok(())
}

#[sqlx::test(migrator = "MIGRATOR")]
async fn independent_replicas_share_admission_and_fail_closed_on_store_loss(
    pool: PgPool,
) -> sqlx::Result<()> {
    use deypay_application::login::LoginStore;
    let replica_a = runtime_with_limits(&pool, 3, 1).await?;
    let replica_b = auth_pool(&pool).await?;
    sqlx::query("INSERT INTO identity.auth_window(release,minute) VALUES($1,date_trunc(\x27minute\x27,now())+interval \x271 minute\x27)").bind(RELEASE.parse::<Uuid>().unwrap()).execute(&pool).await?;
    let a = store(replica_a);
    let b = store(replica_b);
    let (one, two) = tokio::join!(
        a.admit(Some(" unknown@example.test ")),
        b.admit(Some("UNKNOWN@example.test"))
    );
    assert!(matches!(
        (one, two),
        (Ok(()), Err(CredentialError::Busy)) | (Err(CredentialError::Busy), Ok(()))
    ));
    let attempts = tokio::join!(a.admit(None), b.admit(None), a.admit(None), b.admit(None));
    let results = [attempts.0, attempts.1, attempts.2, attempts.3];
    assert_eq!(results.iter().filter(|r| r.is_ok()).count(), 3);
    assert_eq!(
        results
            .iter()
            .filter(|r| matches!(r, Err(CredentialError::Busy)))
            .count(),
        1
    );
    assert!(a.admit(Some("second@example.test")).await.is_ok());
    assert!(b.admit(Some("third@example.test")).await.is_ok());
    assert_eq!(
        a.admit(Some("fourth@example.test")).await,
        Err(CredentialError::Busy)
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM identity.auth_identifier_window")
            .fetch_one(&pool)
            .await?,
        3
    );
    sqlx::raw_sql("UPDATE identity.auth_window SET minute=now()-interval '2 minutes';
        INSERT INTO identity.auth_receipt(release,nonce,valid_until) SELECT id,repeat('7',64),now()-interval '1 minute' FROM identity.auth_policy;").execute(&pool).await?;
    assert!(b.admit(None).await.is_ok());
    assert!(
        sqlx::query_scalar::<_, bool>(
            "SELECT NOT EXISTS(SELECT FROM identity.auth_identifier_window)
        AND NOT EXISTS(SELECT FROM identity.auth_receipt)"
        )
        .fetch_one(&pool)
        .await?
    );
    sqlx::query("REVOKE EXECUTE ON FUNCTION identity.auth_admit(uuid,text) FROM deypay_auth")
        .execute(&pool)
        .await?;
    assert_eq!(a.admit(None).await, Err(CredentialError::Unavailable));
    assert!(!b.ready().await);
    Ok(())
}

#[sqlx::test(migrator = "MIGRATOR")]
async fn startup_requires_matching_policy_environment_role_and_signer(
    pool: PgPool,
) -> sqlx::Result<()> {
    use deypay_common::config::Environment::{Production, Test};
    let auth = runtime(&pool).await?;
    let release = RELEASE.parse().unwrap();
    let good = store(auth.clone());
    let policy = good.policy(Test).await.unwrap();
    assert_eq!(policy.origin, ORIGIN);
    assert_eq!(policy.lifetime.idle_seconds(), 120);
    assert_eq!(policy.lifetime.absolute_seconds(), 600);
    assert!(good.ready().await);
    assert!(good.policy(Production).await.is_err());
    assert!(store(pool.clone()).policy(Test).await.is_err());
    assert!(
        PgSessions::new(auth.clone(), release, [0x11; 32].into())
            .policy(Test)
            .await
            .is_err()
    );
    assert!(
        PgSessions::new(auth, Uuid::nil(), [0x37; 32].into())
            .policy(Test)
            .await
            .is_err()
    );
    sqlx::query("UPDATE identity.auth_policy SET enabled=false WHERE id=$1")
        .bind(release)
        .execute(&pool)
        .await?;
    assert!(!good.ready().await);
    assert!(good.policy(Test).await.is_err());
    Ok(())
}

#[sqlx::test(migrator = "MIGRATOR")]
async fn auth_roles_cannot_substitute_customer_records_for_verifier_authorization(
    pool: PgPool,
) -> sqlx::Result<()> {
    let auth = runtime(&pool).await?;
    let victim = seed(&pool, "ada").await?;
    let app = router(auth.clone(), hasher());
    let real = pair(&login(&app, "ada@example.test", PW, None).await);
    let release = RELEASE.parse::<Uuid>().unwrap();
    // An identifier and even the authentic stored fingerprint confer no issuance authority.
    let fingerprint: String = sqlx::query_scalar(
        "SELECT encode(sha256(convert_to(password_hash,'UTF8')),'hex')
        FROM identity.password_credentials WHERE user_id=$1",
    )
    .bind(victim)
    .fetch_one(&pool)
    .await?;
    let generated = SessionToken::generate().unwrap();
    let issue = sqlx::query_scalar::<_, bool>(
        "SELECT identity.auth_issue($1,$2,$3,$4,$5,NULL,
        floor(extract(epoch FROM clock_timestamp()))::bigint,$6)",
    )
    .bind(release)
    .bind(victim)
    .bind("ada@example.test")
    .bind(fingerprint)
    .bind(generated.digest())
    .bind(vec![0u8; 32])
    .fetch_one(&auth)
    .await;
    assert!(!matches!(issue, Ok(true)));
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM identity.sessions")
            .fetch_one(&pool)
            .await?,
        1
    );
    assert_eq!(principal(&app, Some(&real)).await.status(), StatusCode::OK);
    for table in [
        "identity.auth_policy",
        "identity.password_credentials",
        "identity.sessions",
    ] {
        let has_access: bool =
            sqlx::query_scalar("SELECT has_any_column_privilege(current_user,$1,'SELECT')")
                .bind(table)
                .fetch_one(&auth)
                .await?;
        assert!(!has_access);
    }
    assert!(sqlx::query_scalar::<_,bool>("SELECT NOT has_function_privilege('deypay_app',
        'identity.auth_issue(uuid,uuid,text,text,text,text,bigint,bytea)','EXECUTE') AND
        to_regprocedure('identity.issue_sandbox_session(uuid,text,text,text,text,integer,integer)') IS NULL")
        .fetch_one(&pool).await?);
    let mut ordinary = pool.begin().await?;
    sqlx::query("SET LOCAL ROLE deypay_app")
        .execute(&mut *ordinary)
        .await?;
    assert!(sqlx::query("INSERT INTO audit.audit_events(actor_type,action,resource_type) VALUES ('SYSTEM','SESSION_ISSUED','SESSION')")
        .execute(&mut *ordinary).await.is_err());
    ordinary.rollback().await?;
    Ok(())
}

#[sqlx::test(migrator = "MIGRATOR")]
async fn issuance_proofs_expire_and_only_one_replica_can_consume_them(
    pool: PgPool,
) -> sqlx::Result<()> {
    let a = runtime(&pool).await?;
    let b = auth_pool(&pool).await?;
    let id = seed(&pool, "ada").await?;
    let release = RELEASE.parse::<Uuid>().unwrap();
    let token = SessionToken::generate().unwrap().digest();
    let fingerprint:String=sqlx::query_scalar("SELECT identity.auth_hex(password_hash) FROM identity.password_credentials WHERE user_id=$1")
        .bind(id).fetch_one(&pool).await?;
    let now: i64 =
        sqlx::query_scalar("SELECT floor(extract(epoch FROM clock_timestamp()))::bigint")
            .fetch_one(&pool)
            .await?;
    // The fixture owner signs test vectors. Neither runtime may retrieve this key.
    let sign = |purpose: &'static str, at: i64| {
        sqlx::query_scalar::<_,Vec<u8>>(
        "SELECT auth_crypto.hmac(convert_to(array_to_string(ARRAY['deypay-auth-v1',$2,id::text,$3::bigint::text,
         $4::uuid::text,identity.auth_hex('ada@example.test'),$5,$6,''],chr(10)),'UTF8'),signing_key,'sha256')
         FROM identity.auth_policy WHERE id=$1")
        .bind(release).bind(purpose).bind(at).bind(id).bind(fingerprint.clone()).bind(token.clone()).fetch_one(&pool)
    };
    let issue = |at: i64, tag: Vec<u8>, customer: Uuid| {
        sqlx::query_scalar::<_, bool>(
            "SELECT identity.auth_issue($1,$2,'ada@example.test',$3,$4,NULL,$5,$6)",
        )
        .bind(release)
        .bind(customer)
        .bind(fingerprint.clone())
        .bind(token.clone())
        .bind(at)
        .bind(tag)
    };
    for (at, purpose, customer) in [
        (now - 90, "issue", id),
        (now, "lookup", id),
        (now, "issue", Uuid::nil()),
    ] {
        assert!(
            issue(at, sign(purpose, at).await?, customer)
                .fetch_one(&a)
                .await
                .is_err()
        );
    }
    let tag = sign("issue", now).await?;
    let (x, y) = tokio::join!(
        issue(now, tag.clone(), id).fetch_one(&a),
        issue(now, tag.clone(), id).fetch_one(&b)
    );
    assert_ne!(x?, y?);
    // Rotation changes the live token, but the original issuance nonce remains spent.
    use deypay_application::login::SessionStore;
    assert!(
        store(a.clone())
            .validate(&token, Some(&SessionToken::generate().unwrap().digest()))
            .await
            .unwrap()
            .is_some()
    );
    assert!(!issue(now, tag, id).fetch_one(&b).await?);
    assert!(sqlx::query_scalar::<_,bool>("SELECT (SELECT count(*) FROM identity.sessions)=1
        AND (SELECT count(*) FROM identity.auth_receipt)=1 AND (SELECT count(*) FROM audit.audit_events WHERE action='SESSION_ISSUED')=1")
        .fetch_one(&pool).await?);
    Ok(())
}

#[sqlx::test(migrator = "MIGRATOR")]
async fn general_api_only_validates_and_release_retirement_invalidates_all_replicas(
    pool: PgPool,
) -> sqlx::Result<()> {
    use deypay_application::login::SessionStore;
    use deypay_infrastructure::sessions::PgSessionReader;
    let auth = runtime(&pool).await?;
    let app = router(auth.clone(), hasher());
    let id = seed(&pool, "ada").await?;
    let cookie = pair(&login(&app, "ada@example.test", PW, None).await);
    let token = SessionToken::from_cookie(cookie.split_once('=').unwrap().1)
        .unwrap()
        .digest();
    let general = PgPoolOptions::new()
        .after_connect(|c, _| {
            Box::pin(async move {
                sqlx::query("SET ROLE deypay_app").execute(c).await?;
                Ok(())
            })
        })
        .connect_with((*pool.connect_options()).clone())
        .await?;
    let reader = PgSessionReader {
        pool: general.clone(),
        release: RELEASE.parse().unwrap(),
        environment: "test",
        origin: ORIGIN.to_owned(),
    };
    assert!(reader.ready().await);
    let principal = reader.validate(&token, None).await.unwrap().unwrap();
    assert_eq!(
        (principal.customer_id, principal.account_status.as_str()),
        (id, "CONTACT_VERIFIED")
    );
    assert!(reader.validate(&token, Some(&token)).await.is_err());
    assert!(
        store(general.clone())
            .credential("ada@example.test")
            .await
            .is_err()
    );
    use deypay_application::login::LoginStore;
    assert!(
        sqlx::query("SELECT token_hash FROM identity.sessions")
            .execute(&general)
            .await
            .is_err()
    );
    sqlx::query("UPDATE identity.users SET status='KYC_PENDING' WHERE id=$1")
        .bind(id)
        .execute(&pool)
        .await?;
    assert!(reader.validate(&token, None).await.unwrap().is_none());
    let rotated = pair(&call(&app, post(REFRESH, String::new(), Some(&cookie))).await);
    let next = SessionToken::from_cookie(rotated.split_once('=').unwrap().1)
        .unwrap()
        .digest();
    assert_eq!(
        reader
            .validate(&next, None)
            .await
            .unwrap()
            .unwrap()
            .account_status,
        "KYC_PENDING"
    );
    sqlx::query("UPDATE identity.auth_policy SET enabled=false")
        .execute(&pool)
        .await?;
    assert!(!reader.ready().await);
    assert!(!store(auth).ready().await);
    assert!(reader.validate(&next, None).await.is_err());
    assert!(
        sqlx::query("UPDATE identity.auth_policy SET enabled=true")
            .execute(&pool)
            .await
            .is_err()
    );
    Ok(())
}

#[sqlx::test(migrations = false)]
async fn forward_upgrade_retires_legacy_sessions_and_capabilities(
    pool: PgPool,
) -> sqlx::Result<()> {
    let mut migrations = MIGRATOR
        .iter()
        .filter(|m| m.version <= 202611250502)
        .peekable();
    while let Some(m) = migrations.next() {
        if migrations.peek().is_none() {
            sqlx::raw_sql("GRANT EXECUTE ON FUNCTION identity.sandbox_login_credential(text) TO deypay_app;
                WITH customer AS (INSERT INTO identity.users(email,status) VALUES('old@example.test','REGISTERED') RETURNING id)
                INSERT INTO identity.sessions(user_id,token_hash,expires_at,absolute_expires_at)
                SELECT id,repeat('c',64),now()+interval '1 hour',now()+interval '1 day' FROM customer").execute(&pool).await?;
        }
        let mut tx = pool.begin().await?;
        sqlx::raw_sql(&m.sql).execute(&mut *tx).await?;
        tx.commit().await?;
    }
    assert!(sqlx::query_scalar::<_,bool>("SELECT NOT EXISTS(SELECT FROM identity.sessions WHERE revoked_at IS NULL)
        AND EXISTS(SELECT FROM audit.audit_events WHERE action='SESSION_REVOKED' AND reason='AUTHORITY_UPGRADE')
        AND to_regprocedure('identity.sandbox_login_credential(text)') IS NULL
        AND NOT EXISTS(SELECT FROM identity.auth_policy)").fetch_one(&pool).await?);
    Ok(())
}

#[sqlx::test(migrator = "MIGRATOR")]
async fn hosted_configuration_requires_independent_database_authority_and_unique_release_key(
    pool: PgPool,
) -> sqlx::Result<()> {
    use deypay_common::config::Environment::Staging;
    let switched = runtime(&pool).await?;
    sqlx::query("UPDATE identity.auth_policy SET enabled=false")
        .execute(&pool)
        .await?;
    let next = Uuid::parse_str("ed000000-0000-4000-8000-000000000002").unwrap();
    let install = |byte: &'static str| {
        sqlx::query("INSERT INTO identity.auth_policy
        SELECT $1,true,'staging',origin,security_ref,product_ref,audit_ref,decode(repeat($2,32),'hex'),allowed_states,
          idle_seconds,absolute_seconds,total_limit,identifier_limit,memory_kib,passes,lanes,slots,assertion_seconds
        FROM identity.auth_policy WHERE id=$3").bind(next).bind(byte).bind(RELEASE.parse::<Uuid>().unwrap())
    };
    assert!(install("37").execute(&pool).await.is_err());
    install("48").execute(&pool).await?;
    assert!(
        PgSessions::new(switched, next, [0x48; 32].into())
            .policy(Staging)
            .await
            .is_err()
    );
    // Simulate the actual authenticated PostgreSQL principal, rather than an
    // administrator using SET ROLE. TLS/ingress certification is a deployment check.
    let isolated = PgPoolOptions::new()
        .after_connect(|c, _| {
            Box::pin(async move {
                sqlx::query("SET SESSION AUTHORIZATION deypay_auth")
                    .execute(c)
                    .await?;
                Ok(())
            })
        })
        .connect_with((*pool.connect_options()).clone())
        .await?;
    let verifier = PgSessions::new(isolated, next, [0x48; 32].into());
    assert!(verifier.policy(Staging).await.is_ok());
    assert!(verifier.ready().await);
    assert!(
        sqlx::query_scalar::<_, bool>("SELECT identity.auth_owner_ok() AND identity.auth_acl_ok()")
            .fetch_one(&pool)
            .await?
    );
    Ok(())
}
