//! Process entry point for durable background work.

use deypay_common::{
    config::Settings,
    runtime::{init_logging, shutdown_signal},
};
use deypay_infrastructure::{
    database, email_verification::VerificationWorker, password_recovery::RecoveryWorker,
};
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
    let verification = VerificationWorker::from_env(settings.environment)?;
    let recovery = RecoveryWorker::from_env(settings.environment)?;
    if verification.is_none() && recovery.is_none() {
        tracing::info!("Worker connected; no job handlers registered");
        shutdown.await;
    } else {
        tracing::info!(
            verification = verification.is_some(),
            password_reset = recovery.is_some(),
            "Worker connected; email handlers active"
        );
        tokio::pin!(shutdown);
        loop {
            // Each handler processes at most one email per tick. A failing
            // handler never blocks the other one.
            let mut busy = false;
            let mut failed = false;
            if let Some(worker) = &verification {
                let result = tokio::select! {
                    biased;
                    _ = &mut shutdown => break,
                    result = worker.tick(&pool) => result,
                };
                match result {
                    Ok(worked) => busy |= worked,
                    Err(_) => {
                        tracing::error!("Verification email database operation failed");
                        failed = true;
                    }
                }
            }
            if let Some(worker) = &recovery {
                let result = tokio::select! {
                    biased;
                    _ = &mut shutdown => break,
                    result = worker.tick(&pool) => result,
                };
                match result {
                    Ok(worked) => busy |= worked,
                    Err(_) => {
                        tracing::error!("Password reset email database operation failed");
                        failed = true;
                    }
                }
            }
            let delay = match (busy, failed) {
                (true, _) => std::time::Duration::ZERO,
                (false, false) => std::time::Duration::from_secs(1),
                (false, true) => std::time::Duration::from_secs(2),
            };
            tokio::select! { biased; _=&mut shutdown=>break, _=tokio::time::sleep(delay)=>{} }
        }
    }
    pool.close().await;
    tracing::info!("Worker stopped");
    Ok(())
}
