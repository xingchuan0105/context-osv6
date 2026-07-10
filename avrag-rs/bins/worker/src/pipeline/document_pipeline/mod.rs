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
        filename = %filename,
        document_id = %document_id,
        blocks = document_ir.blocks.len(),
        assets = document_ir.assets.len(),
        elapsed_ms = stage_started.elapsed().as_millis(),
        "ingestion stage parse+validate done"
    );

    // Stage 2 — project IR blocks
    let stage_started = std::time::Instant::now();
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
        filename = %filename,
        document_id = %document_id,
        blocks = document_ir.blocks.len(),
        elapsed_ms = stage_started.elapsed().as_millis(),
        "ingestion stage IR project done"
    );

    // Stage 3 — chunks, assets, multimodal, toc/profile
    let stage_started = std::time::Instant::now();
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
        filename = %filename,
        document_id = %document_id,
        processed_chunk_count = materialize.processed_chunk_count,
        elapsed_ms = stage_started.elapsed().as_millis(),
        "ingestion stage materialize done"
    );

    // Stage 4 — summary (best-effort, non-fatal)
    let stage_started = std::time::Instant::now();
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
        filename = %filename,
        document_id = %document_id,
        elapsed_ms = stage_started.elapsed().as_millis(),
        "ingestion stage summary done"
    );

    // Stage 5 — retrieval index replace
    let stage_started = std::time::Instant::now();
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
        filename = %filename,
        document_id = %document_id,
        elapsed_ms = stage_started.elapsed().as_millis(),
        total_elapsed_ms = pipeline_started.elapsed().as_millis(),
        "ingestion stage index done"
    );

    if materialize.processed_chunk_count == 0 {
        return Err(IngestionError::parse(format!(
            "refusing to complete ingestion for {filename}: zero indexed chunks"
        )));
    }

    Ok(IngestionPipelineMetrics {
        content: materialize.content,
        processed_chunk_count: materialize.processed_chunk_count,
    })
}
