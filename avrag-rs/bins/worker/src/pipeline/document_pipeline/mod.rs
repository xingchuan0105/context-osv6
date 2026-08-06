use contracts::auth_runtime::AuthContext;
use ingestion::{DocumentIr, IngestionError, IngestionTask};
use tracing::info;
use uuid::Uuid;

use super::helpers::ParseRunOutputs;
use super::processor::PgTaskProcessor;

#[derive(Debug, Default, Clone)]
pub(crate) struct ParseRunState {
    pub(crate) document_ir: Option<DocumentIr>,
    pub(crate) validation_warnings: Vec<ingestion::DocumentIrValidationIssue>,
    pub(crate) outputs: ParseRunOutputs,
    /// markitdown 产出的 markdown 原文（表格阶段消费；图片等非 markitdown 路径为 None）。
    pub(crate) markdown: Option<String>,
    /// Windowed PS+triplet result held for the index stage (design 2026-08-06).
    pub(crate) pending_triplets: Option<super::triplet_extraction::TripletExtractionOutput>,
}

pub(crate) struct IngestionPipelineMetrics {
    pub(crate) content: String,
    pub(crate) processed_chunk_count: usize,
}

mod index;
mod materialize;
mod parse;
mod profile;
mod struct_stage;

use index::stage_build_and_replace_retrieval_index;
use materialize::stage_materialize_chunks_assets_profile;
use parse::{stage_parse_and_validate_ir, stage_project_document_ir};
use profile::generate_document_summary;
pub(crate) use struct_stage::StructTablesOutcome;
pub(crate) use struct_stage::remove_struct_store_files;
use struct_stage::{stage_struct_line_map, stage_struct_tables};

pub(crate) struct RunDocumentPipelineParams<'a> {
    pub(crate) task: &'a IngestionTask,
    pub(crate) context: &'a AuthContext,
    pub(crate) workspace_id: Uuid,
    pub(crate) document_id: Uuid,
    pub(crate) parse_run_id: Uuid,
    pub(crate) bytes: &'a [u8],
    pub(crate) filename: &'a str,
    pub(crate) route_decision: &'a ingestion::parser::ParseRouteDecision,
}

pub(crate) async fn run_document_pipeline(
    processor: &PgTaskProcessor,
    params: RunDocumentPipelineParams<'_>,
    parse_run_state: &mut ParseRunState,
) -> Result<IngestionPipelineMetrics, IngestionError> {
    let RunDocumentPipelineParams {
        task,
        context,
        workspace_id,
        document_id,
        parse_run_id,
        bytes,
        filename,
        route_decision,
    } = params;

    let pipeline_started = std::time::Instant::now();

    // Stage 1 — parse + validate
    let stage_started = std::time::Instant::now();
    info!(
        stage = "parse_validate",
        document_id = %document_id,
        filename = %filename,
        attempt_count = task.attempt_count,
        "ingestion stage begin"
    );
    let document_ir = stage_parse_and_validate_ir(
        bytes,
        filename,
        document_id,
        route_decision,
        parse_run_state,
    )
    .await?;
    info!(
        stage = "parse_validate",
        filename = %filename,
        document_id = %document_id,
        attempt_count = task.attempt_count,
        blocks = document_ir.blocks.len(),
        assets = document_ir.assets.len(),
        elapsed_ms = stage_started.elapsed().as_millis(),
        "ingestion stage done"
    );

    // Stage 2 — project IR blocks
    let stage_started = std::time::Instant::now();
    info!(
        stage = "ir_project",
        document_id = %document_id,
        filename = %filename,
        attempt_count = task.attempt_count,
        "ingestion stage begin"
    );
    stage_project_document_ir(
        processor,
        task,
        context,
        workspace_id,
        document_id,
        parse_run_id,
        &document_ir,
    )
    .await?;
    info!(
        stage = "ir_project",
        filename = %filename,
        document_id = %document_id,
        attempt_count = task.attempt_count,
        blocks = document_ir.blocks.len(),
        elapsed_ms = stage_started.elapsed().as_millis(),
        "ingestion stage done"
    );

    // Stage 2.5 — struct tables（best-effort，不阻断主链）
    let stage_started = std::time::Instant::now();
    let struct_outcome = stage_struct_tables(
        processor,
        task,
        context,
        document_id,
        filename,
        parse_run_state.markdown.as_deref(),
    )
    .await;
    info!(
        stage = "struct_tables",
        filename = %filename,
        document_id = %document_id,
        attempt_count = task.attempt_count,
        has_markdown = parse_run_state.markdown.is_some(),
        outcome = ?struct_outcome,
        elapsed_ms = stage_started.elapsed().as_millis(),
        "ingestion stage done"
    );

    // Stage 3 — chunks, assets, multimodal, toc/profile
    let stage_started = std::time::Instant::now();
    info!(
        stage = "materialize",
        document_id = %document_id,
        filename = %filename,
        attempt_count = task.attempt_count,
        "ingestion stage begin"
    );
    let materialize = stage_materialize_chunks_assets_profile(
        processor,
        task,
        context,
        workspace_id,
        document_id,
        parse_run_id,
        filename,
        &document_ir,
        parse_run_state,
    )
    .await?;
    info!(
        stage = "materialize",
        filename = %filename,
        document_id = %document_id,
        attempt_count = task.attempt_count,
        processed_chunk_count = materialize.processed_chunk_count,
        elapsed_ms = stage_started.elapsed().as_millis(),
        "ingestion stage done"
    );

    // Stage 3.5 — struct line map（best-effort，不阻断主链）。body chunks 此刻在
    // PG chunks 表、尚未被 index 阶段迁走；须紧随 materialize 成功之后。
    // H1 gating：仅当 struct_tables 本轮成功重建 duckdb 时才写 _line_map；
    // 旧库保留时跳过，防止新版 body chunk 行区间映射到旧库行号（静默错配）。
    if struct_outcome == StructTablesOutcome::Rebuilt {
        let stage_started = std::time::Instant::now();
        stage_struct_line_map(processor, context, document_id).await;
        info!(
            stage = "struct_line_map",
            filename = %filename,
            document_id = %document_id,
            attempt_count = task.attempt_count,
            elapsed_ms = stage_started.elapsed().as_millis(),
            "ingestion stage done"
        );
    }

    // Stage 4 — windowed profile+summary+triplet (best-effort, non-fatal)
    let stage_started = std::time::Instant::now();
    info!(
        stage = "windowed_ingestion_llm",
        document_id = %document_id,
        filename = %filename,
        attempt_count = task.attempt_count,
        "ingestion stage begin"
    );
    generate_document_summary(
        processor,
        context,
        task,
        document_id,
        workspace_id,
        filename,
        &document_ir.title,
        &materialize.content,
        parse_run_state,
    )
    .await;
    info!(
        stage = "windowed_ingestion_llm",
        filename = %filename,
        document_id = %document_id,
        attempt_count = task.attempt_count,
        elapsed_ms = stage_started.elapsed().as_millis(),
        "ingestion stage done"
    );

    // Stage 5 — retrieval index replace
    let stage_started = std::time::Instant::now();
    info!(
        stage = "index",
        document_id = %document_id,
        filename = %filename,
        attempt_count = task.attempt_count,
        "ingestion stage begin"
    );
    stage_build_and_replace_retrieval_index(
        processor,
        task,
        context,
        workspace_id,
        document_id,
        parse_run_id,
        &document_ir,
        &materialize,
        parse_run_state,
    )
    .await?;
    info!(
        stage = "index",
        filename = %filename,
        document_id = %document_id,
        attempt_count = task.attempt_count,
        elapsed_ms = stage_started.elapsed().as_millis(),
        total_elapsed_ms = pipeline_started.elapsed().as_millis(),
        "ingestion stage done"
    );

    let body_chunks = materialize.chunks.len();
    let multimodal_chunks = materialize.stored_multimodal_chunks.len();
    info!(
        stage = "terminal",
        document_id = %document_id,
        filename = %filename,
        attempt_count = task.attempt_count,
        body_chunks,
        multimodal_chunks,
        processed_chunk_count = materialize.processed_chunk_count,
        "ingestion terminal integrity check"
    );

    if materialize.processed_chunk_count == 0 {
        tracing::error!(
            stage = "terminal",
            document_id = %document_id,
            filename = %filename,
            body_chunks,
            multimodal_chunks,
            "refusing completed: zero indexed chunks"
        );
        return Err(IngestionError::empty_index(
            document_id,
            format!("refusing to complete ingestion for {filename}: zero indexed chunks"),
        ));
    }

    // materialize.processed_chunk_count is max(body, multimodal); both counts are logged above
    // so ops can see which side is empty when only one modality is present.
    debug_assert_eq!(
        materialize.processed_chunk_count,
        body_chunks.max(multimodal_chunks),
        "processed_chunk_count must match max(body, multimodal)"
    );

    // Dual-assert: PG body/multimodal counts must match materialize before status flip.
    // StateSink also re-checks has_content; this catches partial persist mismatches early.
    let (pg_body, pg_multimodal) = processor
        .storage
        .repo
        .documents()
        .document_ingest_content_counts(context, document_id)
        .await
        .map_err(crate::ingestion_guard::from_storage_error)?;
    let pg_body_usize = usize::try_from(pg_body).unwrap_or(0);
    let pg_multimodal_usize = usize::try_from(pg_multimodal).unwrap_or(0);
    info!(
        stage = "terminal",
        document_id = %document_id,
        filename = %filename,
        attempt_count = task.attempt_count,
        body_chunks,
        multimodal_chunks,
        pg_body_chunks = pg_body,
        pg_multimodal_chunks = pg_multimodal,
        "ingestion terminal materialize vs PG dual-check"
    );
    if pg_body_usize != body_chunks || pg_multimodal_usize != multimodal_chunks {
        tracing::error!(
            stage = "terminal",
            document_id = %document_id,
            filename = %filename,
            body_chunks,
            multimodal_chunks,
            pg_body_chunks = pg_body,
            pg_multimodal_chunks = pg_multimodal,
            "materialize counts disagree with PG before completed"
        );
        return Err(IngestionError::empty_index(
            document_id,
            format!(
                "refusing completed for {filename}: materialize body={body_chunks} multimodal={multimodal_chunks} vs PG body={pg_body} multimodal={pg_multimodal}"
            ),
        ));
    }
    if pg_body == 0 && pg_multimodal == 0 {
        return Err(IngestionError::empty_index(
            document_id,
            format!("refusing completed for {filename}: PG has no body or multimodal chunks"),
        ));
    }

    Ok(IngestionPipelineMetrics {
        content: materialize.content,
        processed_chunk_count: materialize.processed_chunk_count,
    })
}
