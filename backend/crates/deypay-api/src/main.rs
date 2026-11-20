//! HTTP entry point for the DEYPAY API.

use axum::{
    Router,
    extract::State,
    http::{StatusCode, header},
    response::IntoResponse,
    routing::get,
};
use deypay_common::{
    config::{Settings, api_bind},
    runtime::{init_logging, shutdown_signal},
};
use deypay_infrastructure::database::{self, DatabasePool};
use std::error::Error;
use tokio::net::TcpListener;

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let settings = Settings::from_env()?;
    init_logging(&settings.log_level)?;
    let address = api_bind(std::env::var("API_BIND_ADDR").ok().as_deref())?;
    let pool = database::pool(&settings)?;
    let app = Router::new()
        .route(
            "/health/live",
            get(|| async { health(StatusCode::OK, r#"{"status":"live"}"#) }),
        )
        .route("/health/ready", get(readiness))
        .merge(deypay_api::email_verification::routes(
            deypay_api::email_verification::Policy::from_env(settings.environment)?,
        ))
        .merge(deypay_api::registration::routes(
            settings.environment,
            deypay_api::registration::RegistrationSettings::from_env(settings.environment)?,
        ))
        .with_state(pool.clone());
    let listener = TcpListener::bind(address).await?;
    let shutdown = shutdown_signal()?;
    tracing::info!(address = %listener.local_addr()?, "API listening");
    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown)
        .await?;
    pool.close().await;
    tracing::info!("API stopped");
    Ok(())
}

fn health(status: StatusCode, body: &'static str) -> impl IntoResponse {
    (
        status,
        [
            (header::CONTENT_TYPE, "application/json"),
            (header::CACHE_CONTROL, "no-store"),
        ],
        body,
    )
}

async fn readiness(State(pool): State<DatabasePool>) -> impl IntoResponse {
    if database::ready(&pool).await {
        health(StatusCode::OK, r#"{"status":"ready"}"#)
    } else {
        tracing::warn!("Database readiness probe failed");
        health(
            StatusCode::SERVICE_UNAVAILABLE,
            r#"{"status":"unavailable"}"#,
        )
    }
}
