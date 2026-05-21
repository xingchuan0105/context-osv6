use anyhow::Result;
use tracing_subscriber::{EnvFilter, fmt};

pub mod prometheus;

pub fn init(service_name: &str) -> Result<()> {
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));
    let subscriber = fmt()
        .with_target(true)
        .with_env_filter(filter)
        .with_thread_ids(true)
        .with_thread_names(true)
        .finish();

    let _ = tracing::subscriber::set_global_default(subscriber);
    tracing::info!(service_name, "telemetry initialized");
    Ok(())
}
