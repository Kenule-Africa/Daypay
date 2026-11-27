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
    let auth = deypay_api::login::CustomerAuth::from_env(settings.environment, pool.clone())?;
    if let Some(auth) = &auth
        && !auth.ready().await
    {
        return Err("Authentication database is unready".into());
    }
    let readiness_auth = auth.clone();
    // Customer profile and wallet routes exist only behind the customer session guard.
    let profiles = deypay_api::profile::routes(deypay_api::profile::ProfileApi::new(pool.clone()));
    let wallets = deypay_api::wallets::routes(deypay_api::wallets::WalletApi::new(pool.clone()));
    let customer_routes = match &auth {
        Some(auth) => auth.protect(profiles.merge(wallets)),
        None => deypay_api::profile::unavailable().merge(deypay_api::wallets::unavailable()),
    };
    let app = Router::new()
        .route(
            "/health/live",
            get(|| async { health(StatusCode::OK, r#"{"status":"live"}"#) }),
        )
        .route(
            "/health/ready",
            get(move |State(pool)| readiness(pool, readiness_auth)),
        )
        .merge(deypay_api::email_verification::routes(
            deypay_api::email_verification::Policy::from_env(settings.environment)?,
        ))
        .merge(deypay_api::registration::routes(
            settings.environment,
            deypay_api::registration::RegistrationSettings::from_env(settings.environment)?,
        ))
        .merge(deypay_api::usernames::routes())
        .with_state(pool.clone())
        .merge(deypay_api::login::routes(auth.as_ref()))
        .merge(deypay_api::password_recovery::page_routes())
        .merge(customer_routes);
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

async fn readiness(
    pool: DatabasePool,
    auth: Option<std::sync::Arc<deypay_api::login::CustomerAuth>>,
) -> impl IntoResponse {
    let ready = match auth {
        Some(auth) => auth.ready().await,
        None => database::ready(&pool).await,
    };
    if ready {
        health(StatusCode::OK, r#"{"status":"ready"}"#)
    } else {
        tracing::warn!("Database readiness probe failed");
        health(
            StatusCode::SERVICE_UNAVAILABLE,
            r#"{"status":"unavailable"}"#,
        )
    }
}
