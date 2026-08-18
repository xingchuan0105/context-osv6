#![cfg_attr(windows, windows_subsystem = "windows")]

use anyhow::Result;
use app::{AppConfig, AppState};
use tokio::net::TcpListener;
use tracing::info;

#[tokio::main]
async fn main() -> Result<()> {
    let _ = dotenvy::dotenv();
    telemetry::init("avrag-api")?;
    let _ = any_spawner::Executor::init_tokio();

    let addr = std::env::var("AVRAG_API_ADDR").unwrap_or_else(|_| "0.0.0.0:8080".to_string());
    let state = AppState::bootstrap(AppConfig::from_env()).await?;
    let router = transport_http::build_router(state);
    let listener = TcpListener::bind(&addr).await?;
    info!(addr, "avrag api listening");

    axum::serve(listener, router)
        .with_graceful_shutdown(shutdown_signal())
        .await?;

    Ok(())
}

async fn shutdown_signal() {
    // Docker stop sends SIGTERM; ctrl_c covers interactive use.
    #[cfg(unix)]
    {
        let mut sigterm = tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
            .expect("install SIGTERM handler");
        tokio::select! {
            _ = tokio::signal::ctrl_c() => {},
            _ = sigterm.recv() => {},
        }
    }
    #[cfg(not(unix))]
    {
        let _ = tokio::signal::ctrl_c().await;
    }
    tracing::info!("shutdown signal received");
}
