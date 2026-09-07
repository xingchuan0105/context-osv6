//! subtexd entrypoint: thin wiring around the daemon core — env config, a
//! recursive watcher per attached root, and the tick loop.

use std::time::Duration;

use notify::Watcher;
use subtexd::{Subtexd, SubtexdConfig};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    dotenvy::dotenv().ok();
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();

    let subtexd = std::sync::Arc::new(Subtexd::new(SubtexdConfig::from_env()?)?);
    tracing::info!("subtexd starting, roots: {:?}", subtexd.root_paths());

    let event_subtexd = subtexd.clone();
    let mut watcher =
        notify::recommended_watcher(move |res: Result<notify::Event, notify::Error>| {
            match res {
                Ok(event) => {
                    for path in event.paths {
                        event_subtexd.mark_path_dirty(&path);
                    }
                }
                Err(e) => tracing::warn!("watch error: {e}"),
            }
        })?;
    for root in subtexd.root_paths() {
        watcher.watch(&root, notify::RecursiveMode::Recursive)?;
    }
    let mut drop_points = subtexd.drop_points();
    for drop in &drop_points {
        watcher.watch(drop, notify::RecursiveMode::NonRecursive)?;
    }

    let mut ticker = tokio::time::interval(Duration::from_millis(200));
    loop {
        tokio::select! {
            _ = tokio::signal::ctrl_c() => {
                tracing::info!("subtexd shutting down");
                break;
            }
            _ = ticker.tick() => {
                let report = subtexd.tick().await?;
                for root in &report.new_roots {
                    watcher.watch(root, notify::RecursiveMode::Recursive)?;
                }
                let current = subtexd.drop_points();
                for drop in current.iter().filter(|p| !drop_points.contains(p)) {
                    watcher.watch(drop, notify::RecursiveMode::NonRecursive)?;
                }
                drop_points = current;
                if report.has_activity() {
                    tracing::info!(
                        reconciled = ?report.reconciled_roots,
                        indexed = report.indexed_files,
                        removed = report.removed_files,
                        failed = report.failed,
                        jobs = report.jobs_done,
                        inbox_seen = report.inbox_seen,
                        inbox_moved = report.inbox_moved,
                        "subtexd tick"
                    );
                }
            }
        }
    }
    Ok(())
}
