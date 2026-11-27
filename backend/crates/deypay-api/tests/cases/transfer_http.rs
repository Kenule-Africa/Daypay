//! Internal transfer HTTP tests (IMP-P7-001–010). Sessions come from the
//! merged login routes, every route sits behind the same session guard as in
//! production, and persistence runs as the restricted runtime role. The only
//! stand-in is `ApprovedForTests`, an approved financial policy that defers to
//! the database's checks; the shipped `NoApprovedPolicy` is tested as well.
use super::login_sessions::RELEASE;
use super::profile_http::{customer, grants, login, role_pool};
use super::wallet_http::deposit;
use axum::{
    Router,
    body::{Body, to_bytes},
    http::{HeaderMap, Method, Request, StatusCode, header},
};
use deypay_api::{login::BrowserAuth, sessions::BrowserOrigin, transfers, wallets};
use deypay_application::ledger::{FinancialGate, GatedOperation, LedgerError};
use deypay_infrastructure::{MIGRATOR, password_hashing::Argon2idHasher, sessions::PgSessions};
use serde_json::{Value, json};
use sqlx::PgPool;
use tower::ServiceExt;
use uuid::Uuid;

const ORIGIN: &str = "https://sandbox.example.test";
const SCRIPTS: [&str; 6] = [
    include_str!("../../../../../infrastructure/postgres/ledger-roles.psql"),
    include_str!("../../../../../infrastructure/postgres/wallet-roles.psql"),
    include_str!("../../../../../infrastructure/postgres/payments-roles.psql"),
    include_str!("../../../../../infrastructure/postgres/ledger-grants.psql"),
    include_str!("../../../../../infrastructure/postgres/wallet-grants.psql"),
    include_str!("../../../../../infrastructure/postgres/payments-grants.psql"),
];

/// Test-only stand-in for a future approved financial policy.
struct ApprovedForTests;

impl FinancialGate for ApprovedForTests {
    async fn authorize(&self, _: Uuid, _: GatedOperation<'_>) -> Result<(), LedgerError> {
        Ok(())
    }
    fn policy_approved(&self) -> bool {
        true
    }
}

async fn app(pool: &PgPool, approved: bool) -> sqlx::Result<Router> {
    grants(pool).await?;
    for script in SCRIPTS {
        sqlx::raw_sql(script).execute(pool).await?;
    }
    sqlx::raw_sql(
        "INSERT INTO payments.recipient_lookup_policy(window_seconds, lookups_per_window, approval_reference)
         VALUES (600, 100, 'TEST-FIXTURE')",
    )
    .execute(pool)
    .await?;
    let verifier = role_pool(pool, "SET ROLE deypay_auth").await?;
    let runtime = role_pool(pool, "SET ROLE deypay_app").await?;
    let auth = BrowserAuth::new(
        PgSessions::new(verifier, RELEASE.parse().unwrap(), [0x37; 32].into()),
        Argon2idHasher::new(32, 2, 1, 4).unwrap(),
        BrowserOrigin::parse(ORIGIN).unwrap(),
    );
    let transfer_routes = if approved {
        transfers::routes(transfers::TransferApi::with_gate(
            runtime.clone(),
            ApprovedForTests,
        ))
    } else {
        transfers::routes(transfers::TransferApi::new(runtime.clone()))
    };
    Ok(auth.routes().merge(
        auth.protect(wallets::routes(wallets::WalletApi::new(runtime)).merge(transfer_routes)),
    ))
}

/// A KYC-approved customer, signed in.
async fn approved_customer(pool: &PgPool, app: &Router, name: &str) -> (Uuid, String) {
    let id = customer(pool, name).await.unwrap();
    sqlx::query("UPDATE identity.users SET status = 'KYC_APPROVED' WHERE id = $1")
        .bind(id)
        .execute(pool)
        .await
        .unwrap();
    (id, login(app, name).await)
}

async fn call(
    app: &Router,
    method: Method,
    path: &str,
    cookie: Option<&str>,
    headers: &[(&str, &str)],
    body: Option<Value>,
) -> (StatusCode, HeaderMap, Value) {
    let mut req = Request::builder().method(method.clone()).uri(path);
    if method != Method::GET {
        req = req.header(header::ORIGIN, ORIGIN);
    }
    if let Some(cookie) = cookie {
        req = req.header(header::COOKIE, cookie);
    }
    for (name, value) in headers {
        req = req.header(*name, *value);
    }
    let req = match body {
        Some(body) => req
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from(body.to_string())),
        None => req.body(Body::empty()),
    };
    let res = app.clone().oneshot(req.unwrap()).await.unwrap();
    let status = res.status();
    let headers = res.headers().clone();
    assert_eq!(headers[header::CACHE_CONTROL], "no-store", "{path}");
    let bytes = to_bytes(res.into_body(), 1 << 20).await.unwrap();
    (status, headers, serde_json::from_slice(&bytes).unwrap())
}

fn code(body: &Value) -> &str {
    body["error"]["code"].as_str().unwrap()
}

fn keys(value: &Value) -> Vec<&str> {
    let mut keys: Vec<&str> = value
        .as_object()
        .unwrap()
        .keys()
        .map(String::as_str)
        .collect();
    keys.sort_unstable();
    keys
}

async fn activate_transfers(pool: &PgPool, symbol: &str) {
    sqlx::query(
        "INSERT INTO ledger.capability_activations(capability, asset_id, approval_reference, max_operation_minor)
         SELECT 'CUSTOMER_TRANSFER', id, 'TEST-FIXTURE', 1000000000000 FROM wallets.assets WHERE symbol = $1",
    )
    .bind(symbol)
    .execute(pool)
    .await
    .unwrap();
}

#[sqlx::test(migrator = "MIGRATOR")]
async fn transfer_routes_require_a_customer_session(pool: PgPool) -> sqlx::Result<()> {
    let app = app(&pool, true).await?;
    let id = Uuid::from_u128(7);
    for path in [
        "/api/v1/recipients/resolve?username=someone".to_owned(),
        "/api/v1/transactions".to_owned(),
        format!("/api/v1/transactions/{id}"),
    ] {
        let (status, _, body) = call(&app, Method::GET, &path, None, &[], None).await;
        assert_eq!(
            (status, code(&body)),
            (StatusCode::UNAUTHORIZED, "AUTHENTICATION_REQUIRED"),
            "{path}"
        );
    }
    for path in [
        "/api/v1/transaction-authorizations".to_owned(),
        format!("/api/v1/transaction-authorizations/{id}/confirm"),
        "/api/v1/transfers/internal".to_owned(),
    ] {
        let (status, _, _) = call(&app, Method::POST, &path, None, &[], Some(json!({}))).await;
        assert_eq!(status, StatusCode::UNAUTHORIZED, "{path}");
    }
    // State changes need the configured browser origin.
    let (_, cookie) = approved_customer(&pool, &app, "xferorigin").await;
    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri("/api/v1/transaction-authorizations")
                .header(header::COOKIE, &cookie)
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from("{}"))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::FORBIDDEN);
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM identity.verification_challenges")
            .fetch_one(&pool)
            .await?,
        0
    );
    Ok(())
}

#[sqlx::test(migrator = "MIGRATOR")]
async fn customers_confirm_and_send_once_over_http(pool: PgPool) -> sqlx::Result<()> {
    let app = app(&pool, true).await?;
    let (alice, alice_cookie) = approved_customer(&pool, &app, "xferalice").await;
    let (bob, bob_cookie) = approved_customer(&pool, &app, "xferbob").await;
    let (_, carol_cookie) = approved_customer(&pool, &app, "xfercarol").await;
    // Wallets exist once read; then the ledger receives a verified deposit.
    let (status, _, _) = call(
        &app,
        Method::GET,
        "/api/v1/wallets",
        Some(&alice_cookie),
        &[],
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    deposit(&pool, alice, "USDC", "50000000").await;
    activate_transfers(&pool, "USDC").await;

    // Resolution exposes only the permitted identity details.
    let (status, _, body) = call(
        &app,
        Method::GET,
        "/api/v1/recipients/resolve?username=%40XferBob",
        Some(&alice_cookie),
        &[],
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        body["data"],
        json!({"recipient_id": bob.to_string(), "username": "xferbob",
               "display_name": "Ada Okafor", "can_receive": true})
    );
    for (query, expected) in [
        ("username=nobody_here", StatusCode::NOT_FOUND),
        ("username=a..b", StatusCode::UNPROCESSABLE_ENTITY),
        ("username=bob&x=1", StatusCode::BAD_REQUEST),
    ] {
        let (status, _, _) = call(
            &app,
            Method::GET,
            &format!("/api/v1/recipients/resolve?{query}"),
            Some(&alice_cookie),
            &[],
            None,
        )
        .await;
        assert_eq!(status, expected, "{query}");
    }

    // Step 1: the exact terms, normalized for display.
    let terms = json!({"recipient_id": bob.to_string(), "asset": "USDC", "amount": "12.5",
                       "description": "Dinner"});
    let mut request = terms.clone();
    request["operation_type"] = json!("internal_transfer");
    let (status, _, body) = call(
        &app,
        Method::POST,
        "/api/v1/transaction-authorizations",
        Some(&alice_cookie),
        &[],
        Some(request),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);
    let challenge = body["data"]["challenge_id"].as_str().unwrap().to_owned();
    assert_eq!(body["data"]["verification_method"], "confirmation");
    assert_eq!(
        body["data"]["terms"],
        json!({"recipient": {"id": bob.to_string(), "username": "xferbob", "display_name": "Ada Okafor"},
               "asset": "USDC", "amount": "12.500000", "fee": "0.000000", "total": "12.500000",
               "description": "Dinner"})
    );
    // Step 2: explicit confirmation, by the same customer only.
    let confirm = format!("/api/v1/transaction-authorizations/{challenge}/confirm");
    let (status, _, body) = call(
        &app,
        Method::POST,
        &confirm,
        Some(&carol_cookie),
        &[],
        Some(json!({"confirm": true})),
    )
    .await;
    assert_eq!(
        (status, code(&body)),
        (StatusCode::FORBIDDEN, "AUTHORIZATION_INVALID")
    );
    let (status, _, _) = call(
        &app,
        Method::POST,
        &confirm,
        Some(&alice_cookie),
        &[],
        Some(json!({"confirm": false})),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    let (status, _, body) = call(
        &app,
        Method::POST,
        &confirm,
        Some(&alice_cookie),
        &[],
        Some(json!({"confirm": true})),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(keys(&body["data"]), ["authorization_token", "expires_at"]);
    let token = body["data"]["authorization_token"]
        .as_str()
        .unwrap()
        .to_owned();

    // Step 3: the transfer, once, under an idempotency key.
    let mut transfer = terms.clone();
    transfer["authorization_token"] = json!(token);
    let (status, _, _) = call(
        &app,
        Method::POST,
        "/api/v1/transfers/internal",
        Some(&alice_cookie),
        &[],
        Some(transfer.clone()),
    )
    .await;
    assert_eq!(
        status,
        StatusCode::BAD_REQUEST,
        "the Idempotency-Key header is required"
    );
    let key = [("Idempotency-Key", "http-transfer-0001")];
    let (status, headers, body) = call(
        &app,
        Method::POST,
        "/api/v1/transfers/internal",
        Some(&alice_cookie),
        &key,
        Some(transfer.clone()),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);
    assert!(headers.get("idempotency-replayed").is_none());
    let sent = body["data"].clone();
    assert_eq!(
        (
            sent["type"].as_str(),
            sent["status"].as_str(),
            sent["asset"].as_str(),
            sent["amount"].as_str(),
            sent["fee"].as_str(),
            sent["recipient"]["username"].as_str(),
        ),
        (
            Some("internal_transfer"),
            Some("completed"),
            Some("USDC"),
            Some("12.500000"),
            Some("0.000000"),
            Some("xferbob")
        )
    );
    let id = sent["transaction_id"].as_str().unwrap().to_owned();
    // A retry returns the accepted response, even after the recipient edits their profile.
    sqlx::query("UPDATE customers.profiles SET first_name = 'Amaka' WHERE user_id = $1")
        .bind(bob)
        .execute(&pool)
        .await?;
    let (status, headers, body) = call(
        &app,
        Method::POST,
        "/api/v1/transfers/internal",
        Some(&alice_cookie),
        &key,
        Some(transfer.clone()),
    )
    .await;
    assert_eq!((status, &body["data"]), (StatusCode::CREATED, &sent));
    assert_eq!(headers["idempotency-replayed"], "true");
    transfer["amount"] = json!("1");
    let (status, _, body) = call(
        &app,
        Method::POST,
        "/api/v1/transfers/internal",
        Some(&alice_cookie),
        &key,
        Some(transfer),
    )
    .await;
    assert_eq!(
        (status, code(&body)),
        (StatusCode::CONFLICT, "IDEMPOTENCY_CONFLICT")
    );

    // Both participants see one transaction with the same reference.
    let (_, _, mine) = call(
        &app,
        Method::GET,
        "/api/v1/transactions?limit=1",
        Some(&alice_cookie),
        &[],
        None,
    )
    .await;
    let (_, _, theirs) = call(
        &app,
        Method::GET,
        "/api/v1/transactions",
        Some(&bob_cookie),
        &[],
        None,
    )
    .await;
    assert_eq!(mine["data"].as_array().unwrap().len(), 1);
    assert_eq!(mine["meta"]["next_cursor"], Value::Null);
    assert_eq!(theirs["data"].as_array().unwrap().len(), 1);
    let (mine, theirs) = (&mine["data"][0], &theirs["data"][0]);
    assert_eq!(
        (mine["id"].as_str(), theirs["id"].as_str()),
        (Some(id.as_str()), Some(id.as_str()))
    );
    assert_eq!(mine["reference"], theirs["reference"]);
    assert_eq!(
        (mine["direction"].as_str(), theirs["direction"].as_str()),
        (Some("debit"), Some("credit"))
    );
    assert_eq!(theirs["counterparty"]["id"], alice.to_string());
    assert_eq!(
        keys(mine),
        [
            "completed_at",
            "counterparty",
            "created_at",
            "description",
            "destination_amount",
            "destination_asset",
            "direction",
            "fee",
            "id",
            "reference",
            "source_amount",
            "source_asset",
            "status",
            "type"
        ]
    );
    for (cookie, path, expected) in [
        (
            &carol_cookie,
            format!("/api/v1/transactions/{id}"),
            StatusCode::NOT_FOUND,
        ),
        (
            &alice_cookie,
            "/api/v1/transactions/not-a-uuid".to_owned(),
            StatusCode::NOT_FOUND,
        ),
        (
            &bob_cookie,
            format!("/api/v1/transactions/{id}"),
            StatusCode::OK,
        ),
        (
            &alice_cookie,
            "/api/v1/transactions?limit=0".to_owned(),
            StatusCode::BAD_REQUEST,
        ),
    ] {
        let (status, _, _) = call(&app, Method::GET, &path, Some(cookie), &[], None).await;
        assert_eq!(status, expected, "{path}");
    }

    // The balances are the ledger's.
    let (_, _, wallets) = call(
        &app,
        Method::GET,
        "/api/v1/wallets",
        Some(&alice_cookie),
        &[],
        None,
    )
    .await;
    let usdt = wallets["data"]
        .as_array()
        .unwrap()
        .iter()
        .find(|w| w["asset"] == "USDC")
        .unwrap();
    assert_eq!(usdt["balance"]["available"], "37.500000");
    Ok(())
}

#[sqlx::test(migrator = "MIGRATOR")]
async fn the_shipped_gate_keeps_transfers_disabled(pool: PgPool) -> sqlx::Result<()> {
    let app = app(&pool, false).await?;
    let (_, alice_cookie) = approved_customer(&pool, &app, "shutalice").await;
    let (bob, _) = approved_customer(&pool, &app, "shutbob").await;
    activate_transfers(&pool, "USDC").await;
    let (status, _, body) = call(
        &app,
        Method::POST,
        "/api/v1/transaction-authorizations",
        Some(&alice_cookie),
        &[],
        Some(
            json!({"operation_type": "internal_transfer", "recipient_id": bob.to_string(),
                    "asset": "USDC", "amount": "1"}),
        ),
    )
    .await;
    assert_eq!(
        (status, code(&body), &body["error"]["details"]),
        (
            StatusCode::SERVICE_UNAVAILABLE,
            "SERVICE_UNAVAILABLE",
            &json!({"reason": "transfers_not_enabled"})
        )
    );
    let (status, _, body) = call(
        &app,
        Method::POST,
        "/api/v1/transfers/internal",
        Some(&alice_cookie),
        &[("Idempotency-Key", "shut-transfer-01")],
        Some(
            json!({"recipient_id": bob.to_string(), "asset": "USDC", "amount": "1",
                    "authorization_token": "0".repeat(64)}),
        ),
    )
    .await;
    assert_eq!(
        (status, &body["error"]["details"]["reason"]),
        (
            StatusCode::SERVICE_UNAVAILABLE,
            &json!("transfers_not_enabled")
        )
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT (SELECT count(*) FROM identity.verification_challenges)
                  + (SELECT count(*) FROM payments.transactions)"
        )
        .fetch_one(&pool)
        .await?,
        0
    );
    // Looking a recipient up is not a movement of funds.
    let (status, _, _) = call(
        &app,
        Method::GET,
        "/api/v1/recipients/resolve?username=shutbob",
        Some(&alice_cookie),
        &[],
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    Ok(())
}
