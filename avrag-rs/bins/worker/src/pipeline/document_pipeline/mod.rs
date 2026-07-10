use contracts::auth_runtime::AuthContext;
use ingestion::{
    DocumentIr, IngestionError, IngestionTask,
};
use tracing::info;
use uuid::Uuid;

use super::helpers::ParseRunOutputs;
use super::processor::PgTaskProcessor;

#[derive(Debug, Default, Clone)]
pub(crate) struct ParseRunState {
    pub(crate) document_ir: Option<DocumentIr>,
    pub(crate) validation_warnings: Vec<ingestion::DocumentIrValidationIssue>,
    pub(crate) outputs: ParseRunOutputs,
}

pub(crate) struct IngestionPipelineMetrics {
    pub(crate) content: String,
    pub(crate) processed_chunk_count: usize,
}

mod parse;
mod materialize;
mod index;
mod profile;

use parse::{stage_parse_and_validate_ir, stage_project_document_ir};
use materialize::stage_materialize_chunks_assets_profile;
use index::stage_build_and_replace_retrieval_index;
use profile::generate_document_summary;

pub(crate) struct RunDocumentPipelineParams<'a> {
    pub(crate) task: &'a IngestionTask,
    pub(crate) context: &'a AuthContext,
    pub(crate) workspace_id: Uuid,
    pub(crate) document_id: Uuid,
    pub(crate) parse_run_id: Uuid,
    pub(crate) bytes: &'a [u8],
    pub(crate) filename: &'a str,
    pub(crate) object_path: &'a str,
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
        object_path,
        route_decision,
    } = params;

    let pipeline_started = std::time::Instant::now();

    // Stage 1 — parse + validate
    let stage_started = std::time::Instant::now();
    info!(
        stage = "parse_validate",
        document_id = %document_id,
        filename = %filename,
        "ingestion stage begin"
    );
    let document_ir = stage_parse_and_validate_ir(
        processor,
        bytes,
        filename,
        object_path,
        document_id,
        parse_run_id,
        route_decision,
        parse_run_state,
    )
    .await?;
    info!(
        stage = "parse_validate",
        filename = %filename,
        document_id = %document_id,
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
        blocks = document_ir.blocks.len(),
        elapsed_ms = stage_started.elapsed().as_millis(),
        "ingestion stage done"
    );

    // Stage 3 — chunks, assets, multimodal, toc/profile
    let stage_started = std::time::Instant::now();
    info!(
        stage = "materialize",
        document_id = %document_id,
        filename = %filename,
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
        processed_chunk_count = materialize.processed_chunk_count,
        elapsed_ms = stage_started.elapsed().as_millis(),
        "ingestion stage done"
    );

    // Stage 4 — summary (best-effort, non-fatal)
    let stage_started = std::time::Instant::now();
    info!(
        stage = "summary",
        document_id = %document_id,
        filename = %filename,
        "ingestion stage begin"
    );
    generate_document_summary(
        processor,
        context,
        task,
        document_id,
        filename,
        materialize.content.as_str(),
        &document_ir.title,
    )
    .await;
    info!(
        stage = "summary",
        filename = %filename,
        document_id = %document_id,
        elapsed_ms = stage_started.elapsed().as_millis(),
        "ingestion stage done"
    );

    // Stage 5 — retrieval index replace
    let stage_started = std::time::Instant::now();
    info!(
        stage = "index",
        document_id = %document_id,
        filename = %filename,
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

    Ok(IngestionPipelineMetrics {
        content: materialize.content,
        processed_chunk_count: materialize.processed_chunk_count,
    })
}
