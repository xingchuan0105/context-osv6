use contracts::auth_runtime::AuthContext;
use ingestion::parser::{ParsePlan, ParseRouter};
use ingestion::{
    DocumentIr, DocumentIrValidationOptions, IngestionError, IngestionTask,
    sanitize_and_validate_document_ir,
};
use uuid::Uuid;

use super::super::helpers::{
    build_document_block_rows, execute_external_parse, execute_local_parse,
    execute_office_parse,
};
use super::super::processor::PgTaskProcessor;
use crate::ingestion_guard::{ensure_ingestion_side_effects_allowed, from_storage_error};
use crate::pdf;

use super::{ParseRunState};

async fn execute_parse_plan(
    processor: &PgTaskProcessor,
    bytes: &[u8],
    filename: &str,
    object_path: &str,
    document_id: Uuid,
    parse_run_id: Uuid,
    route_decision: &ingestion::parser::ParseRouteDecision,
) -> Result<DocumentIr, IngestionError> {
    match &route_decision.plan {
        ParsePlan::Local(plan) => {
            execute_local_parse(bytes, filename, document_id, &plan.kind).await
        }
        ParsePlan::Office(plan) => {
            execute_office_parse(processor, bytes, filename, document_id, &plan.doc_type).await
        }
        ParsePlan::External(plan) => {
            execute_external_parse(
                processor,
                bytes,
                filename,
                object_path,
                document_id,
                parse_run_id,
                &plan.kind,
            )
            .await
        }
        ParsePlan::Pdf(plan) => {
            let (pdf_bytes, pdf_filename) = pdf::maybe_convert_office_to_pdf(bytes, filename)
                .await
                .map_err(|e| {
                    IngestionError::parse(format!(
                        "office to pdf conversion failed for {filename}: {e}"
                    ))
                })?;

            let (effective_plan, liteparse_snapshot) = if plan.pages.is_empty() {
                let routed = ParseRouter::route(&pdf_bytes, &pdf_filename, "application/pdf")
                    .map_err(|e| IngestionError::storage(e))?;
                match routed.plan {
                    ParsePlan::Pdf(p) => (p, routed.liteparse_snapshot),
                    other => {
                        return Err(IngestionError::parse(format!(
                            "expected pdf plan after office conversion, got {other:?}"
                        )));
                    }
                }
            } else {
                (plan.clone(), route_decision.liteparse_snapshot.clone())
            };

            let ctx = pdf::PdfParseContext::new(
                processor.parse.pdf_renderer_client.clone(),
                processor.llm.ingestion_llm.clone(),
            );
            pdf::execute_pdf_parse(
                &ctx,
                &pdf_bytes,
                &pdf_filename,
                document_id,
                &effective_plan,
                liteparse_snapshot.as_ref(),
            )
            .await
        }
    }
}


pub(crate) async fn stage_parse_and_validate_ir(
    processor: &PgTaskProcessor,
    bytes: &[u8],
    filename: &str,
    object_path: &str,
    document_id: Uuid,
    parse_run_id: Uuid,
    route_decision: &ingestion::parser::ParseRouteDecision,
    parse_run_state: &mut ParseRunState,
) -> Result<DocumentIr, IngestionError> {
    let validation_report = sanitize_and_validate_document_ir(
        execute_parse_plan(
            processor,
            bytes,
            filename,
            object_path,
            document_id,
            parse_run_id,
            route_decision,
        )
        .await?,
        &DocumentIrValidationOptions::default(),
    )
    .map_err(|error| IngestionError::storage(error))?;

    let document_ir = validation_report.document;
    parse_run_state.validation_warnings = validation_report.warnings;
    parse_run_state.outputs.block_count = document_ir.blocks.len();
    parse_run_state.outputs.asset_count = document_ir.assets.len();
    parse_run_state.document_ir = Some(document_ir.clone());

    Ok(document_ir)
}

pub(crate) async fn stage_project_document_ir(
    processor: &PgTaskProcessor,
    task: &IngestionTask,
    context: &AuthContext,
    notebook_id: Uuid,
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
            notebook_id,
            document_id,
            &build_document_block_rows(document_ir, parse_run_id),
        )
        .await
        .map_err(from_storage_error)?;

    Ok(())
}
