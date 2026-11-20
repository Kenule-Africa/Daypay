//! Process entry point for durable background work.

use deypay_common::{
    config::Settings,
    runtime::{init_logging, shutdown_signal},
};
use deypay_infrastructure::{database, email_verification::VerificationWorker};
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
    let shutdown = shutdown_signal()?;
    let worker = VerificationWorker::from_env(settings.environment)?;
    if let Some(worker) = worker {
        tracing::info!("Worker connected; verification email handler active");
        tokio::pin!(shutdown);
        loop {
            let result = tokio::select! {
                biased;
                _ = &mut shutdown => break,
                result = worker.tick(&pool) => result,
            };
            let delay = match result {
                Ok(true) => std::time::Duration::ZERO,
                Ok(false) => std::time::Duration::from_secs(1),
                Err(_) => {
                    tracing::error!("Verification email database operation failed");
                    std::time::Duration::from_secs(2)
                }
            };
            tokio::select! { biased; _=&mut shutdown=>break, _=tokio::time::sleep(delay)=>{} }
        }
    } else {
        tracing::info!("Worker connected; no job handlers registered");
        shutdown.await;
    }
    pool.close().await;
    tracing::info!("Worker stopped");
    Ok(())
}
