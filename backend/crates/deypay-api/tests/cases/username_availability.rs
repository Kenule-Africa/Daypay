//! IMP-P3-009 advisory username availability through the general API role and
//! the IMP-P3-012 admission. Registration's unique keys remain the authority.
use axum::{
    Router,
    body::{Body, to_bytes},
    http::{Request, StatusCode, header},
};
use deypay_infrastructure::MIGRATOR;
use serde_json::{Value, json};
use sqlx::{PgPool, postgres::PgPoolOptions};
use tower::ServiceExt;

async fn runtime(pool: &PgPool) -> sqlx::Result<PgPool> {
    for script in [
        include_str!("../../../../../infrastructure/postgres/phase-two-grants.psql"),
        include_str!("../../../../../infrastructure/postgres/public-admission-grants.psql"),
        include_str!("../../../../../infrastructure/postgres/username-availability-grants.psql"),
        super::SANDBOX,
    ] {
        sqlx::raw_sql(script).execute(pool).await?;
    }
    sqlx::query(
        "INSERT INTO identity.admission_policy(operation,total_limit,subject_limit)
         VALUES ('username_availability',100,20)",
    )
    .execute(pool)
    .await?;
    super::pin_admission_windows(pool).await?;
    PgPoolOptions::new()
        .max_connections(8)
        .after_connect(|conn, _| {
            Box::pin(async move {
                sqlx::query("SET ROLE deypay_app").execute(conn).await?;
                Ok(())
            })
        })
        .connect_with((*pool.connect_options()).clone())
        .await
}

fn app(runtime: PgPool) -> Router {
    deypay_api::usernames::routes().with_state(runtime)
}

async fn check(app: &Router, query: &str) -> (StatusCode, Value) {
    let res = app
        .clone()
        .oneshot(
            Request::get(format!("/api/v1/usernames/availability{query}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.headers()[header::CACHE_CONTROL], "no-store");
    assert!(res.headers().get(header::SET_COOKIE).is_none());
    let status = res.status();
    let body = to_bytes(res.into_body(), 4096).await.unwrap();
    (status, serde_json::from_slice(&body).unwrap())
}

fn available(value: bool) -> (StatusCode, Value) {
    (StatusCode::OK, json!({"data": {"available": value}}))
}

async fn register(pool: &PgPool, email: &str, handle: &str) -> sqlx::Result<()> {
    sqlx::query("SELECT identity.register_customer($1,'+2348012345678',$2,'$argon2id$test-only','Test','Customer','NG','1.0','1.0',true,true)")
        .bind(email).bind(handle).execute(pool).await?;
    Ok(())
}

#[sqlx::test(migrator = "MIGRATOR")]
async fn claimed_and_reserved_handles_look_alike_and_reveal_no_owner(
    pool: PgPool,
) -> sqlx::Result<()> {
    let app = app(runtime(&pool).await?);
    register(&pool, "ada@example.test", "@Ada_Okafor").await?;
    register(&pool, "old@example.test", "old.handle").await?;
    sqlx::query("UPDATE customers.usernames SET is_active=false,retired_at=clock_timestamp() WHERE normalized_username='old.handle'")
        .execute(&pool).await?;
    sqlx::query("INSERT INTO customers.username_keys VALUES ('reserved_fixture',true)")
        .execute(&pool)
        .await?;

    for query in [
        "?username=ada_okafor",
        "?username=ADA_OKAFOR",
        "?username=%40Ada_Okafor",
        "?username=reserved_fixture",
        "?username=@RESERVED_Fixture",
    ] {
        assert_eq!(check(&app, query).await, available(false), "{query}");
    }
    // Free and retired handles are available; registration still decides.
    for query in ["?username=free.handle", "?username=old.handle"] {
        assert_eq!(check(&app, query).await, available(true), "{query}");
    }
    // The approved format is enforced before any lookup.
    for query in [
        "?username=ab",
        "?username=ada-okafor",
        "?username=mich%D0%B0el",
        "?username=.ada",
        "?username=",
    ] {
        let (status, body) = check(&app, query).await;
        assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "{query}");
        assert_eq!(body["error"]["details"], json!({"fields": ["username"]}));
    }
    for query in [
        "",
        "?user=ada",
        "?username=ada&username=bob",
        "?username=ada&extra=1",
    ] {
        assert_eq!(
            check(&app, query).await.0,
            StatusCode::BAD_REQUEST,
            "{query}"
        );
    }
    // The runtime reads no username rows directly.
    let runtime = runtime_only(&pool).await?;
    for statement in [
        "SELECT count(*) FROM customers.usernames",
        "SELECT count(*) FROM customers.username_keys",
    ] {
        assert!(sqlx::query(statement).execute(&runtime).await.is_err());
    }
    Ok(())
}

async fn runtime_only(pool: &PgPool) -> sqlx::Result<PgPool> {
    PgPoolOptions::new()
        .max_connections(1)
        .after_connect(|conn, _| {
            Box::pin(async move {
                sqlx::query("SET ROLE deypay_app").execute(conn).await?;
                Ok(())
            })
        })
        .connect_with((*pool.connect_options()).clone())
        .await
}

#[sqlx::test(migrator = "MIGRATOR")]
async fn availability_is_advisory_while_concurrent_registration_decides(
    pool: PgPool,
) -> sqlx::Result<()> {
    let app = app(runtime(&pool).await?);
    assert_eq!(check(&app, "?username=racer").await, available(true));
    let mut tasks = tokio::task::JoinSet::new();
    for (i, handle) in ["racer", "RACER", "@Racer", "rAcEr"]
        .into_iter()
        .enumerate()
    {
        let (pool, app) = (pool.clone(), app.clone());
        tasks.spawn(async move {
            let checked = check(&app, "?username=Racer").await;
            register(&pool, &format!("racer{i}@example.test"), handle)
                .await
                .unwrap();
            checked
        });
    }
    while let Some(result) = tasks.join_next().await {
        let (status, body) = result.unwrap();
        assert_eq!(status, StatusCode::OK);
        assert!(body["data"]["available"].is_boolean());
    }
    let holders: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM customers.usernames WHERE normalized_username='racer' AND is_active",
    )
    .fetch_one(&pool)
    .await?;
    assert_eq!(holders, 1);
    assert_eq!(check(&app, "?username=racer").await, available(false));
    Ok(())
}

#[sqlx::test(migrator = "MIGRATOR")]
async fn availability_follows_the_shared_admission_contract(pool: PgPool) -> sqlx::Result<()> {
    let app = app(runtime(&pool).await?);
    // A handle's budget is charged after validation, before the lookup.
    sqlx::query("UPDATE identity.admission_policy SET subject_limit=1 WHERE operation='username_availability'")
        .execute(&pool)
        .await?;
    super::pin_admission_windows(&pool).await?;
    assert_eq!(check(&app, "?username=Daniel").await, available(true));
    let (status, body) = check(&app, "?username=@daniel").await;
    assert_eq!(status, StatusCode::TOO_MANY_REQUESTS);
    assert_eq!(body["error"]["code"], "RATE_LIMITED");
    assert_eq!(check(&app, "?username=other").await, available(true));

    // The deployment budget applies before the query is parsed.
    sqlx::query("UPDATE identity.admission_policy SET total_limit=1,subject_limit=1 WHERE operation='username_availability'")
        .execute(&pool)
        .await?;
    super::pin_admission_windows(&pool).await?;
    assert_eq!(check(&app, "?bad").await.0, StatusCode::BAD_REQUEST);
    assert_eq!(check(&app, "?bad").await.0, StatusCode::TOO_MANY_REQUESTS);

    // Without an operator policy, or without the database, the check is closed.
    sqlx::query("DELETE FROM identity.admission_policy WHERE operation='username_availability'")
        .execute(&pool)
        .await?;
    let (status, body) = check(&app, "?username=daniel").await;
    assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
    assert_eq!(body["error"]["code"], "SERVICE_UNAVAILABLE");
    let closed = deypay_api::usernames::routes().with_state({
        let gone = PgPool::connect_with((*pool.connect_options()).clone()).await?;
        gone.close().await;
        gone
    });
    assert_eq!(
        check(&closed, "?username=daniel").await.0,
        StatusCode::SERVICE_UNAVAILABLE
    );
    Ok(())
}
