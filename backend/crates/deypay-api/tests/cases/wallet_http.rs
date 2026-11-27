//! Customer wallet HTTP tests (IMP-P6-006/009/010). Sessions come from the
//! merged login routes; wallet persistence runs as the restricted runtime
//! role, and balances are posted through the ledger's poster role.
use super::login_sessions::RELEASE;
use super::profile_http::{customer, grants, login, role_pool};
use axum::{
    Router,
    body::{Body, to_bytes},
    http::{Method, Request, StatusCode, header},
    response::Response,
};
use deypay_api::{login::BrowserAuth, sessions::BrowserOrigin, wallets};
use deypay_infrastructure::{MIGRATOR, password_hashing::Argon2idHasher, sessions::PgSessions};
use serde_json::{Value, json};
use sqlx::PgPool;
use tower::ServiceExt;
use uuid::Uuid;

const ORIGIN: &str = "https://sandbox.example.test";
const SCRIPTS: [&str; 4] = [
    include_str!("../../../../../infrastructure/postgres/ledger-roles.psql"),
    include_str!("../../../../../infrastructure/postgres/wallet-roles.psql"),
    include_str!("../../../../../infrastructure/postgres/ledger-grants.psql"),
    include_str!("../../../../../infrastructure/postgres/wallet-grants.psql"),
];

async fn app(pool: &PgPool) -> sqlx::Result<(Router, PgPool)> {
    grants(pool).await?;
    for script in SCRIPTS {
        sqlx::raw_sql(script).execute(pool).await?;
    }
    let verifier = role_pool(pool, "SET ROLE deypay_auth").await?;
    let runtime = role_pool(pool, "SET ROLE deypay_app").await?;
    let auth = BrowserAuth::new(
        PgSessions::new(verifier, RELEASE.parse().unwrap(), [0x37; 32].into()),
        Argon2idHasher::new(32, 2, 1, 4).unwrap(),
        BrowserOrigin::parse(ORIGIN).unwrap(),
    );
    let routes = auth
        .routes()
        .merge(auth.protect(wallets::routes(wallets::WalletApi::new(runtime.clone()))));
    Ok((routes, runtime))
}

async fn send(
    app: &Router,
    method: Method,
    path: &str,
    cookie: Option<&str>,
    body: Option<Value>,
) -> Response {
    let mut req = Request::builder().method(method.clone()).uri(path);
    if method != Method::GET {
        req = req.header(header::ORIGIN, ORIGIN);
    }
    if let Some(cookie) = cookie {
        req = req.header(header::COOKIE, cookie);
    }
    let req = match body {
        Some(body) => req
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from(body.to_string())),
        None => req.body(Body::empty()),
    };
    app.clone().oneshot(req.unwrap()).await.unwrap()
}

async fn call(
    app: &Router,
    method: Method,
    path: &str,
    cookie: Option<&str>,
    body: Option<Value>,
) -> (StatusCode, Value) {
    let res = send(app, method, path, cookie, body).await;
    let status = res.status();
    assert_eq!(res.headers()[header::CACHE_CONTROL], "no-store", "{path}");
    let bytes = to_bytes(res.into_body(), 1 << 20).await.unwrap();
    (status, serde_json::from_slice(&bytes).unwrap())
}

async fn get(app: &Router, path: &str, cookie: Option<&str>) -> (StatusCode, Value) {
    call(app, Method::GET, path, cookie, None).await
}

fn code(body: &Value) -> &str {
    body["error"]["code"].as_str().unwrap()
}

/// Credits a verified deposit through the ledger's poster role.
async fn deposit(pool: &PgPool, customer: Uuid, symbol: &str, minor: &str) {
    sqlx::raw_sql(&format!(
        "INSERT INTO ledger.capability_activations(capability, asset_id, approval_reference, max_operation_minor)
         SELECT 'CUSTOMER_CREDIT', id, 'TEST-FIXTURE', 1000000000000 FROM wallets.assets WHERE symbol = '{symbol}'
         ON CONFLICT DO NOTHING;
         INSERT INTO ledger.accounts(chart_code, account_type, normal_balance, negative_allowed, asset_id, owner_type)
         SELECT '1100', 'ASSET', 'DEBIT', false, id, 'PLATFORM' FROM wallets.assets WHERE symbol = '{symbol}'
         ON CONFLICT DO NOTHING;"
    ))
    .execute(pool)
    .await
    .unwrap();
    let mut tx = pool.begin().await.unwrap();
    sqlx::query("SET LOCAL ROLE deypay_ledger_poster")
        .execute(&mut *tx)
        .await
        .unwrap();
    sqlx::query(
        "SELECT * FROM ledger.post_journal('DEPOSIT_CONFIRMED', 'chain:http:' || $1::text, 'RECOGNITION', NULL,
             ARRAY[(SELECT a.id FROM ledger.accounts a JOIN wallets.assets s ON s.id = a.asset_id
                    WHERE a.chart_code = '1100' AND s.symbol = $2),
                   (SELECT a.id FROM ledger.accounts a JOIN wallets.assets s ON s.id = a.asset_id
                    WHERE a.owner_id = $1 AND a.customer_role = 'AVAILABLE' AND s.symbol = $2)],
             ARRAY[(SELECT id FROM wallets.assets WHERE symbol = $2), (SELECT id FROM wallets.assets WHERE symbol = $2)],
             ARRAY['DEBIT','CREDIT'], ARRAY[$3, $3], NULL, 'SYSTEM', NULL, NULL)",
    )
    .bind(customer)
    .bind(symbol)
    .bind(minor)
    .execute(&mut *tx)
    .await
    .unwrap();
    tx.commit().await.unwrap();
}

#[sqlx::test(migrator = "MIGRATOR")]
async fn wallet_routes_require_a_customer_session(pool: PgPool) -> sqlx::Result<()> {
    let (app, _) = app(&pool).await?;
    let wallet = Uuid::from_u128(7);
    for path in [
        "/api/v1/wallets".to_owned(),
        format!("/api/v1/wallets/{wallet}"),
        format!("/api/v1/wallets/{wallet}/balance"),
        format!("/api/v1/wallets/{wallet}/deposit-addresses?network=base"),
        "/api/v1/networks".to_owned(),
        "/api/v1/assets/USDC/networks".to_owned(),
    ] {
        let (status, body) = get(&app, &path, None).await;
        assert_eq!(
            (status, code(&body)),
            (StatusCode::UNAUTHORIZED, "AUTHENTICATION_REQUIRED"),
            "{path}"
        );
    }
    let (status, _) = call(
        &app,
        Method::POST,
        &format!("/api/v1/wallets/{wallet}/deposit-addresses"),
        Some("__Host-deypay_session=00"),
        Some(json!({"network": "base"})),
    )
    .await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM wallets.wallet_profiles")
            .fetch_one(&pool)
            .await?,
        0,
        "nothing was created without a session"
    );
    Ok(())
}

#[sqlx::test(migrator = "MIGRATOR")]
async fn customers_see_only_their_own_ledger_backed_wallets(pool: PgPool) -> sqlx::Result<()> {
    let (app, _) = app(&pool).await?;
    let alice = customer(&pool, "walletalice").await?;
    customer(&pool, "walletbob").await?;
    let alice_cookie = login(&app, "walletalice").await;
    let bob_cookie = login(&app, "walletbob").await;

    let (status, body) = get(&app, "/api/v1/wallets", Some(&alice_cookie)).await;
    assert_eq!(status, StatusCode::OK);
    let wallets = body["data"].as_array().unwrap();
    assert_eq!(
        wallets
            .iter()
            .map(|w| w["asset"].as_str().unwrap())
            .collect::<Vec<_>>(),
        ["USDC", "USDT"],
        "USDC and USDT only; no customer NGN wallet"
    );
    for wallet in wallets {
        let mut keys: Vec<&str> = wallet
            .as_object()
            .unwrap()
            .keys()
            .map(String::as_str)
            .collect();
        keys.sort_unstable();
        assert_eq!(
            keys,
            [
                "asset",
                "asset_name",
                "balance",
                "capabilities",
                "created_at",
                "id",
                "restrictions",
                "status",
                "updated_at"
            ]
        );
        assert_eq!(wallet["status"], "active");
        assert_eq!(
            wallet["balance"],
            json!({"available": "0.000000", "reserved": "0.000000", "total": "0.000000"})
        );
        assert_eq!(
            wallet["restrictions"],
            json!(["kyc_required", "financial_activity_not_enabled"])
        );
        assert_eq!(
            wallet["capabilities"],
            json!({"deposit": false, "send": false})
        );
    }
    let usdc = wallets[0]["id"].as_str().unwrap().to_owned();

    // The balance is the ledger's, as exact decimal strings.
    deposit(&pool, alice, "USDC", "12500000").await;
    let (status, body) = get(
        &app,
        &format!("/api/v1/wallets/{usdc}/balance"),
        Some(&alice_cookie),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        body["data"],
        json!({"wallet_id": usdc, "asset": "USDC", "status": "active",
               "available": "12.500000", "reserved": "0.000000", "total": "12.500000"})
    );
    let (status, body) = get(
        &app,
        &format!("/api/v1/wallets/{usdc}"),
        Some(&alice_cookie),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["data"]["balance"]["total"], "12.500000");
    assert_eq!(
        body["data"]["networks"],
        json!([]),
        "no network is approved"
    );

    // Another customer's wallet, an unknown one and a malformed identifier all look alike.
    let (_, bob_wallets) = get(&app, "/api/v1/wallets", Some(&bob_cookie)).await;
    assert!(
        bob_wallets["data"]
            .as_array()
            .unwrap()
            .iter()
            .all(|w| w["id"] != usdc)
    );
    let not_found =
        json!({"error": {"code": "NOT_FOUND", "message": "Wallet not found", "details": {}}});
    for path in [
        format!("/api/v1/wallets/{usdc}"),
        format!("/api/v1/wallets/{usdc}/balance"),
        format!("/api/v1/wallets/{usdc}/deposit-addresses?network=base"),
        format!("/api/v1/wallets/{}", Uuid::from_u128(9)),
        "/api/v1/wallets/not-a-wallet/balance".to_owned(),
    ] {
        let (status, body) = get(&app, &path, Some(&bob_cookie)).await;
        assert_eq!(
            (status, &body),
            (StatusCode::NOT_FOUND, &not_found),
            "{path}"
        );
    }
    let (status, body) = call(
        &app,
        Method::POST,
        &format!("/api/v1/wallets/{usdc}/deposit-addresses"),
        Some(&bob_cookie),
        Some(json!({"network": "base"})),
    )
    .await;
    assert_eq!((status, &body), (StatusCode::NOT_FOUND, &not_found));

    // Deposit addresses: only explicitly supported networks, never a placeholder.
    let addresses = format!("/api/v1/wallets/{usdc}/deposit-addresses");
    let (status, body) = get(
        &app,
        &format!("{addresses}?network=base"),
        Some(&alice_cookie),
    )
    .await;
    assert_eq!(
        (status, code(&body)),
        (StatusCode::UNPROCESSABLE_ENTITY, "UNSUPPORTED_NETWORK")
    );
    for query in ["", "?network=base&network=polygon", "?chain=base"] {
        let (status, body) = get(&app, &format!("{addresses}{query}"), Some(&alice_cookie)).await;
        assert_eq!(
            (status, code(&body)),
            (StatusCode::BAD_REQUEST, "VALIDATION_ERROR"),
            "{query}"
        );
    }
    for (body, expected) in [
        (json!({"network": "base"}), StatusCode::UNPROCESSABLE_ENTITY),
        (
            json!({"network": "base", "customer_id": alice.to_string()}),
            StatusCode::BAD_REQUEST,
        ),
        (json!({"network": 1}), StatusCode::BAD_REQUEST),
        (json!({}), StatusCode::BAD_REQUEST),
    ] {
        let (status, _) = call(
            &app,
            Method::POST,
            &addresses,
            Some(&alice_cookie),
            Some(body.clone()),
        )
        .await;
        assert_eq!(status, expected, "{body}");
    }
    let without_origin = Request::builder()
        .method(Method::POST)
        .uri(&addresses)
        .header(header::COOKIE, &alice_cookie)
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(json!({"network": "base"}).to_string()))
        .unwrap();
    assert_eq!(
        app.clone().oneshot(without_origin).await.unwrap().status(),
        StatusCode::FORBIDDEN
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM wallets.wallet_provisioning_requests")
            .fetch_one(&pool)
            .await?,
        0
    );

    // Supported networks are those explicitly enabled: none yet.
    let (status, body) = get(&app, "/api/v1/networks", Some(&alice_cookie)).await;
    assert_eq!((status, &body["data"]), (StatusCode::OK, &json!([])));
    let (status, body) = get(&app, "/api/v1/assets/usdc/networks", Some(&alice_cookie)).await;
    assert_eq!((status, &body["data"]), (StatusCode::OK, &json!([])));
    for asset in ["NGN", "DOGE", "x"] {
        let (status, body) = get(
            &app,
            &format!("/api/v1/assets/{asset}/networks"),
            Some(&alice_cookie),
        )
        .await;
        assert_eq!(
            (status, code(&body)),
            (StatusCode::UNPROCESSABLE_ENTITY, "UNSUPPORTED_ASSET"),
            "{asset}"
        );
    }
    Ok(())
}

#[sqlx::test(migrator = "MIGRATOR")]
async fn wallet_reads_fail_closed_when_storage_is_unavailable(pool: PgPool) -> sqlx::Result<()> {
    let (app, runtime) = app(&pool).await?;
    customer(&pool, "walletcarol").await?;
    let cookie = login(&app, "walletcarol").await;
    let (status, _) = get(&app, "/api/v1/wallets", Some(&cookie)).await;
    assert_eq!(status, StatusCode::OK);
    runtime.close().await;
    let (status, body) = get(&app, "/api/v1/wallets", Some(&cookie)).await;
    assert_eq!(
        (status, &body),
        (
            StatusCode::SERVICE_UNAVAILABLE,
            &json!({"error": {"code": "SERVICE_UNAVAILABLE", "message": "Wallets unavailable", "details": {}}})
        ),
        "no balance is ever substituted"
    );
    Ok(())
}
