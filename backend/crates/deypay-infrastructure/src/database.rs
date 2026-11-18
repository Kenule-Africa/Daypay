use deypay_common::config::{Environment, Settings};
use sqlx::postgres::{PgConnectOptions, PgPoolOptions, PgSslMode};
use sqlx::{ConnectOptions, PgPool};
use std::time::Duration;

pub fn pool(settings: &Settings) -> Result<PgPool, &'static str> {
    let mut options = settings
        .database_url
        .parse::<PgConnectOptions>()
        .map_err(|_| "Invalid PostgreSQL connection settings")?
        .disable_statement_logging();
    if matches!(
        settings.environment,
        Environment::Staging | Environment::Production
    ) {
        options = options.ssl_mode(PgSslMode::VerifyFull);
    }
    Ok(PgPoolOptions::new()
        .max_connections(settings.max_connections)
        .acquire_timeout(Duration::from_secs(2))
        .connect_lazy_with(options))
}

pub async fn ready(pool: &PgPool) -> bool {
    matches!(
        tokio::time::timeout(
            Duration::from_secs(2),
            sqlx::query_scalar::<_, i32>("SELECT 1").fetch_one(pool),
        )
        .await,
        Ok(Ok(1))
    )
}

pub use sqlx::PgPool as DatabasePool;
