//! Dedicated credential and session-control listener with its own deployment identity.
//! Route POST login, refresh and logout, GET sessions and DELETE sessions/{session_id}
//! under /api/v1/auth directly here at the trusted HTTPS ingress.
use axum::{Router, http::StatusCode, routing::get};
use deypay_api::login::VerifierAuth;
use deypay_common::{
    config::{Settings, api_bind},
    runtime::{init_logging, shutdown_signal},
};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let settings = Settings::from_env()?;
    init_logging(&settings.log_level)?;
    let auth = VerifierAuth::from_env(&settings).await?;
    let addr = api_bind(std::env::var("API_BIND_ADDR").ok().as_deref())?;
    let routes = auth.routes().merge(
        Router::new()
            .route("/health/live", get(|| async { StatusCode::OK }))
            .route(
                "/health/ready",
                get(move || async move {
                    if auth.ready().await {
                        StatusCode::OK
                    } else {
                        StatusCode::SERVICE_UNAVAILABLE
                    }
                }),
            ),
    );
    let listener = tokio::net::TcpListener::bind(addr).await?;
    let shutdown = shutdown_signal()?;
    tracing::info!(address=%listener.local_addr()?, "Verifier listening");
    axum::serve(listener, routes)
        .with_graceful_shutdown(shutdown)
        .await?;
    Ok(())
}
