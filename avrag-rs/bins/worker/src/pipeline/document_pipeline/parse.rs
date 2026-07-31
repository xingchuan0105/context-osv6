use contracts::auth_runtime::AuthContext;
use ingestion::parser::ParsePlan;
use ingestion::{
    DocumentIr, DocumentIrValidationOptions, IngestionError, IngestionTask,
    sanitize_and_validate_document_ir,
};
use uuid::Uuid;

use super::super::helpers::{build_document_block_rows, execute_external_parse, execute_local_parse};
use super::super::processor::PgTaskProcessor;
use crate::ingestion_guard::{ensure_ingestion_side_effects_allowed, from_storage_error};

use super::ParseRunState;

async fn execute_parse_plan(
    bytes: &[u8],
    filename: &str,
    document_id: Uuid,
    route_decision: &ingestion::parser::ParseRouteDecision,
) -> Result<(DocumentIr, Option<String>), IngestionError> {
    match &route_decision.plan {
        ParsePlan::Local(plan) => {
            execute_local_parse(bytes, filename, document_id, &plan.kind).await
        }
        ParsePlan::External(plan) => {
            let ir = execute_external_parse(bytes, filename, document_id, &plan.kind).await?;
            Ok((ir, None))
        }
    }
}

pub(crate) async fn stage_parse_and_validate_ir(
    bytes: &[u8],
    filename: &str,
    document_id: Uuid,
    route_decision: &ingestion::parser::ParseRouteDecision,
    parse_run_state: &mut ParseRunState,
) -> Result<DocumentIr, IngestionError> {
    let (ir, markdown) = execute_parse_plan(bytes, filename, document_id, route_decision).await?;
    let validation_report = sanitize_and_validate_document_ir(
        ir,
        &DocumentIrValidationOptions::default(),
    )
    .map_err(|error| IngestionError::storage(error))?;

    let document_ir = validation_report.document;
    parse_run_state.validation_warnings = validation_report.warnings;
    parse_run_state.outputs.block_count = document_ir.blocks.len();
    parse_run_state.outputs.asset_count = document_ir.assets.len();
    parse_run_state.markdown = markdown;
    parse_run_state.document_ir = Some(document_ir.clone());

    Ok(document_ir)
}

pub(crate) async fn stage_project_document_ir(
    processor: &PgTaskProcessor,
    task: &IngestionTask,
    context: &AuthContext,
    workspace_id: Uuid,
    document_id: Uuid,
    parse_run_id: Uuid,
    document_ir: &DocumentIr,
) -> Result<(), IngestionError> {
    ensure_ingestion_side_effects_allowed(
        &processor.storage.repo,
        context,
        task,
        document_id,
        "IR projection writes",
    )
    .await?;
    processor.storage.repo
        .documents()
        .clear_document_ir_projection(context, document_id)
        .await
        .map_err(from_storage_error)?;
    processor.storage.repo
        .documents()
        .replace_document_blocks(
            context,
            workspace_id,
            document_id,
            &build_document_block_rows(document_ir, parse_run_id),
        )
        .await
        .map_err(from_storage_error)?;

    Ok(())
}
