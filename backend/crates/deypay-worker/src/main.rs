//! Process entry point for durable background work.

use deypay_common::{
    config::Settings,
    runtime::{init_logging, shutdown_signal},
};
use deypay_infrastructure::database;
use std::error::Error;

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let settings = Settings::from_env()?;
    init_logging(&settings.log_level)?;
    let pool = database::pool(&settings)?;
    if !database::ready(&pool).await {
        tracing::error!("Worker database check failed");
        return Err("Worker startup failed".into());
    }
    tracing::info!("Worker connected; no job handlers registered");
    let signal = shutdown_signal().await;
    pool.close().await;
    tracing::info!("Worker stopped");
    signal?;
    Ok(())
}
