use contracts::auth_runtime::AuthContext;
use ingestion::chunker::ChunkPolicy;
use ingestion::parser::{ParsePlan, ParseRouter};
use ingestion::{
    DocumentIr, DocumentIrValidationOptions, IngestionError, IngestionTask,
    sanitize_and_validate_document_ir,
};
use tracing::info;
use uuid::Uuid;

use super::helpers::{
    GraphIndexRecords, ParseRunOutputs, build_asset_object_key, build_document_block_rows,
    build_document_chunk_rows, build_document_index_batch, build_graph_index_records,
    build_text_index_records, collect_document_text,
    enrich_multimodal_source_locator, execute_external_parse, execute_local_parse,
    execute_office_parse, extract_triplets_for_index, extract_visual_triplets_for_index,
    generate_document_profile_with_llm, merge_extracted_triplets, mirror_document_asset,
    triplet_extraction_enabled, visual_triplet_extraction_enabled,
};
use super::processor::PgTaskProcessor;
use crate::indexing::{
    StoredMultimodalChunk, build_multimodal_index_records,
    maybe_enrich_visual_multimodal_summaries, record_multimodal_degrade,
};
use crate::ingestion_guard::{ensure_ingestion_side_effects_allowed, from_storage_error};
use crate::pdf;
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
                processor.pdf_renderer_client.clone(),
                processor.ingestion_llm.clone(),
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

pub(crate) struct RunDocumentPipelineParams<'a> {
    pub(crate) task: &'a IngestionTask,
    pub(crate) context: &'a AuthContext,
    pub(crate) notebook_id: Uuid,
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
        notebook_id,
        document_id,
        parse_run_id,
        bytes,
        filename,
        object_path,
        route_decision,
    } = params;

    // Stage 1 — parse + validate
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

    // Stage 2 — project IR blocks
    stage_project_document_ir(
        processor,
        task,
        context,
        notebook_id,
        document_id,
        parse_run_id,
        &document_ir,
    )
    .await?;

    // Stage 3 — chunks, assets, multimodal, toc/profile
    let materialize = stage_materialize_chunks_assets_profile(
        processor,
        task,
        context,
        notebook_id,
        document_id,
        parse_run_id,
        filename,
        &document_ir,
        parse_run_state,
    )
    .await?;

    // Stage 4 — summary (best-effort, non-fatal)
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

    // Stage 5 — retrieval index replace
    stage_build_and_replace_retrieval_index(
        processor,
        task,
        context,
        notebook_id,
        document_id,
        parse_run_id,
        &document_ir,
        &materialize,
        parse_run_state,
    )
    .await?;

    Ok(IngestionPipelineMetrics {
        content: materialize.content,
        processed_chunk_count: materialize.processed_chunk_count,
    })
}

async fn stage_parse_and_validate_ir(
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

async fn stage_project_document_ir(
    processor: &PgTaskProcessor,
    task: &IngestionTask,
    context: &AuthContext,
    notebook_id: Uuid,
    document_id: Uuid,
    parse_run_id: Uuid,
    document_ir: &DocumentIr,
) -> Result<(), IngestionError> {
    ensure_ingestion_side_effects_allowed(
        &processor.repo,
        context,
        task,
        document_id,
        "IR projection writes",
    )
    .await?;
    processor
        .repo
        .documents()
        .clear_document_ir_projection(context, document_id)
        .await
        .map_err(from_storage_error)?;
    processor
        .repo
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

struct MaterializeOutput {
    content: String,
    processed_chunk_count: usize,
    chunks: Vec<avrag_storage_pg::IndexedChunk>,
    stored_multimodal_chunks: Vec<StoredMultimodalChunk>,
}

async fn stage_materialize_chunks_assets_profile(
    processor: &PgTaskProcessor,
    task: &IngestionTask,
    context: &AuthContext,
    notebook_id: Uuid,
    document_id: Uuid,
    parse_run_id: Uuid,
    filename: &str,
    document_ir: &DocumentIr,
    parse_run_state: &mut ParseRunState,
) -> Result<MaterializeOutput, IngestionError> {
    let chunk_plan =
        ingestion::chunker::build_ir_chunk_plan(document_ir, filename, &ChunkPolicy::default());
    parse_run_state.outputs.text_chunk_count = chunk_plan.text_chunks.len();
    parse_run_state.outputs.multimodal_chunk_count = chunk_plan.multimodal_chunks.len();

    let content = collect_document_text(&chunk_plan);
    let body_chunks = build_document_chunk_rows(&chunk_plan, parse_run_id);

    info!(
        filename = %filename,
        text_chunks = chunk_plan.text_chunks.len(),
        multimodal_chunks = chunk_plan.multimodal_chunks.len(),
        "IR chunk plan generated"
    );

    let chunks = persist_body_chunks(
        processor,
        task,
        context,
        document_id,
        parse_run_id,
        &content,
        &body_chunks,
    )
    .await?;
    let processed_chunk_count = chunks.len().max(1);

    persist_profile_and_toc(
        processor,
        task,
        context,
        notebook_id,
        document_id,
        document_ir,
        filename,
        &chunks,
    )
    .await?;

    let (asset_uuid_by_ref, stored_asset_path_by_ref) = persist_document_assets(
        processor,
        task,
        context,
        notebook_id,
        document_id,
        parse_run_id,
        document_ir,
        parse_run_state,
    )
    .await?;

    let mut stored_multimodal_chunks = persist_multimodal_chunks(
        processor,
        task,
        context,
        notebook_id,
        document_id,
        parse_run_id,
        &chunk_plan,
        &asset_uuid_by_ref,
        &stored_asset_path_by_ref,
        parse_run_state,
    )
    .await?;

    maybe_enrich_visual_multimodal_summaries(
        processor,
        &mut stored_multimodal_chunks,
        &mut parse_run_state.outputs,
    )
    .await;

    Ok(MaterializeOutput {
        content,
        processed_chunk_count,
        chunks,
        stored_multimodal_chunks,
    })
}

async fn persist_body_chunks(
    processor: &PgTaskProcessor,
    task: &IngestionTask,
    context: &AuthContext,
    document_id: Uuid,
    parse_run_id: Uuid,
    content: &str,
    body_chunks: &[avrag_storage_pg::StoreDocumentChunkParams],
) -> Result<Vec<avrag_storage_pg::IndexedChunk>, IngestionError> {
    ensure_ingestion_side_effects_allowed(
        &processor.repo,
        context,
        task,
        document_id,
        "body chunk writes",
    )
    .await?;
    processor
        .repo
        .bootstrap()
        .store_document_body_chunks(
            context,
            document_id,
            Some(parse_run_id),
            content,
            body_chunks,
        )
        .await
        .map_err(from_storage_error)
}

async fn persist_profile_and_toc(
    processor: &PgTaskProcessor,
    task: &IngestionTask,
    context: &AuthContext,
    notebook_id: Uuid,
    document_id: Uuid,
    document_ir: &DocumentIr,
    filename: &str,
    chunks: &[avrag_storage_pg::IndexedChunk],
) -> Result<(), IngestionError> {
    let profile_result =
        generate_document_profile_with_llm(processor, document_id, document_ir, chunks, filename)
            .await;
    if !profile_result.toc_entries.is_empty() {
        ensure_ingestion_side_effects_allowed(
            &processor.repo,
            context,
            task,
            document_id,
            "toc writes",
        )
        .await?;
        if let Err(error) = processor
            .repo
            .bootstrap()
            .replace_document_toc(context, notebook_id, document_id, &profile_result.toc_entries)
            .await
        {
            info!(document_id = %document_id, error = %error, "failed to write document toc");
        } else {
            info!(
                document_id = %document_id,
                toc_count = profile_result.toc_entries.len(),
                "document toc written"
            );
        }
    }
    if let Some(profile_metadata) = profile_result.profile_metadata {
        ensure_ingestion_side_effects_allowed(
            &processor.repo,
            context,
            task,
            document_id,
            "profile metadata write",
        )
        .await?;
        if let Err(error) = processor
            .repo
            .documents()
            .update_document_profile(
                context,
                document_id,
                &profile_metadata,
                Some(&task.task_id),
                task.lock_token.as_deref(),
            )
            .await
        {
            info!(document_id = %document_id, error = %error, "failed to write document profile metadata");
        }
    }
    Ok(())
}

async fn persist_document_assets(
    processor: &PgTaskProcessor,
    task: &IngestionTask,
    context: &AuthContext,
    notebook_id: Uuid,
    document_id: Uuid,
    parse_run_id: Uuid,
    document_ir: &DocumentIr,
    parse_run_state: &mut ParseRunState,
) -> Result<
    (
        std::collections::HashMap<String, Uuid>,
        std::collections::HashMap<String, Option<String>>,
    ),
    IngestionError,
> {
    let mut asset_uuid_by_ref = std::collections::HashMap::new();
    let mut stored_asset_path_by_ref = std::collections::HashMap::new();

    ensure_ingestion_side_effects_allowed(
        &processor.repo,
        context,
        task,
        document_id,
        "asset writes",
    )
    .await?;
    for asset in &document_ir.assets {
        ensure_ingestion_side_effects_allowed(
            &processor.repo,
            context,
            task,
            document_id,
            "asset write",
        )
        .await?;
        let stored_asset_id = Uuid::new_v4();
        asset_uuid_by_ref.insert(asset.asset_id.clone(), stored_asset_id);
        let stored_asset_object_key = build_asset_object_key(
            context,
            &task.notebook_id,
            &task.document_id,
            stored_asset_id,
            &asset.storage_path,
        );

        let stored_image_path = mirror_document_asset(
            &processor.object_store,
            context,
            &task.notebook_id,
            &task.document_id,
            stored_asset_id,
            &asset.storage_path,
            processor.asset_url_ttl_secs,
        )
        .await
        .map_err(|error| IngestionError::storage_object(error))?;
        if stored_image_path.is_some() {
            parse_run_state.outputs.mirrored_asset_count += 1;
        }

        if let Err(error) = ensure_ingestion_side_effects_allowed(
            &processor.repo,
            context,
            task,
            document_id,
            "asset metadata write",
        )
        .await
        {
            let _ = processor
                .object_store
                .delete(&stored_asset_object_key)
                .await;
            return Err(error);
        }

        let store_result = processor
            .repo
            .assets()
            .store_document_asset(
                context,
                avrag_storage_pg::StoreDocumentAssetParams {
                    asset_id: stored_asset_id,
                    notebook_id,
                    document_id,
                    parse_run_id: Some(parse_run_id),
                    page: asset.page.map(|page| page as i32),
                    asset_kind: asset.asset_kind.as_str().to_string(),
                    storage_path: stored_image_path.clone(),
                    mime_type: asset.mime_type.clone(),
                    width: asset.width.map(|value| value as i32),
                    height: asset.height.map(|value| value as i32),
                    caption: None,
                    parser_backend: asset.parser_backend.as_str().to_string(),
                },
            )
            .await;
        if let Err(error) = store_result {
            let _ = processor
                .object_store
                .delete(&stored_asset_object_key)
                .await;
            return Err(IngestionError::storage_database(error));
        }
        stored_asset_path_by_ref.insert(asset.asset_id.clone(), stored_image_path.clone());
    }
    Ok((asset_uuid_by_ref, stored_asset_path_by_ref))
}

async fn persist_multimodal_chunks(
    processor: &PgTaskProcessor,
    task: &IngestionTask,
    context: &AuthContext,
    notebook_id: Uuid,
    document_id: Uuid,
    parse_run_id: Uuid,
    chunk_plan: &ingestion::chunker::IrChunkPlan,
    asset_uuid_by_ref: &std::collections::HashMap<String, Uuid>,
    stored_asset_path_by_ref: &std::collections::HashMap<String, Option<String>>,
    parse_run_state: &mut ParseRunState,
) -> Result<Vec<StoredMultimodalChunk>, IngestionError> {
    ensure_ingestion_side_effects_allowed(
        &processor.repo,
        context,
        task,
        document_id,
        "multimodal chunk writes",
    )
    .await?;
    let mut stored_multimodal_chunks = Vec::new();
    for multimodal_chunk in &chunk_plan.multimodal_chunks {
        ensure_ingestion_side_effects_allowed(
            &processor.repo,
            context,
            task,
            document_id,
            "multimodal chunk write",
        )
        .await?;
        let asset_id = asset_uuid_by_ref
            .get(&multimodal_chunk.asset_ref)
            .copied()
            .ok_or_else(|| {
                IngestionError::storage(format!(
                    "missing stored asset for multimodal block {}",
                    multimodal_chunk.block_id
                ))
            })?;
        let chunk_id =
            Uuid::parse_str(&multimodal_chunk.chunk_id).unwrap_or_else(|_| Uuid::new_v4());
        let stored_image_path = stored_asset_path_by_ref
            .get(&multimodal_chunk.asset_ref)
            .cloned()
            .flatten()
            .unwrap_or_else(|| multimodal_chunk.image_path.clone());

        processor
            .repo
            .assets()
            .store_multimodal_chunk(
                context,
                avrag_storage_pg::StoreMultimodalChunkParams {
                    chunk_id,
                    notebook_id,
                    document_id,
                    parse_run_id: Some(parse_run_id),
                    asset_id: Some(asset_id),
                    page: multimodal_chunk.page.map(|page| page as i32),
                    context_text: Some(multimodal_chunk.context_text.clone()),
                    caption: multimodal_chunk.caption.clone(),
                    normalized_text: multimodal_chunk.summary_text.clone(),
                    parser_backend: multimodal_chunk.parser_backend.as_str().to_string(),
                    metadata: serde_json::json!({
                        "block_id": multimodal_chunk.block_id,
                        "block_type": multimodal_chunk.block_type.as_str(),
                        "source_locator": enrich_multimodal_source_locator(
                            &multimodal_chunk.source_locator,
                            &multimodal_chunk.metadata,
                        ),
                        "source_image_path": multimodal_chunk.image_path,
                        "ingest_route": multimodal_chunk.metadata.get("ingest_route"),
                        "page_range_start": multimodal_chunk.metadata.get("page_range_start"),
                        "page_range_end": multimodal_chunk.metadata.get("page_range_end"),
                        "fusion_asset_refs": multimodal_chunk.metadata.get("fusion_asset_refs"),
                    }),
                },
            )
            .await
            .map_err(from_storage_error)?;

        let fusion_asset_refs = multimodal_chunk
            .metadata
            .get("fusion_asset_refs")
            .map(|refs| {
                refs.split(',')
                    .map(str::trim)
                    .filter(|asset_ref| !asset_ref.is_empty())
                    .map(str::to_string)
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        let mut fusion_image_paths = Vec::new();
        for asset_ref in &fusion_asset_refs {
            if let Some(Some(path)) = stored_asset_path_by_ref.get(asset_ref) {
                fusion_image_paths.push(path.clone());
            }
        }
        if fusion_asset_refs.len() > 1 {
            if fusion_image_paths.len() != fusion_asset_refs.len() {
                record_multimodal_degrade(
                    &mut parse_run_state.outputs,
                    format!(
                        "chunk {chunk_id}: fusion assets mirrored {}/{}",
                        fusion_image_paths.len(),
                        fusion_asset_refs.len()
                    ),
                );
            }
            if fusion_image_paths.len() <= 1 {
                fusion_image_paths.clear();
            }
        } else {
            fusion_image_paths.clear();
        }

        stored_multimodal_chunks.push(StoredMultimodalChunk {
            chunk_id,
            asset_id,
            image_path: stored_image_path,
            fusion_image_paths,
            caption: multimodal_chunk.caption.clone(),
            context_text: multimodal_chunk.context_text.clone(),
            page: multimodal_chunk.page.map(i64::from),
            chunk_type: multimodal_chunk.block_type.as_str().to_string(),
            parser_backend: multimodal_chunk.parser_backend.as_str().to_string(),
            source_locator: Some(enrich_multimodal_source_locator(
                &multimodal_chunk.source_locator,
                &multimodal_chunk.metadata,
            )),
        });
    }
    Ok(stored_multimodal_chunks)
}

async fn stage_build_and_replace_retrieval_index(
    processor: &PgTaskProcessor,
    task: &IngestionTask,
    context: &AuthContext,
    notebook_id: Uuid,
    document_id: Uuid,
    parse_run_id: Uuid,
    document_ir: &DocumentIr,
    materialize: &MaterializeOutput,
    parse_run_state: &mut ParseRunState,
) -> Result<(), IngestionError> {
    let needs_text_vector_index = processor.retrieval_data_plane.is_some();
    let text_index_records = if needs_text_vector_index {
        build_text_index_records(processor, &materialize.chunks).await?
    } else {
        Vec::new()
    };
    if !text_index_records.is_empty() {
        parse_run_state.outputs.text_vector_count = text_index_records.len();
    }

    let needs_multimodal_vector_index = processor.retrieval_data_plane.is_some();
    let multimodal_index_records = if needs_multimodal_vector_index {
        build_multimodal_index_records(
            processor,
            document_ir,
            &materialize.stored_multimodal_chunks,
            &mut parse_run_state.outputs,
        )
        .await?
    } else {
        Vec::new()
    };
    if !multimodal_index_records.is_empty() {
        parse_run_state.outputs.multimodal_vector_count = multimodal_index_records.len();
    }

    let graph_records = if processor.retrieval_data_plane.is_some() && triplet_extraction_enabled()
    {
        let mut extraction = extract_triplets_for_index(
            processor,
            document_id,
            &text_index_records,
            parse_run_state,
        )
        .await;
        if visual_triplet_extraction_enabled() {
            let visual = extract_visual_triplets_for_index(
                processor,
                document_id,
                &materialize.stored_multimodal_chunks,
                parse_run_state,
            )
            .await;
            extraction.total_tokens = extraction.total_tokens.saturating_add(visual.total_tokens);
            extraction.triplets = merge_extracted_triplets(extraction.triplets, visual.triplets);
        }
        if extraction.total_tokens > 0 {
            let _ = processor
                .repo
                .sessions()
                .record_usage_event(
                    context,
                    "triplet_extraction_tokens",
                    i64::from(extraction.total_tokens),
                    "worker_ingestion",
                )
                .await;
        }
        build_graph_index_records(processor, extraction.triplets, parse_run_state).await
    } else {
        GraphIndexRecords::default()
    };

    if let Some(data_plane) = &processor.retrieval_data_plane {
        ensure_ingestion_side_effects_allowed(
            &processor.repo,
            context,
            task,
            document_id,
            "retrieval index replace",
        )
        .await?;
        let batch = build_document_index_batch(
            context,
            Some(notebook_id),
            document_id,
            parse_run_id,
            text_index_records,
            multimodal_index_records,
            graph_records,
        );
        let report = data_plane
            .replace_document_index(batch)
            .await
            .map_err(|error| {
                IngestionError::index(format!("retrieval data plane indexing failed: {error}"))
            })?;
        parse_run_state.outputs.text_vector_count = report.text_chunk_count;
        parse_run_state.outputs.multimodal_vector_count = report.multimodal_chunk_count;
        parse_run_state.outputs.entity_count = report.entity_count;
        parse_run_state.outputs.relation_count = report.relation_count;
        parse_run_state.outputs.graph_passage_count = report.graph_passage_count;
    }

    Ok(())
}

async fn generate_document_summary(
    processor: &PgTaskProcessor,
    context: &AuthContext,
    task: &IngestionTask,
    document_id: Uuid,
    filename: &str,
    content: &str,
    title: &str,
) {
    let Some(ref summary_gen) = processor.summary_generator else {
        return;
    };
    let user_uuid = task
        .requested_by
        .as_deref()
        .and_then(|value| Uuid::parse_str(value).ok());
    let mut skip_llm_summary = false;

    if let (Some(svc), Some(user_id)) = (&processor.usage_limit, user_uuid) {
        match svc.check_quota(context.org_id().into_uuid(), user_id).await {
            Ok(quota) => {
                if quota.blocked_5h || quota.blocked_7d {
                    info!(document_id = %document_id, user_id = %user_id, "skipping LLM summary — quota exhausted");
                    skip_llm_summary = true;
                }
            }
            Err(error) => {
                info!(document_id = %document_id, error = %error, "quota check failed; skipping LLM summary (fail-closed)");
                skip_llm_summary = true;
            }
        }
    }

    if skip_llm_summary {
        return;
    }

    let generated_summary = summary_gen
        .synthesize(&document_id.to_string(), title, filename, content)
        .await;

    let Ok((summary, llm_usage)) = generated_summary else {
        info!(document_id = %document_id, "Summary generation failed, keeping naive fallback");
        return;
    };

    if ensure_ingestion_side_effects_allowed(
        &processor.repo,
        context,
        task,
        document_id,
        "summary update",
    )
    .await
    .is_ok()
    {
        if let Err(error) = processor
            .repo
            .documents()
            .update_document_summary(
                context,
                document_id,
                &summary,
                Some(&task.task_id),
                task.lock_token.as_deref(),
            )
            .await
        {
            info!(document_id = %document_id, error = %error, "failed to update document summary");
        }
    }

    if let (Some(svc), Some(user_id)) = (&processor.usage_limit, user_uuid) {
        let ctx = avrag_billing::usage_limit::MeteringContext {
            user_id,
            org_id: context.org_id().into_uuid(),
            feature: avrag_billing::usage_limit::BillableFeature::Summary,
            stage: "worker_summary".to_string(),
            session_id: None,
            document_id: Some(document_id),
            request_id: None,
            trace_id: None,
        };
        if let Err(error) = svc
            .record_usage(
                &ctx,
                avrag_billing::usage_limit::UsageRecord {
                    provider: &llm_usage.provider,
                    model: &llm_usage.model,
                    prompt_tokens: llm_usage.prompt_tokens,
                    completion_tokens: llm_usage.completion_tokens,
                    total_tokens: llm_usage.total_tokens,
                    usage_source: avrag_billing::usage_limit::UsageSource::Actual,
                },
            )
            .await
        {
            info!(document_id = %document_id, error = %error, "failed to record summary usage");
        }
    }

    if let (Some(analytics), Some(user_id)) = (&processor.analytics, user_uuid) {
        let event = analytics::CostEvent {
            event_id: Uuid::new_v4(),
            event_time: chrono::Utc::now(),
            user_id,
            session_id: None,
            notebook_id: None,
            event_name: analytics::CostEventName::SummaryUsageMetered,
            feature: "summary".to_string(),
            provider: if llm_usage.provider.trim().is_empty() {
                "unknown".to_string()
            } else {
                llm_usage.provider.clone()
            },
            model: if llm_usage.model.trim().is_empty() {
                "unknown".to_string()
            } else {
                llm_usage.model.clone()
            },
            prompt_tokens: i64::from(llm_usage.prompt_tokens),
            completion_tokens: i64::from(llm_usage.completion_tokens),
            embedding_tokens: 0,
            usage_units: avrag_billing::usage_limit::compute_usage_units(
                &llm_usage.provider,
                &llm_usage.model,
                llm_usage.prompt_tokens,
                llm_usage.completion_tokens,
            ),
            storage_bytes_delta: 0,
            external_call_count: 0,
            source: "worker".to_string(),
            metadata: serde_json::json!({
                "task_id": task.task_id,
                "document_id": document_id,
                "filename": filename,
            }),
        };
        if let Err(error) = analytics.record_cost_event(&event).await {
            info!(document_id = %document_id, error = %error, "failed to record summary analytics event");
        }
    }
}
