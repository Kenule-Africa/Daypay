use std::io;
use tracing_subscriber::{filter::Targets, layer::SubscriberExt, util::SubscriberInitExt};

pub fn init_logging(level: &str) -> Result<(), &'static str> {
    let level = level
        .parse::<tracing::Level>()
        .map_err(|_| "Invalid log level")?;
    tracing_subscriber::registry()
        .with(Targets::new().with_target("deypay", level))
        .with(tracing_subscriber::fmt::layer().json().with_ansi(false))
        .try_init()
        .map_err(|_| "Failed to initialize structured logging")
}

pub async fn shutdown_signal() -> io::Result<()> {
    #[cfg(unix)]
    {
        use tokio::signal::unix::{SignalKind, signal};
        let mut terminate = signal(SignalKind::terminate())?;
        tokio::select! {
            result = tokio::signal::ctrl_c() => result,
            _ = terminate.recv() => Ok(()),
        }
    }
    #[cfg(not(unix))]
    tokio::signal::ctrl_c().await
}
