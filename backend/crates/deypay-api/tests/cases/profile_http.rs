//! Customer profile, preferences, status and profile-image HTTP tests
//! (IMP-P3-009/010). Sessions come from the merged login, logout and
//! revocation routes; persistence runs as the restricted runtime role.
use super::login_sessions::{RELEASE, auth_fixture};
use axum::{
    Router,
    body::{Body, to_bytes},
    http::{Method, Request, StatusCode, header},
    response::Response,
};
use deypay_api::{login::BrowserAuth, profile, sessions::BrowserOrigin};
use deypay_application::passwords::PasswordHasher;
use deypay_infrastructure::{
    MIGRATOR,
    password_hashing::Argon2idHasher,
    sessions::{PgSessions, SessionToken},
};
use image::{DynamicImage, ImageBuffer, ImageFormat, Rgb, Rgba};
use serde_json::{Value, json};
use sqlx::{PgPool, postgres::PgPoolOptions};
use std::io::Cursor;
use tower::ServiceExt;
use uuid::Uuid;

const ORIGIN: &str = "https://sandbox.example.test";
const PASSWORD: &str = "Profile1!password";
const ME: &str = "/api/v1/me";
const PREFERENCES: &str = "/api/v1/me/preferences";
const STATUS: &str = "/api/v1/me/status";
const IMAGE: &str = "/api/v1/me/profile-image";

fn hasher() -> Argon2idHasher {
    Argon2idHasher::new(32, 2, 1, 4).unwrap()
}

pub(super) async fn role_pool(pool: &PgPool, statement: &'static str) -> sqlx::Result<PgPool> {
    PgPoolOptions::new()
        .max_connections(8)
        .after_connect(move |conn, _| {
            Box::pin(async move {
                sqlx::query(statement).execute(conn).await?;
                Ok(())
            })
        })
        .connect_with((*pool.connect_options()).clone())
        .await
}

pub(super) async fn grants(pool: &PgPool) -> sqlx::Result<()> {
    for script in [
        include_str!("../../../../../infrastructure/postgres/phase-two-grants.psql"),
        include_str!("../../../../../infrastructure/postgres/profile-grants.psql"),
        super::SANDBOX,
    ] {
        sqlx::raw_sql(script).execute(pool).await?;
    }
    auth_fixture(pool, 500, 200).await
}

/// The merged verifier routes issue, rotate and revoke sessions. Profile
/// routes are mounted behind the same session guard as production.
async fn app(pool: &PgPool) -> sqlx::Result<Router> {
    grants(pool).await?;
    let verifier = role_pool(pool, "SET ROLE deypay_auth").await?;
    let runtime = role_pool(pool, "SET ROLE deypay_app").await?;
    let auth = BrowserAuth::new(
        PgSessions::new(verifier, RELEASE.parse().unwrap(), [0x37; 32].into()),
        hasher(),
        BrowserOrigin::parse(ORIGIN).unwrap(),
    );
    Ok(auth
        .routes()
        .merge(auth.protect(profile::routes(profile::ProfileApi::new(runtime)))))
}

pub(super) async fn customer(pool: &PgPool, name: &str) -> sqlx::Result<Uuid> {
    let email = format!("{name}@example.test");
    let hash = hasher().hash(PASSWORD).await.unwrap();
    sqlx::query("SELECT identity.register_customer($1,'+2348012345678',$2,$3,'Ada','Okafor','NG','1.0','1.0',true,true)")
        .bind(&email).bind(name).bind(hash).execute(pool).await?;
    sqlx::query_scalar(
        "UPDATE identity.users SET email_verified_at=clock_timestamp() WHERE email=$1 RETURNING id",
    )
    // Registration stores the normalized address.
    .bind(email.to_lowercase())
    .fetch_one(pool)
    .await
}

async fn send(app: &Router, req: Request<Body>) -> Response {
    app.clone().oneshot(req).await.unwrap()
}

fn request(method: Method, path: &str, cookie: Option<&str>) -> axum::http::request::Builder {
    let mut req = Request::builder().method(method.clone()).uri(path);
    if method != Method::GET {
        req = req.header(header::ORIGIN, ORIGIN);
    }
    if let Some(cookie) = cookie {
        req = req.header(header::COOKIE, cookie);
    }
    req
}

pub(super) async fn login(app: &Router, name: &str) -> String {
    let body = json!({"identifier": format!("{name}@example.test"), "password": PASSWORD});
    let res = send(
        app,
        request(Method::POST, "/api/v1/auth/login", None)
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from(body.to_string()))
            .unwrap(),
    )
    .await;
    assert_eq!(res.status(), StatusCode::OK);
    let cookie = res.headers()[header::SET_COOKIE].to_str().unwrap();
    cookie.split_once("; ").unwrap().0.to_owned()
}

async fn json_body(res: Response) -> Value {
    serde_json::from_slice(&to_bytes(res.into_body(), 1 << 20).await.unwrap()).unwrap()
}

async fn get(app: &Router, path: &str, cookie: Option<&str>) -> (StatusCode, Value) {
    let res = send(
        app,
        request(Method::GET, path, cookie)
            .body(Body::empty())
            .unwrap(),
    )
    .await;
    let status = res.status();
    assert_eq!(res.headers()[header::CACHE_CONTROL], "no-store");
    (status, json_body(res).await)
}

async fn patch(app: &Router, path: &str, cookie: &str, body: Value) -> (StatusCode, Value) {
    let res = send(
        app,
        request(Method::PATCH, path, Some(cookie))
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from(body.to_string()))
            .unwrap(),
    )
    .await;
    let status = res.status();
    assert_eq!(res.headers()[header::CACHE_CONTROL], "no-store");
    (status, json_body(res).await)
}

async fn put_image(app: &Router, cookie: &str, content_type: &str, bytes: Vec<u8>) -> Response {
    send(
        app,
        request(Method::PUT, IMAGE, Some(cookie))
            .header(header::CONTENT_TYPE, content_type)
            .body(Body::from(bytes))
            .unwrap(),
    )
    .await
}

async fn get_image(app: &Router, cookie: &str, etag: Option<&str>) -> Response {
    let mut req = request(Method::GET, IMAGE, Some(cookie));
    if let Some(etag) = etag {
        req = req.header(header::IF_NONE_MATCH, etag);
    }
    send(app, req.body(Body::empty()).unwrap()).await
}

async fn delete_image(app: &Router, cookie: &str) -> StatusCode {
    send(
        app,
        request(Method::DELETE, IMAGE, Some(cookie))
            .body(Body::empty())
            .unwrap(),
    )
    .await
    .status()
}

fn encode(image: DynamicImage, format: ImageFormat) -> Vec<u8> {
    let mut out = Cursor::new(Vec::new());
    image.write_to(&mut out, format).unwrap();
    out.into_inner()
}

fn photo(width: u32, height: u32, shade: u8) -> DynamicImage {
    DynamicImage::ImageRgb8(ImageBuffer::from_fn(width, height, |x, y| {
        Rgb([(x % 256) as u8, (y % 256) as u8, shade])
    }))
}

fn error_code(body: &Value) -> (&str, &Value) {
    (
        body["error"]["code"].as_str().unwrap(),
        &body["error"]["details"],
    )
}

async fn audit_actions(pool: &PgPool, customer: Uuid) -> sqlx::Result<Vec<String>> {
    sqlx::query_scalar(
        "SELECT action FROM audit.audit_events WHERE actor_id=$1 AND action NOT IN
            ('LOGIN_SUCCEEDED','SESSION_ISSUED','SESSION_REFRESHED','SESSION_REVOKED','CUSTOMER_REGISTERED')
         ORDER BY created_at, action",
    )
    .bind(customer)
    .fetch_all(pool)
    .await
}

#[sqlx::test(migrator = "MIGRATOR")]
async fn profile_routes_reject_missing_expired_logged_out_and_revoked_sessions(
    pool: PgPool,
) -> sqlx::Result<()> {
    let app = app(&pool).await?;
    let ada = customer(&pool, "ada").await?;
    let unauthorized = |(status, body): (StatusCode, Value)| {
        assert_eq!(status, StatusCode::UNAUTHORIZED);
        assert_eq!(body["error"]["code"], "AUTHENTICATION_REQUIRED");
    };
    for path in [ME, PREFERENCES, STATUS] {
        unauthorized(get(&app, path, None).await);
        unauthorized(get(&app, path, Some("__Host-deypay_session=nonsense")).await);
        let unknown = format!(
            "__Host-deypay_session={}",
            SessionToken::generate().unwrap().cookie_value().as_str()
        );
        unauthorized(get(&app, path, Some(&unknown)).await);
    }
    // Idle-expired, absolutely expired and revoked sessions.
    for (idle, absolute, revoked) in [
        (true, false, false),
        (true, true, false),
        (false, false, true),
    ] {
        let token = SessionToken::generate().unwrap();
        sqlx::query("INSERT INTO identity.sessions(auth_release,user_id,token_hash,created_at,last_activity_at,expires_at,absolute_expires_at,revoked_at)
            SELECT $1,$2,$3,now()-interval '1 hour',now()-interval '1 hour',
            now()+CASE WHEN $4 THEN interval '-1 second' ELSE interval '1 minute' END,
            now()+CASE WHEN $5 THEN interval '-1 second' ELSE interval '1 day' END,CASE WHEN $6 THEN now() END")
            .bind(RELEASE.parse::<Uuid>().unwrap()).bind(ada).bind(token.digest()).bind(idle).bind(absolute).bind(revoked).execute(&pool).await?;
        let cookie = format!("__Host-deypay_session={}", token.cookie_value().as_str());
        unauthorized(get(&app, ME, Some(&cookie)).await);
        assert_eq!(delete_image(&app, &cookie).await, StatusCode::UNAUTHORIZED);
    }
    // Logout through the merged verifier route ends profile access.
    let cookie = login(&app, "ada").await;
    assert_eq!(get(&app, ME, Some(&cookie)).await.0, StatusCode::OK);
    let res = send(
        &app,
        request(Method::POST, "/api/v1/auth/logout", Some(&cookie))
            .body(Body::empty())
            .unwrap(),
    )
    .await;
    assert_eq!(res.status(), StatusCode::OK);
    unauthorized(get(&app, ME, Some(&cookie)).await);
    unauthorized(
        patch(
            &app,
            PREFERENCES,
            &cookie,
            json!({"push_notifications": true}),
        )
        .await,
    );
    // Targeted revocation from another session ends profile access.
    let victim = login(&app, "ada").await;
    let controller = login(&app, "ada").await;
    let digest = SessionToken::from_cookie(victim.split_once('=').unwrap().1)
        .unwrap()
        .digest();
    let target: Uuid = sqlx::query_scalar("SELECT id FROM identity.sessions WHERE token_hash=$1")
        .bind(digest)
        .fetch_one(&pool)
        .await?;
    let res = send(
        &app,
        request(
            Method::DELETE,
            &format!("/api/v1/auth/sessions/{target}"),
            Some(&controller),
        )
        .body(Body::empty())
        .unwrap(),
    )
    .await;
    assert_eq!(res.status(), StatusCode::OK);
    unauthorized(get(&app, STATUS, Some(&victim)).await);
    assert_eq!(get(&app, STATUS, Some(&controller)).await.0, StatusCode::OK);
    // An account status change revokes the session before a profile write.
    sqlx::query("UPDATE identity.users SET status='SUSPENDED' WHERE id=$1")
        .bind(ada)
        .execute(&pool)
        .await?;
    unauthorized(patch(&app, ME, &controller, json!({"first_name": "Changed"})).await);
    let first: String =
        sqlx::query_scalar("SELECT first_name FROM customers.profiles WHERE user_id=$1")
            .bind(ada)
            .fetch_one(&pool)
            .await?;
    assert_eq!(first, "Ada");
    // Customer profile writes need the configured origin.
    let bob = customer(&pool, "bob").await?;
    let cookie = login(&app, "bob").await;
    for origin in [None, Some("https://elsewhere.test")] {
        let mut req = Request::builder()
            .method(Method::PATCH)
            .uri(ME)
            .header(header::COOKIE, &cookie)
            .header(header::CONTENT_TYPE, "application/json");
        if let Some(origin) = origin {
            req = req.header(header::ORIGIN, origin);
        }
        let res = send(
            &app,
            req.body(Body::from(r#"{"first_name":"Forged"}"#)).unwrap(),
        )
        .await;
        assert_eq!(res.status(), StatusCode::FORBIDDEN);
    }
    assert!(audit_actions(&pool, bob).await?.is_empty());
    Ok(())
}

#[sqlx::test(migrator = "MIGRATOR")]
async fn customers_read_and_edit_only_their_own_profile(pool: PgPool) -> sqlx::Result<()> {
    let app = app(&pool).await?;
    let ada = customer(&pool, "Ada_Okafor").await?;
    let bob = customer(&pool, "bob").await?;
    let cookie = login(&app, "Ada_Okafor").await;
    let (status, body) = get(&app, ME, Some(&cookie)).await;
    assert_eq!(status, StatusCode::OK);
    let data = body["data"].as_object().unwrap();
    let mut keys: Vec<_> = data.keys().map(String::as_str).collect();
    keys.sort_unstable();
    assert_eq!(
        keys,
        [
            "account_status",
            "country_code",
            "created_at",
            "email",
            "email_verified",
            "first_name",
            "id",
            "last_name",
            "names_editable",
            "phone",
            "phone_verified",
            "profile_image",
            "updated_at",
            "username"
        ]
    );
    assert_eq!(body["data"]["id"], ada.to_string());
    assert_eq!(body["data"]["username"], "Ada_Okafor");
    assert_eq!(body["data"]["email"], "ada_okafor@example.test");
    assert_eq!(body["data"]["email_verified"], true);
    assert_eq!(body["data"]["phone"], "+2348012345678");
    assert_eq!(body["data"]["phone_verified"], false);
    assert_eq!(body["data"]["account_status"], "contact_verified");
    assert_eq!(body["data"]["names_editable"], true);
    assert_eq!(body["data"]["profile_image"], Value::Null);
    assert!(body["data"]["created_at"].as_str().unwrap().ends_with('Z'));
    let text = body.to_string();
    for secret in ["argon2", "password", "token", "kyc", "risk", "session"] {
        assert!(!text.to_ascii_lowercase().contains(secret), "{secret}");
    }
    assert_eq!(
        get(&app, STATUS, Some(&cookie)).await.1,
        json!({"data": {"account_status": "contact_verified"}})
    );

    // Client-supplied identity is ignored or refused.
    let spoofed = send(
        &app,
        request(
            Method::GET,
            &format!("{ME}?customer_id={bob}&id={bob}"),
            Some(&cookie),
        )
        .header("x-customer-id", bob.to_string())
        .header("x-user-id", bob.to_string())
        .body(Body::empty())
        .unwrap(),
    )
    .await;
    assert_eq!(json_body(spoofed).await["data"]["id"], ada.to_string());
    for forbidden in [
        json!({"id": bob.to_string()}),
        json!({"customer_id": bob.to_string(), "first_name": "X"}),
        json!({"email": "taken@example.test"}),
        json!({"email_verified": false}),
        json!({"phone": "+2348099999999"}),
        json!({"username": "new_name"}),
        json!({"account_status": "kyc_approved"}),
        json!({"status": "KYC_APPROVED"}),
        json!({"kyc_status": "approved"}),
        json!({"country_code": "GH"}),
        json!({"password": "Another1!password"}),
        json!({"names_editable": true}),
        json!({"first_name": null}),
        json!({"first_name": 7}),
        json!({}),
        json!([]),
        json!("first_name"),
    ] {
        let (status, body) = patch(&app, ME, &cookie, forbidden.clone()).await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{forbidden}");
        assert_eq!(error_code(&body).0, "VALIDATION_ERROR");
    }
    let wrong_type = send(
        &app,
        request(Method::PATCH, ME, Some(&cookie))
            .header(header::CONTENT_TYPE, "text/plain")
            .body(Body::from(r#"{"first_name":"Ade"}"#))
            .unwrap(),
    )
    .await;
    assert_eq!(wrong_type.status(), StatusCode::BAD_REQUEST);
    for (bad, fields) in [
        (json!({"first_name": "  "}), json!(["first_name"])),
        (json!({"last_name": "A\u{200b}da"}), json!(["last_name"])),
        (
            json!({"first_name": "x".repeat(101), "last_name": "\u{202e}x"}),
            json!(["first_name", "last_name"]),
        ),
    ] {
        let (status, body) = patch(&app, ME, &cookie, bad).await;
        assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
        assert_eq!(
            error_code(&body),
            ("VALIDATION_ERROR", &json!({"fields": fields}))
        );
    }
    let untouched: (String, String, String, String) = sqlx::query_as(
        "SELECT p.first_name, p.last_name, u.email, u.status FROM customers.profiles p
         JOIN identity.users u ON u.id=p.user_id WHERE u.id=$1",
    )
    .bind(ada)
    .fetch_one(&pool)
    .await?;
    assert_eq!(
        untouched,
        (
            "Ada".into(),
            "Okafor".into(),
            "ada_okafor@example.test".into(),
            "CONTACT_VERIFIED".into()
        )
    );
    assert!(audit_actions(&pool, ada).await?.is_empty());

    // Valid edits are trimmed, partial, owner-scoped and audited without values.
    let (status, body) = patch(
        &app,
        ME,
        &cookie,
        json!({"first_name": "  Adaeze ", "last_name": "Okafor-Eze"}),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        (
            body["data"]["first_name"].as_str(),
            body["data"]["last_name"].as_str()
        ),
        (Some("Adaeze"), Some("Okafor-Eze"))
    );
    let (_, body) = patch(&app, ME, &cookie, json!({"last_name": "O'Neil"})).await;
    assert_eq!(
        (
            body["data"]["first_name"].as_str(),
            body["data"]["last_name"].as_str()
        ),
        (Some("Adaeze"), Some("O'Neil"))
    );
    let (_, body) = patch(&app, ME, &cookie, json!({"last_name": "O'Neil"})).await;
    assert_eq!(body["data"]["last_name"], "O'Neil");
    let metadata: Vec<Value> = sqlx::query_scalar(
        "SELECT metadata FROM audit.audit_events WHERE actor_id=$1 AND action='PROFILE_NAME_CHANGED' ORDER BY created_at",
    )
    .bind(ada)
    .fetch_all(&pool)
    .await?;
    assert_eq!(
        metadata,
        [
            json!({"fields": ["first_name", "last_name"]}),
            json!({"fields": ["last_name"]})
        ]
    );
    let bob_names: (String, String) =
        sqlx::query_as("SELECT first_name, last_name FROM customers.profiles WHERE user_id=$1")
            .bind(bob)
            .fetch_one(&pool)
            .await?;
    assert_eq!(bob_names, ("Ada".into(), "Okafor".into()));

    // Names lock once identity verification starts. A DRAFT case does not lock them.
    let case: Uuid = sqlx::query_scalar(
        "INSERT INTO compliance.kyc_cases(user_id,status) VALUES ($1,'DRAFT') RETURNING id",
    )
    .bind(ada)
    .fetch_one(&pool)
    .await?;
    assert_eq!(
        patch(&app, ME, &cookie, json!({"first_name": "Ada"}))
            .await
            .0,
        StatusCode::OK
    );
    sqlx::query(
        "UPDATE compliance.kyc_cases SET status='SUBMITTED', submitted_at=now() WHERE id=$1",
    )
    .bind(case)
    .execute(&pool)
    .await?;
    assert_eq!(
        get(&app, ME, Some(&cookie)).await.1["data"]["names_editable"],
        false
    );
    let (status, body) = patch(&app, ME, &cookie, json!({"first_name": "Changed"})).await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(
        error_code(&body),
        ("VALIDATION_ERROR", &json!({"reason": "names_locked"}))
    );
    let first: String =
        sqlx::query_scalar("SELECT first_name FROM customers.profiles WHERE user_id=$1")
            .bind(ada)
            .fetch_one(&pool)
            .await?;
    assert_eq!(first, "Ada");
    Ok(())
}

#[sqlx::test(migrator = "MIGRATOR")]
async fn preferences_start_from_approved_defaults_and_merge_changes(
    pool: PgPool,
) -> sqlx::Result<()> {
    let app = app(&pool).await?;
    let ada = customer(&pool, "ada").await?;
    let bob = customer(&pool, "bob").await?;
    let cookie = login(&app, "ada").await;
    let defaults = json!({"data": {"phone_discoverable": false, "email_notifications": true,
        "push_notifications": false, "marketing_consent": false, "updated_at": null}});
    assert_eq!(
        get(&app, PREFERENCES, Some(&cookie)).await,
        (StatusCode::OK, defaults.clone())
    );
    for bad in [
        json!({}),
        json!({"email_notifications": "false"}),
        json!({"email_notifications": null}),
        json!({"sms_notifications": true}),
        json!({"user_id": bob.to_string(), "push_notifications": true}),
    ] {
        assert_eq!(
            patch(&app, PREFERENCES, &cookie, bad).await.0,
            StatusCode::BAD_REQUEST
        );
    }
    // API §16.5: discovery needs a verified phone; MVP phones are unverified.
    let (status, body) = patch(
        &app,
        PREFERENCES,
        &cookie,
        json!({"phone_discoverable": true, "push_notifications": true}),
    )
    .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(
        error_code(&body),
        ("VALIDATION_ERROR", &json!({"reason": "phone_unverified"}))
    );
    assert_eq!(get(&app, PREFERENCES, Some(&cookie)).await.1, defaults);

    let (status, body) = patch(
        &app,
        PREFERENCES,
        &cookie,
        json!({"email_notifications": false, "marketing_consent": true}),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["data"]["email_notifications"], false);
    assert_eq!(body["data"]["marketing_consent"], true);
    assert_eq!(body["data"]["push_notifications"], false);
    assert!(body["data"]["updated_at"].as_str().unwrap().ends_with('Z'));
    let (_, body) = patch(
        &app,
        PREFERENCES,
        &cookie,
        json!({"marketing_consent": false}),
    )
    .await;
    assert_eq!(
        (
            body["data"]["email_notifications"].as_bool(),
            body["data"]["marketing_consent"].as_bool()
        ),
        (Some(false), Some(false))
    );
    // An unchanged value writes no audit event.
    patch(
        &app,
        PREFERENCES,
        &cookie,
        json!({"marketing_consent": false}),
    )
    .await;
    let changes: Vec<Value> = sqlx::query_scalar(
        "SELECT metadata FROM audit.audit_events WHERE actor_id=$1 AND action='PREFERENCES_CHANGED' ORDER BY created_at",
    )
    .bind(ada)
    .fetch_all(&pool)
    .await?;
    let wording = json!({"document_id": "DEP-LEG-003", "version": "1.0"});
    assert_eq!(
        changes,
        [
            json!({"email_notifications": false, "marketing_consent": true, "consent_wording": wording.clone()}),
            json!({"marketing_consent": false, "consent_wording": wording}),
        ]
    );
    assert_eq!(
        get(&app, PREFERENCES, Some(&login(&app, "bob").await))
            .await
            .1,
        defaults
    );

    // A verified phone may opt in to discovery.
    sqlx::query("UPDATE identity.users SET phone_verified_at=now() WHERE id=$1")
        .bind(ada)
        .execute(&pool)
        .await?;
    let cookie = login(&app, "ada").await;
    let (status, body) = patch(
        &app,
        PREFERENCES,
        &cookie,
        json!({"phone_discoverable": true}),
    )
    .await;
    assert_eq!(
        (status, body["data"]["phone_discoverable"].as_bool()),
        (StatusCode::OK, Some(true))
    );
    Ok(())
}

#[sqlx::test(migrator = "MIGRATOR")]
async fn concurrent_edits_are_atomic_and_lose_no_other_field(pool: PgPool) -> sqlx::Result<()> {
    let app = app(&pool).await?;
    let ada = customer(&pool, "ada").await?;
    let cookie = login(&app, "ada").await;
    let mut edits = tokio::task::JoinSet::new();
    for i in 0..12 {
        let (app, cookie) = (app.clone(), cookie.clone());
        edits.spawn(async move {
            let pair = format!("Pair{i}");
            patch(
                &app,
                ME,
                &cookie,
                json!({"first_name": pair, "last_name": pair}),
            )
            .await
            .0
        });
    }
    for (i, field) in [
        "email_notifications",
        "push_notifications",
        "marketing_consent",
    ]
    .into_iter()
    .enumerate()
    {
        let (app, cookie) = (app.clone(), cookie.clone());
        edits.spawn(async move {
            let value = i != 0;
            patch(&app, PREFERENCES, &cookie, json!({field: value}))
                .await
                .0
        });
    }
    while let Some(status) = edits.join_next().await {
        assert_eq!(status.unwrap(), StatusCode::OK);
    }
    let (first, last): (String, String) =
        sqlx::query_as("SELECT first_name, last_name FROM customers.profiles WHERE user_id=$1")
            .bind(ada)
            .fetch_one(&pool)
            .await?;
    assert_eq!(first, last, "a rename never mixes two requests");
    let (_, body) = get(&app, PREFERENCES, Some(&cookie)).await;
    assert_eq!(
        (
            body["data"]["email_notifications"].as_bool(),
            body["data"]["push_notifications"].as_bool(),
            body["data"]["marketing_consent"].as_bool()
        ),
        (Some(false), Some(true), Some(true))
    );
    Ok(())
}

#[sqlx::test(migrator = "MIGRATOR")]
async fn profile_images_are_uploaded_served_replaced_and_removed_by_their_owner(
    pool: PgPool,
) -> sqlx::Result<()> {
    let app = app(&pool).await?;
    let ada = customer(&pool, "ada").await?;
    customer(&pool, "bob").await?;
    let cookie = login(&app, "ada").await;
    let bob = login(&app, "bob").await;
    let transparent = DynamicImage::ImageRgba8(ImageBuffer::from_fn(1600, 800, |x, y| {
        Rgba([
            (x % 256) as u8,
            (y % 256) as u8,
            50,
            if x < 800 { 0 } else { 255 },
        ])
    }));
    let res = put_image(
        &app,
        &cookie,
        "image/png",
        encode(transparent, ImageFormat::Png),
    )
    .await;
    assert_eq!(res.status(), StatusCode::OK);
    assert_eq!(res.headers()[header::CACHE_CONTROL], "no-store");
    let first = json_body(res).await["data"]["profile_image"].clone();
    assert_eq!(
        (
            first["content_type"].as_str(),
            first["width"].as_u64(),
            first["height"].as_u64()
        ),
        (Some("image/jpeg"), Some(512), Some(256))
    );
    let version = first["version"].as_str().unwrap().to_owned();
    assert_eq!(
        get(&app, ME, Some(&cookie)).await.1["data"]["profile_image"],
        first
    );

    let res = get_image(&app, &cookie, None).await;
    assert_eq!(res.status(), StatusCode::OK);
    let etag = format!("\"{version}\"");
    for (name, value) in [
        (header::CONTENT_TYPE.as_str(), "image/jpeg"),
        ("cache-control", "private, no-cache"),
        ("etag", etag.as_str()),
        ("vary", "Cookie"),
        ("x-content-type-options", "nosniff"),
        ("cross-origin-resource-policy", "same-site"),
    ] {
        assert_eq!(res.headers()[name], value, "{name}");
    }
    let served = to_bytes(res.into_body(), 1 << 20).await.unwrap();
    let decoded = image::load_from_memory_with_format(&served, ImageFormat::Jpeg).unwrap();
    assert_eq!((decoded.width(), decoded.height()), (512, 256));
    let res = get_image(&app, &cookie, Some(&etag)).await;
    assert_eq!(res.status(), StatusCode::NOT_MODIFIED);
    assert_eq!(res.headers()["etag"], etag.as_str());
    assert!(to_bytes(res.into_body(), 16).await.unwrap().is_empty());

    // Another customer neither sees nor removes it.
    let res = get_image(&app, &bob, None).await;
    assert_eq!(res.status(), StatusCode::NOT_FOUND);
    assert_eq!(json_body(res).await["error"]["code"], "NOT_FOUND");
    assert_eq!(delete_image(&app, &bob).await, StatusCode::NO_CONTENT);
    assert_eq!(
        get_image(&app, &cookie, Some(&etag)).await.status(),
        StatusCode::NOT_MODIFIED
    );

    // Replacement issues a new version; the old tag no longer matches.
    let res = put_image(
        &app,
        &cookie,
        "image/jpeg",
        encode(photo(300, 400, 200), ImageFormat::Jpeg),
    )
    .await;
    let second = json_body(res).await["data"]["profile_image"].clone();
    assert_eq!(
        (second["width"].as_u64(), second["height"].as_u64()),
        (Some(300), Some(400))
    );
    assert_ne!(second["version"], first["version"]);
    assert_eq!(
        get_image(&app, &cookie, Some(&etag)).await.status(),
        StatusCode::OK
    );
    let res = put_image(
        &app,
        &cookie,
        "image/webp",
        encode(photo(64, 64, 10), ImageFormat::WebP),
    )
    .await;
    let third = json_body(res).await["data"]["profile_image"].clone();
    assert_eq!(third["width"], 64);
    let rows: i64 = sqlx::query_scalar("SELECT count(*) FROM customers.profile_images")
        .fetch_one(&pool)
        .await?;
    assert_eq!(rows, 1, "replacement leaves no orphaned image");
    let replaced: Vec<Option<String>> = sqlx::query_scalar(
        "SELECT metadata->>'replaced_version' FROM audit.audit_events WHERE actor_id=$1 AND action='PROFILE_IMAGE_STORED' ORDER BY created_at",
    )
    .bind(ada)
    .fetch_all(&pool)
    .await?;
    assert_eq!(
        replaced,
        [
            None,
            Some(version.clone()),
            second["version"].as_str().map(str::to_owned)
        ]
    );

    // Removal is idempotent and leaves no stale reference.
    assert_eq!(delete_image(&app, &cookie).await, StatusCode::NO_CONTENT);
    assert_eq!(
        get_image(&app, &cookie, None).await.status(),
        StatusCode::NOT_FOUND
    );
    assert_eq!(
        get(&app, ME, Some(&cookie)).await.1["data"]["profile_image"],
        Value::Null
    );
    assert_eq!(delete_image(&app, &cookie).await, StatusCode::NO_CONTENT);
    assert_eq!(
        audit_actions(&pool, ada).await?,
        [
            "PROFILE_IMAGE_STORED",
            "PROFILE_IMAGE_STORED",
            "PROFILE_IMAGE_STORED",
            "PROFILE_IMAGE_REMOVED"
        ]
    );
    Ok(())
}

#[sqlx::test(migrator = "MIGRATOR")]
async fn unsafe_or_oversized_uploads_are_refused_without_changing_the_image(
    pool: PgPool,
) -> sqlx::Result<()> {
    let app = app(&pool).await?;
    customer(&pool, "ada").await?;
    let cookie = login(&app, "ada").await;
    let png = encode(photo(32, 32, 1), ImageFormat::Png);
    let res = put_image(&app, &cookie, "image/png", png.clone()).await;
    let current = json_body(res).await["data"]["profile_image"]["version"].clone();
    let refused = |status: StatusCode, reason: &'static str| {
        move |res: Response| async move {
            assert_eq!(res.status(), status, "{reason}");
            let body = json_body(res).await;
            assert_eq!(
                error_code(&body),
                ("VALIDATION_ERROR", &json!({"reason": reason}))
            );
        }
    };
    for content_type in [
        "image/gif",
        "image/svg+xml",
        "text/html",
        "application/octet-stream",
        "image/jpg",
    ] {
        refused(StatusCode::UNSUPPORTED_MEDIA_TYPE, "unsupported_type")(
            put_image(&app, &cookie, content_type, png.clone()).await,
        )
        .await;
    }
    let missing_type = send(
        &app,
        request(Method::PUT, IMAGE, Some(&cookie))
            .body(Body::from(png.clone()))
            .unwrap(),
    )
    .await;
    refused(StatusCode::UNSUPPORTED_MEDIA_TYPE, "unsupported_type")(missing_type).await;
    // The bytes, not the label, decide the format.
    refused(StatusCode::UNSUPPORTED_MEDIA_TYPE, "unsupported_type")(
        put_image(&app, &cookie, "image/jpeg", png.clone()).await,
    )
    .await;
    refused(StatusCode::UNSUPPORTED_MEDIA_TYPE, "unsupported_type")(
        put_image(
            &app,
            &cookie,
            "image/png",
            b"<svg onload=alert(1)>".to_vec(),
        )
        .await,
    )
    .await;
    let oversized = vec![0_u8; profile_limit() + 1];
    let declared = send(
        &app,
        request(Method::PUT, IMAGE, Some(&cookie))
            .header(header::CONTENT_TYPE, "image/png")
            .header(header::CONTENT_LENGTH, oversized.len())
            .body(Body::from(oversized.clone()))
            .unwrap(),
    )
    .await;
    refused(StatusCode::PAYLOAD_TOO_LARGE, "too_large")(declared).await;
    // Without a declared length, the body limit stops the read.
    refused(StatusCode::PAYLOAD_TOO_LARGE, "too_large")(
        put_image(&app, &cookie, "image/png", oversized).await,
    )
    .await;
    refused(StatusCode::UNPROCESSABLE_ENTITY, "too_many_pixels")(
        put_image(
            &app,
            &cookie,
            "image/png",
            encode(photo(4097, 2, 0), ImageFormat::Png),
        )
        .await,
    )
    .await;
    refused(StatusCode::UNPROCESSABLE_ENTITY, "undecodable")(
        put_image(&app, &cookie, "image/png", png[..png.len() / 2].to_vec()).await,
    )
    .await;
    let res = get_image(&app, &cookie, None).await;
    assert_eq!(
        res.headers()["etag"],
        format!("\"{}\"", current.as_str().unwrap()).as_str()
    );
    Ok(())
}

fn profile_limit() -> usize {
    deypay_application::profile::MAX_UPLOAD_BYTES
}

#[sqlx::test(migrator = "MIGRATOR")]
async fn storage_failures_keep_the_previous_profile_and_image(pool: PgPool) -> sqlx::Result<()> {
    let app = app(&pool).await?;
    let ada = customer(&pool, "ada").await?;
    let cookie = login(&app, "ada").await;
    let res = put_image(
        &app,
        &cookie,
        "image/png",
        encode(photo(20, 20, 3), ImageFormat::Png),
    )
    .await;
    let original = json_body(res).await["data"]["profile_image"]["version"].clone();
    // The audit write fails inside each operation's transaction.
    sqlx::raw_sql(
        "CREATE FUNCTION public.fail_profile_audit() RETURNS trigger LANGUAGE plpgsql AS
            $$ BEGIN RAISE EXCEPTION 'audit store unavailable'; END $$;
         CREATE TRIGGER fail_profile_audit BEFORE INSERT ON audit.audit_events FOR EACH ROW
            WHEN (NEW.action LIKE 'PROFILE%' OR NEW.action='PREFERENCES_CHANGED')
            EXECUTE FUNCTION public.fail_profile_audit();",
    )
    .execute(&pool)
    .await?;
    let res = put_image(
        &app,
        &cookie,
        "image/jpeg",
        encode(photo(40, 40, 9), ImageFormat::Jpeg),
    )
    .await;
    assert_eq!(res.status(), StatusCode::SERVICE_UNAVAILABLE);
    assert_eq!(
        delete_image(&app, &cookie).await,
        StatusCode::SERVICE_UNAVAILABLE
    );
    assert_eq!(
        patch(&app, ME, &cookie, json!({"first_name": "Lost"}))
            .await
            .0,
        StatusCode::SERVICE_UNAVAILABLE
    );
    assert_eq!(
        patch(
            &app,
            PREFERENCES,
            &cookie,
            json!({"push_notifications": true})
        )
        .await
        .0,
        StatusCode::SERVICE_UNAVAILABLE
    );
    let res = get_image(&app, &cookie, None).await;
    assert_eq!(
        res.headers()["etag"],
        format!("\"{}\"", original.as_str().unwrap()).as_str()
    );
    let state: (String, i64) = sqlx::query_as(
        "SELECT (SELECT first_name FROM customers.profiles WHERE user_id=$1),
                (SELECT count(*) FROM customers.preferences WHERE user_id=$1)",
    )
    .bind(ada)
    .fetch_one(&pool)
    .await?;
    assert_eq!(state, ("Ada".into(), 0));
    // Losing the runtime's operation grant fails closed without leaking detail.
    sqlx::raw_sql(
        "DROP TRIGGER fail_profile_audit ON audit.audit_events;
        REVOKE EXECUTE ON FUNCTION customers.customer_profile(uuid) FROM deypay_app;",
    )
    .execute(&pool)
    .await?;
    let (status, body) = get(&app, ME, Some(&cookie)).await;
    assert_eq!(
        (status, error_code(&body).0),
        (StatusCode::SERVICE_UNAVAILABLE, "SERVICE_UNAVAILABLE")
    );
    assert!(!body.to_string().contains("permission"));
    Ok(())
}

/// Starts the real API executable against the test database, as the general
/// runtime role, and returns its listening address.
struct ApiProcess(std::process::Child);

impl Drop for ApiProcess {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

async fn start_api(pool: &PgPool, login_mode: &str) -> (ApiProcess, String) {
    use std::io::BufRead;
    let database: String = sqlx::query_scalar("SELECT current_database()")
        .fetch_one(pool)
        .await
        .unwrap();
    let base = std::env::var("DATABASE_URL").unwrap();
    let base = base.split('?').next().unwrap();
    let url = format!(
        "{}/{database}?options=-c%20role%3Ddeypay_app",
        base.rsplit_once('/').unwrap().0
    );
    let mut child = std::process::Command::new(env!("CARGO_BIN_EXE_deypay-api"))
        .env("APP_ENV", "test")
        .env("API_BIND_ADDR", "127.0.0.1:0")
        .env("DATABASE_URL", url)
        .env("LOG_LEVEL", "info")
        .env("DEYPAY_LOGIN_MODE", login_mode)
        .env("DEYPAY_AUTH_RELEASE", RELEASE)
        .env("DEYPAY_LOGIN_ORIGIN", ORIGIN)
        .env_remove("DEYPAY_REGISTRATION_MODE")
        .env_remove("EMAIL_VERIFICATION_ENABLED")
        .env_remove("DEYPAY_AUTH_SIGNING_KEY_FILE")
        .env_remove("DEYPAY_AUTH_DATABASE_URL")
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::null())
        .spawn()
        .unwrap();
    let stdout = child.stdout.take().unwrap();
    let process = ApiProcess(child);
    let address = tokio::time::timeout(
        std::time::Duration::from_secs(60),
        tokio::task::spawn_blocking(move || {
            for line in std::io::BufReader::new(stdout).lines() {
                let line: Value = serde_json::from_str(&line.ok()?).ok()?;
                if line["fields"]["message"] == "API listening" {
                    return line["fields"]["address"].as_str().map(str::to_owned);
                }
            }
            None
        }),
    )
    .await
    .expect("API startup timed out")
    .unwrap()
    .expect("API exited during startup");
    (process, address)
}

/// One HTTP/1.1 exchange with the spawned process.
async fn exchange(address: &str, request: impl Into<Vec<u8>>) -> (u16, Value) {
    use std::io::{Read, Write};
    let address = address.to_owned();
    let request = request.into();
    tokio::task::spawn_blocking(move || {
        let mut stream = std::net::TcpStream::connect(address).unwrap();
        stream
            .set_read_timeout(Some(std::time::Duration::from_secs(30)))
            .unwrap();
        stream.write_all(&request).unwrap();
        let mut raw = Vec::new();
        stream.read_to_end(&mut raw).unwrap();
        let text = String::from_utf8(raw).unwrap();
        let (head, body) = text.split_once("\r\n\r\n").unwrap();
        let status = head.split(' ').nth(1).unwrap().parse().unwrap();
        (status, serde_json::from_str(body).unwrap_or(Value::Null))
    })
    .await
    .unwrap()
}

fn raw_request(method: &str, path: &str, cookie: Option<&str>, body: Option<Value>) -> String {
    let mut request =
        format!("{method} {path} HTTP/1.1\r\nHost: api.test\r\nConnection: close\r\n");
    if let Some(cookie) = cookie {
        request += &format!("Cookie: {cookie}\r\n");
    }
    let mut body = body.map(|b| b.to_string()).unwrap_or_default();
    if method == "GET" {
        body.clear();
    } else {
        request += &format!(
            "Origin: {ORIGIN}\r\nContent-Type: application/json\r\nContent-Length: {}\r\n",
            body.len()
        );
    }
    request + "\r\n" + &body
}

#[sqlx::test(migrator = "MIGRATOR")]
async fn the_api_executable_mounts_profiles_behind_the_customer_session_guard(
    pool: PgPool,
) -> sqlx::Result<()> {
    grants(&pool).await?;
    let ada = customer(&pool, "ada").await?;
    // Issue the session as the separate verifier process would.
    let verifier = role_pool(&pool, "SET ROLE deypay_auth").await?;
    let auth = BrowserAuth::new(
        PgSessions::new(verifier, RELEASE.parse().unwrap(), [0x37; 32].into()),
        hasher(),
        BrowserOrigin::parse(ORIGIN).unwrap(),
    );
    let cookie = login(&auth.routes(), "ada").await;

    let (api, address) = start_api(&pool, "sandbox").await;
    let (status, body) = exchange(&address, raw_request("GET", ME, None, None)).await;
    assert_eq!(
        (status, body["error"]["code"].as_str()),
        (401, Some("AUTHENTICATION_REQUIRED"))
    );
    let (status, body) = exchange(&address, raw_request("GET", ME, Some(&cookie), None)).await;
    assert_eq!(status, 200);
    assert_eq!(body["data"]["id"], ada.to_string());
    let (status, body) = exchange(
        &address,
        raw_request(
            "PATCH",
            PREFERENCES,
            Some(&cookie),
            Some(json!({"push_notifications": true})),
        ),
    )
    .await;
    assert_eq!(
        (status, body["data"]["push_notifications"].as_bool()),
        (200, Some(true))
    );
    let (status, _) = exchange(&address, raw_request("DELETE", IMAGE, Some(&cookie), None)).await;
    assert_eq!(status, 204);
    drop(api);

    // Without customer authentication every profile route is closed.
    let (_api, address) = start_api(&pool, "off").await;
    for (method, path) in [
        ("GET", ME),
        ("GET", STATUS),
        ("PATCH", PREFERENCES),
        ("PUT", IMAGE),
    ] {
        let (status, body) = exchange(
            &address,
            raw_request(method, path, Some(&cookie), Some(json!({}))),
        )
        .await;
        assert_eq!(
            (status, body["error"]["code"].as_str()),
            (503, Some("SERVICE_UNAVAILABLE")),
            "{method} {path}"
        );
    }
    Ok(())
}

/// Starts an upload whose body is never sent. The server asks for the body,
/// with `100 Continue`, only after the handler holds a processing slot, which
/// it keeps until the connection closes.
fn stalled_upload(address: &str, cookie: &str) -> std::net::TcpStream {
    use std::io::{Read, Write};
    let mut stream = std::net::TcpStream::connect(address).unwrap();
    stream
        .set_read_timeout(Some(std::time::Duration::from_secs(20)))
        .unwrap();
    write!(
        stream,
        "PUT {IMAGE} HTTP/1.1\r\nHost: api.test\r\nCookie: {cookie}\r\nOrigin: {ORIGIN}\r\n\
         Content-Type: image/png\r\nContent-Length: 1000\r\nExpect: 100-continue\r\n\r\n"
    )
    .unwrap();
    let mut head = Vec::new();
    let mut byte = [0_u8; 1];
    while !head.ends_with(b"\r\n\r\n") {
        stream.read_exact(&mut byte).unwrap();
        head.push(byte[0]);
    }
    assert!(
        head.starts_with(b"HTTP/1.1 100 "),
        "{}",
        String::from_utf8_lossy(&head)
    );
    stream
}

fn upload_request(cookie: &str, image: &[u8]) -> Vec<u8> {
    let mut request = format!(
        "PUT {IMAGE} HTTP/1.1\r\nHost: api.test\r\nConnection: close\r\nCookie: {cookie}\r\n\
         Origin: {ORIGIN}\r\nContent-Type: image/png\r\nContent-Length: {}\r\n\r\n",
        image.len()
    )
    .into_bytes();
    request.extend_from_slice(image);
    request
}

#[sqlx::test(migrator = "MIGRATOR")]
async fn uploads_beyond_the_processing_slots_are_refused_as_busy(pool: PgPool) -> sqlx::Result<()> {
    grants(&pool).await?;
    let ada = customer(&pool, "ada").await?;
    let verifier = role_pool(&pool, "SET ROLE deypay_auth").await?;
    let auth = BrowserAuth::new(
        PgSessions::new(verifier, RELEASE.parse().unwrap(), [0x37; 32].into()),
        hasher(),
        BrowserOrigin::parse(ORIGIN).unwrap(),
    );
    let cookie = login(&auth.routes(), "ada").await;
    let (_api, address) = start_api(&pool, "sandbox").await;
    let png = encode(photo(64, 64, 0), ImageFormat::Png);

    // Two stalled uploads hold both processing slots of the instance.
    let first = stalled_upload(&address, &cookie);
    let _second = stalled_upload(&address, &cookie);
    let started = std::time::Instant::now();
    let (status, body) = exchange(&address, upload_request(&cookie, &png)).await;
    assert_eq!(
        (status, error_code(&body)),
        (503, ("SERVICE_UNAVAILABLE", &json!({"reason": "busy"})))
    );
    assert!(started.elapsed() >= std::time::Duration::from_secs(4));

    // An abandoned upload releases its slot and stores nothing.
    drop(first);
    let (status, body) = exchange(&address, upload_request(&cookie, &png)).await;
    assert_eq!(status, 200);
    assert_eq!(body["data"]["profile_image"]["width"], 64);
    assert_eq!(audit_actions(&pool, ada).await?, ["PROFILE_IMAGE_STORED"]);
    Ok(())
}
