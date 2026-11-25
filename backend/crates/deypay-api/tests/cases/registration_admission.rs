use super::{post, settings, submission};
use axum::http::{StatusCode, header};
use deypay_api::registration::routes;
use deypay_common::config::Environment;
use deypay_infrastructure::MIGRATOR;
use sqlx::PgPool;
use tower::ServiceExt;

#[sqlx::test(migrator = "MIGRATOR")]
async fn independent_registration_instances_share_admission(pool: PgPool) -> sqlx::Result<()> {
    super::pin_admission_windows(&pool).await?;
    let other = PgPool::connect_with((*pool.connect_options()).clone()).await?;
    let instance = |db| routes(Environment::Test, Some(settings(1, Ok(true)))).with_state(db);
    let request = || post(submission("test@example.test", "test_user"));
    let (left, right) = tokio::join!(
        instance(pool).oneshot(request()),
        instance(other).oneshot(request())
    );
    let mut statuses = [left.unwrap().status(), right.unwrap().status()];
    statuses.sort();
    assert_eq!(
        statuses,
        [
            StatusCode::UNPROCESSABLE_ENTITY,
            StatusCode::TOO_MANY_REQUESTS
        ]
    );
    Ok(())
}

#[sqlx::test(migrator = "MIGRATOR")]
async fn unavailable_registration_admission_fails_before_parsing(pool: PgPool) {
    pool.close().await;
    let instance = routes(Environment::Test, Some(settings(1, Ok(false)))).with_state(pool);
    let response = instance.oneshot(post(serde_json::json!({}))).await.unwrap();
    assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
    assert_eq!(response.headers()[header::CACHE_CONTROL], "no-store");
}
