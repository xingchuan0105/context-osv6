//! 表格阶段（struct-query W2 S4）：markitdown markdown → grids 提取 → supervision
//! loop → per-doc `<STRUCT_STORE_DIR>/<doc_id>.duckdb` → 表级证据入 PG
//! （`replace_table_evidence_chunks`，幂等先删后插）。
//!
//! 定位：附加阶段，**best-effort**——任何失败只记 warn，不阻断 ingestion 主链
//! （与 summary 阶段同款降级策略；supervision 内部本就有预算兜底终态，
//! 「pipeline 不被 LLM 卡死」）。doc 生命周期：删 doc 时 cleanup 路径删文件
//! （`remove_struct_store_files`）；doc_version 变更重灌时 `write_duckdb` 覆盖 +
//! 证据先删后插，天然重建。

use std::path::PathBuf;

use contracts::auth_runtime::AuthContext;
use tracing::{info, warn};
use uuid::Uuid;

use super::super::processor::PgTaskProcessor;
use crate::ingestion_guard::ensure_ingestion_side_effects_allowed;

/// 与查询侧（rag-core `struct_query.rs`）同一目录约定。
fn struct_store_dir() -> PathBuf {
    std::env::var("STRUCT_STORE_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("storage/struct_store"))
}

fn supervise_max_turns() -> usize {
    std::env::var("STRUCT_SUPERVISE_MAX_TURNS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(40)
}

/// 删除 doc 的 struct_store 产物（duckdb + evidence sidecar）。best-effort。
pub(crate) fn remove_struct_store_files(document_id: Uuid) {
    let dir = struct_store_dir();
    for path in [
        dir.join(format!("{document_id}.duckdb")),
        dir.join(format!("{document_id}.duckdb.evidence.json")),
    ] {
        match std::fs::remove_file(&path) {
            Ok(()) => {
                info!(document_id = %document_id, path = %path.display(), "struct store file removed")
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => {
                warn!(document_id = %document_id, path = %path.display(), error = %error, "struct store file remove failed")
            }
        }
    }
}

/// 表格阶段主入口。`markdown` 为 None 时（非 markitdown 路径，如图片）直接跳过。
pub(crate) async fn stage_struct_tables(
    processor: &PgTaskProcessor,
    task: &ingestion::IngestionTask,
    context: &AuthContext,
    document_id: Uuid,
    filename: &str,
    markdown: Option<&str>,
) {
    let Some(markdown) = markdown else {
        return;
    };
    let input = avrag_struct_supervision::SuperviseInput::from_markdown(
        Some(document_id.to_string()),
        markdown.to_string(),
    );
    if input.grids.is_empty() {
        // 重灌后无表：清掉旧版本可能留下的存储与证据行（幂等）。
        remove_struct_store_files(document_id);
        if let Err(error) = processor
            .storage
            .repo
            .assets()
            .replace_table_evidence_chunks(context, document_id, &[])
            .await
        {
            warn!(stage = "struct_tables", document_id = %document_id, error = %error, "table evidence cleanup on empty grids failed");
        }
        return;
    }

    let Some(llm) = processor.llm.ingestion_llm.clone() else {
        warn!(stage = "struct_tables", document_id = %document_id, filename = %filename,
            "ingestion_llm 未配置；表格阶段跳过（grids 非空）");
        return;
    };
    if let Err(error) = std::fs::create_dir_all(struct_store_dir()) {
        warn!(stage = "struct_tables", document_id = %document_id, error = %error, "struct store dir create failed; table stage skipped");
        return;
    }

    let out_path = struct_store_dir().join(format!("{document_id}.duckdb"));
    let cfg = avrag_struct_supervision::SuperviseConfig {
        max_turns: supervise_max_turns(),
        doc_name: filename.to_string(),
        out_path: out_path.clone(),
        report_path: None,
    };
    let grid_count = input.grids.len();
    let report = match avrag_struct_supervision::supervise(&input, llm.as_ref(), &cfg).await {
        Ok(report) => report,
        Err(error) => {
            // 失败时保留既有 duckdb/证据（同版本重灌：内容仍有效；新版本：旧产物互相一致）。
            warn!(stage = "struct_tables", document_id = %document_id, filename = %filename, error = %error,
                "supervision loop failed; table stage degraded, previous struct store kept");
            return;
        }
    };

    let rows: Vec<avrag_storage_pg::TableEvidenceChunkRow> = report
        .evidence
        .iter()
        .filter_map(|chunk| {
            Uuid::parse_str(&chunk.chunk_id).ok().map(|chunk_id| {
                avrag_storage_pg::TableEvidenceChunkRow {
                    chunk_id,
                    table: chunk.table.clone(),
                    start_line: chunk.start_line as i64,
                    n_rows: chunk.n_rows as i64,
                    md: chunk.md.clone(),
                }
            })
        })
        .collect();
    if rows.len() != report.evidence.len() {
        warn!(stage = "struct_tables", document_id = %document_id,
            "evidence chunk_id 非 uuid，已跳过对应行");
    }

    if let Err(error) = ensure_ingestion_side_effects_allowed(
        &processor.storage.repo,
        context,
        task,
        document_id,
        "table evidence writes",
    )
    .await
    {
        warn!(stage = "struct_tables", document_id = %document_id, error = %error, "table evidence write aborted by ingestion guard");
        return;
    }
    match processor
        .storage
        .repo
        .assets()
        .replace_table_evidence_chunks(context, document_id, &rows)
        .await
    {
        Ok(inserted) => {
            info!(
                stage = "struct_tables",
                document_id = %document_id,
                filename = %filename,
                grids = grid_count,
                evidence_chunks = inserted,
                turns = report.turns,
                budget_exhausted = report.budget_exhausted,
                duckdb = %out_path.display(),
                "struct table stage done"
            );
        }
        Err(error) => {
            warn!(stage = "struct_tables", document_id = %document_id, error = %error, "table evidence insert failed");
        }
    }
}
