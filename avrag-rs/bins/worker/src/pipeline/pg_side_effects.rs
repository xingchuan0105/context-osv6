use anyhow::Result;
use avrag_storage_pg::ObjectStoreHandle;
use contracts::auth_runtime::AuthContext;
use ingestion::DocumentIr;
use std::path::Path;
use tracing::warn;
use uuid::Uuid;

pub(crate) fn build_document_block_rows(
    document_ir: &DocumentIr,
    parse_run_id: Uuid,
) -> Vec<avrag_storage_pg::StoredDocumentBlock> {
    document_ir
        .blocks
        .iter()
        .map(|block| avrag_storage_pg::StoredDocumentBlock {
            block_id: block.block_id.clone(),
            parse_run_id: Some(parse_run_id),
            page: block
                .page
                .or(block.source_locator.page)
                .map(|page| page as i32),
            block_type: block.block_type.as_str().to_string(),
            modality: block.modality.as_str().to_string(),
            text: block.text.clone(),
            summary_text: block.alt_text.clone(),
            caption: block.caption.clone(),
            asset_refs: serde_json::json!(block.asset_refs),
            section_path: serde_json::json!(block.section_path),
            source_locator_json: serde_json::json!(block.source_locator),
            parser_backend: block.parser_backend.as_str().to_string(),
            metadata_json: serde_json::json!(block.metadata),
        })
        .collect()
}

pub(crate) fn build_document_chunk_rows(
    chunk_plan: &ingestion::chunker::IrChunkPlan,
    parse_run_id: Uuid,
) -> Vec<avrag_storage_pg::StoreDocumentChunkParams> {
    chunk_plan
        .text_chunks
        .iter()
        .map(|chunk| avrag_storage_pg::StoreDocumentChunkParams {
            parse_run_id: Some(parse_run_id),
            page: chunk.page.map(|page| page as i32),
            content: chunk.text.clone(),
            metadata: serde_json::json!({
                "kind": chunk.block_type.as_str(),
                "cursor": chunk.cursor,
                "page": chunk.page,
                "block_id": chunk.block_id,
                "block_type": chunk.block_type.as_str(),
                "parser_backend": chunk.parser_backend.as_str(),
                "source_locator": chunk.source_locator,
                "section_path": chunk.section_path,
                "block_metadata": chunk.metadata,
            }),
        })
        .collect()
}

pub(crate) fn collect_document_text(chunk_plan: &ingestion::chunker::IrChunkPlan) -> String {
    chunk_plan
        .text_chunks
        .iter()
        .map(|chunk| chunk.text.as_str())
        .collect::<Vec<_>>()
        .join("\n\n")
}

pub(crate) fn build_asset_object_key(
    context: &AuthContext,
    workspace_id: &str,
    document_id: &str,
    asset_id: Uuid,
    source_path: &str,
) -> String {
    let extension = infer_asset_extension(source_path).unwrap_or("bin");
    format!(
        "{}/{}/{}/assets/{}.{}",
        context.user_id(),
        workspace_id,
        document_id,
        asset_id,
        extension
    )
}

fn infer_asset_extension(path: &str) -> Option<&'static str> {
    common::infer_image_extension(path)
}

pub(crate) async fn mirror_document_asset(
    object_store: &ObjectStoreHandle,
    context: &AuthContext,
    workspace_id: &str,
    document_id: &str,
    asset_id: Uuid,
    source_path: &str,
    ttl_secs: u64,
) -> Result<Option<String>> {
    if source_path.trim().is_empty() {
        return Ok(None);
    }

    let object_key =
        build_asset_object_key(context, workspace_id, document_id, asset_id, source_path);
    if common::is_remote_url(source_path) {
        return mirror_remote_asset(object_store, source_path, &object_key, ttl_secs)
            .await
            .map(Some);
    }

    if let Some(local_path) = source_path.strip_prefix("temporary://") {
        let bytes = tokio::fs::read(local_path).await?;
        object_store.put(&object_key, &bytes).await?;
        if let Err(error) = tokio::fs::remove_file(local_path).await {
            warn!(
                path = local_path,
                error = %error,
                "failed to delete temporary page raster file after mirror"
            );
        }
        return finalize_mirrored_asset_path(object_store, &object_key, ttl_secs)
            .await
            .map(Some);
    }

    let local_path = Path::new(source_path);
    if local_path.exists() {
        let bytes = tokio::fs::read(local_path).await?;
        object_store.put(&object_key, &bytes).await?;
        return finalize_mirrored_asset_path(object_store, &object_key, ttl_secs)
            .await
            .map(Some);
    }

    Ok(Some(source_path.to_string()))
}

pub(crate) async fn mirror_remote_asset(
    object_store: &ObjectStoreHandle,
    source_url: &str,
    object_key: &str,
    ttl_secs: u64,
) -> Result<String> {
    let response = reqwest::Client::new()
        .get(source_url)
        .send()
        .await?
        .error_for_status()?;
    let bytes = response.bytes().await?;
    object_store.put(object_key, &bytes).await?;
    finalize_mirrored_asset_path(object_store, object_key, ttl_secs).await
}

pub(crate) async fn finalize_mirrored_asset_path(
    object_store: &ObjectStoreHandle,
    object_key: &str,
    ttl_secs: u64,
) -> Result<String> {
    if object_store.is_remote() {
        object_store
            .presigned_get_url(object_key, ttl_secs.max(60))
            .await
    } else {
        Ok(object_key.to_string())
    }
}

