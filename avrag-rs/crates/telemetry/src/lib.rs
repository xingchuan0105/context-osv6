use anyhow::Result;
use tracing_subscriber::{EnvFilter, fmt};

pub mod metrics {
    use std::sync::atomic::{AtomicU64, Ordering};

    pub static PLANNER_LATENCY_MS: AtomicU64 = AtomicU64::new(0);
    pub static RAG_ITEM_COUNT: AtomicU64 = AtomicU64::new(0);
    pub static RAG_DENSE_RECALL_COUNT: AtomicU64 = AtomicU64::new(0);
    pub static RAG_BM25_RECALL_COUNT: AtomicU64 = AtomicU64::new(0);
    pub static RAG_FINAL_TOPK_RETURNED: AtomicU64 = AtomicU64::new(0);

    pub fn record_planner_latency(ms: u64) {
        PLANNER_LATENCY_MS.store(ms, Ordering::Relaxed);
    }

    pub fn record_rag_query(item_count: u64, dense_count: u64, bm25_count: u64, topk: u64) {
        RAG_ITEM_COUNT.store(item_count, Ordering::Relaxed);
        RAG_DENSE_RECALL_COUNT.store(dense_count, Ordering::Relaxed);
        RAG_BM25_RECALL_COUNT.store(bm25_count, Ordering::Relaxed);
        RAG_FINAL_TOPK_RETURNED.store(topk, Ordering::Relaxed);
    }
}

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
