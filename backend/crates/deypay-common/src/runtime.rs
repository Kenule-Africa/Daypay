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

pub fn shutdown_signal() -> io::Result<impl Future<Output = ()>> {
    #[cfg(unix)]
    {
        use tokio::signal::unix::{SignalKind, signal};
        let mut interrupt = signal(SignalKind::interrupt())?;
        let mut terminate = signal(SignalKind::terminate())?;
        Ok(async move {
            tokio::select! {
                _ = interrupt.recv() => (),
                _ = terminate.recv() => (),
            }
        })
    }
    #[cfg(windows)]
    {
        let mut interrupt = tokio::signal::windows::ctrl_c()?;
        Ok(async move {
            interrupt.recv().await;
        })
    }
}
