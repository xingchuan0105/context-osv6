use contracts::auth_runtime::AuthContext;
use ingestion::chunker::ChunkPolicy;
use ingestion::{
    DocumentIr, IngestionError, IngestionTask,
};
use tracing::info;
use uuid::Uuid;

use super::super::helpers::{
    build_asset_object_key,
    build_document_chunk_rows, collect_document_text,
    enrich_multimodal_source_locator,
    generate_document_profile_with_llm, mirror_document_asset,
};
use super::super::processor::PgTaskProcessor;
use crate::indexing::{
    StoredMultimodalChunk,
    maybe_enrich_visual_multimodal_summaries, record_multimodal_degrade,
};
use crate::ingestion_guard::{ensure_ingestion_side_effects_allowed, from_storage_error};

use super::ParseRunState;

pub(crate) struct MaterializeOutput {
    pub(crate) content: String,
    pub(crate) processed_chunk_count: usize,
    pub(crate) chunks: Vec<avrag_storage_pg::IndexedChunk>,
    pub(crate) stored_multimodal_chunks: Vec<StoredMultimodalChunk>,
}

pub(crate) async fn stage_materialize_chunks_assets_profile(
    processor: &PgTaskProcessor,
    task: &IngestionTask,
    context: &AuthContext,
    workspace_id: Uuid,
    document_id: Uuid,
    parse_run_id: Uuid,
    filename: &str,
    document_ir: &DocumentIr,
    parse_run_state: &mut ParseRunState,
) -> Result<MaterializeOutput, IngestionError> {
    let chunk_plan_started = std::time::Instant::now();
    let chunk_plan =
        ingestion::chunker::build_ir_chunk_plan(document_ir, filename, &ChunkPolicy::default());
    parse_run_state.outputs.text_chunk_count = chunk_plan.text_chunks.len();
    parse_run_state.outputs.multimodal_chunk_count = chunk_plan.multimodal_chunks.len();

    if chunk_plan.text_chunks.is_empty() && chunk_plan.multimodal_chunks.is_empty() {
        return Err(IngestionError::parse(format!(
            "ingestion produced no text or multimodal chunks for {filename} (blocks={})",
            document_ir.blocks.len()
        )));
    }

    let content = collect_document_text(&chunk_plan);
    let body_chunks = build_document_chunk_rows(&chunk_plan, parse_run_id);

    info!(
        filename = %filename,
        text_chunks = chunk_plan.text_chunks.len(),
        multimodal_chunks = chunk_plan.multimodal_chunks.len(),
        elapsed_ms = chunk_plan_started.elapsed().as_millis(),
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
    // Do not inflate with .max(1): empty body must not look like a successful index.
    let processed_chunk_count = chunks.len().max(chunk_plan.multimodal_chunks.len());
    info!(
        filename = %filename,
        body_chunks = chunks.len(),
        multimodal_chunks = chunk_plan.multimodal_chunks.len(),
        "body chunks persisted"
    );

    persist_profile_and_toc(
        processor,
        task,
        context,
        workspace_id,
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
        workspace_id,
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
        workspace_id,
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
        &processor.storage.repo,
        context,
        task,
        document_id,
        "body chunk writes",
    )
    .await?;
    processor.storage.repo
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
    workspace_id: Uuid,
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
            &processor.storage.repo,
            context,
            task,
            document_id,
            "toc writes",
        )
        .await?;
        if let Err(error) = processor.storage.repo
            .bootstrap()
            .replace_document_toc(context, workspace_id, document_id, &profile_result.toc_entries)
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
            &processor.storage.repo,
            context,
            task,
            document_id,
            "profile metadata write",
        )
        .await?;
        if let Err(error) = processor.storage.repo
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
    workspace_id: Uuid,
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
        &processor.storage.repo,
        context,
        task,
        document_id,
        "asset writes",
    )
    .await?;
    for asset in &document_ir.assets {
        ensure_ingestion_side_effects_allowed(
            &processor.storage.repo,
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
            &task.workspace_id,
            &task.document_id,
            stored_asset_id,
            &asset.storage_path,
        );

        let stored_image_path = mirror_document_asset(
            &processor.storage.object_store,
            context,
            &task.workspace_id,
            &task.document_id,
            stored_asset_id,
            &asset.storage_path,
            processor.storage.asset_url_ttl_secs,
        )
        .await
        .map_err(|error| IngestionError::storage_object(error))?;
        if stored_image_path.is_some() {
            parse_run_state.outputs.mirrored_asset_count += 1;
        }

        if let Err(error) = ensure_ingestion_side_effects_allowed(
            &processor.storage.repo,
            context,
            task,
            document_id,
            "asset metadata write",
        )
        .await
        {
            let _ = processor.storage.object_store
                .delete(&stored_asset_object_key)
                .await;
            return Err(error);
        }

        let store_result = processor.storage.repo
            .assets()
            .store_document_asset(
                context,
                avrag_storage_pg::StoreDocumentAssetParams {
                    asset_id: stored_asset_id,
                    workspace_id,
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
            let _ = processor.storage.object_store
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
    workspace_id: Uuid,
    document_id: Uuid,
    parse_run_id: Uuid,
    chunk_plan: &ingestion::chunker::IrChunkPlan,
    asset_uuid_by_ref: &std::collections::HashMap<String, Uuid>,
    stored_asset_path_by_ref: &std::collections::HashMap<String, Option<String>>,
    parse_run_state: &mut ParseRunState,
) -> Result<Vec<StoredMultimodalChunk>, IngestionError> {
    ensure_ingestion_side_effects_allowed(
        &processor.storage.repo,
        context,
        task,
        document_id,
        "multimodal chunk writes",
    )
    .await?;
    let mut stored_multimodal_chunks = Vec::new();
    for multimodal_chunk in &chunk_plan.multimodal_chunks {
        ensure_ingestion_side_effects_allowed(
            &processor.storage.repo,
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

        processor.storage.repo
            .assets()
            .store_multimodal_chunk(
                context,
                avrag_storage_pg::StoreMultimodalChunkParams {
                    chunk_id,
                    workspace_id,
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
