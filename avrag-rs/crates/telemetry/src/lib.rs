use anyhow::Result;
use tracing_subscriber::{EnvFilter, fmt};

pub mod prometheus;

pub fn init(service_name: &str) -> Result<()> {
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));
    // with_ansi(false)：日志写入文件/pipe（worker 子进程 stdout 被 harness 捕获到
    // worker.log）时保持纯文本可解析——ANSI 颜色码会让 grep/sed/脚本无法解析
    // `stage="x"` 等字段。显式关闭而非依赖 TTY 自动检测。
    let subscriber = fmt()
        .with_ansi(false)
        .with_target(true)
        .with_env_filter(filter)
        .with_thread_ids(true)
        .with_thread_names(true)
        .finish();

    let _ = tracing::subscriber::set_global_default(subscriber);
    tracing::info!(service_name, "telemetry initialized");
    Ok(())
}
