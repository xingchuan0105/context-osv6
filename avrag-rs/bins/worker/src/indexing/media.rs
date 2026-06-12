use std::path::Path;

use avrag_llm::MultiModalEmbeddingInput;
use avrag_storage_pg::ObjectStoreHandle;

use super::types::StoredMultimodalChunk;

#[derive(Clone)]
pub struct MediaResolveContext {
    pub object_store: ObjectStoreHandle,
    pub asset_url_ttl_secs: u64,
}

fn is_remote_media_reference(path: &str) -> bool {
    common::is_remote_url(path)
}

async fn read_asset_bytes(ctx: &MediaResolveContext, path: &str) -> Result<Vec<u8>, String> {
    if let Some(local_path) = path.strip_prefix("temporary://") {
        return tokio::fs::read(local_path)
            .await
            .map_err(|error| format!("read temp asset: {error}"));
    }
    if let Some(local_path) = path.strip_prefix("file://") {
        return tokio::fs::read(local_path)
            .await
            .map_err(|error| format!("read file asset: {error}"));
    }
    let local_path = Path::new(path);
    if local_path.exists() {
        return tokio::fs::read(local_path)
            .await
            .map_err(|error| format!("read local asset: {error}"));
    }
    ctx.object_store
        .get(path)
        .await
        .map_err(|error| format!("read object store asset: {error}"))
}

pub async fn resolve_media_reference_for_remote_api(
    ctx: &MediaResolveContext,
    path: &str,
) -> Result<Option<String>, String> {
    let trimmed = path.trim();
    if trimmed.is_empty() {
        return Ok(None);
    }
    if is_remote_media_reference(trimmed) {
        return Ok(Some(trimmed.to_string()));
    }

    if ctx.object_store.is_remote() {
        if let Ok(presigned) = ctx
            .object_store
            .presigned_get_url(trimmed, ctx.asset_url_ttl_secs.max(300))
            .await
        {
            if is_remote_media_reference(&presigned) {
                return Ok(Some(presigned));
            }
        }
    }

    let bytes = read_asset_bytes(ctx, trimmed).await?;
    let mime = common::infer_image_extension(trimmed)
        .map(|ext| format!("image/{ext}"))
        .unwrap_or_else(|| "image/jpeg".to_string());
    use base64::Engine as _;
    let encoded = base64::engine::general_purpose::STANDARD.encode(bytes);
    Ok(Some(format!("data:{mime};base64,{encoded}")))
}

pub async fn build_multimodal_embed_input(
    ctx: &MediaResolveContext,
    chunk: &StoredMultimodalChunk,
    caption: String,
) -> MultiModalEmbeddingInput {
    let mut resolved_fusion = Vec::new();
    if chunk.fusion_image_paths.len() > 1 {
        for path in &chunk.fusion_image_paths {
            if let Ok(Some(resolved)) = resolve_media_reference_for_remote_api(ctx, path).await {
                resolved_fusion.push(resolved);
            }
        }
    }
    let resolved_single = resolve_media_reference_for_remote_api(ctx, &chunk.image_path)
        .await
        .ok()
        .flatten();
    if resolved_fusion.len() > 1 {
        MultiModalEmbeddingInput::text_images(caption, resolved_fusion)
    } else if let Some(single) = resolved_single {
        MultiModalEmbeddingInput::text_image(caption, single)
    } else {
        MultiModalEmbeddingInput::text(chunk.context_text.clone())
    }
}

pub async fn resolve_visual_chunk_image_refs(
    ctx: &MediaResolveContext,
    chunk: &StoredMultimodalChunk,
) -> Result<Vec<String>, String> {
    let paths = if chunk.fusion_image_paths.len() > 1 {
        chunk.fusion_image_paths.clone()
    } else {
        vec![chunk.image_path.clone()]
    };
    let mut refs = Vec::new();
    for path in paths {
        if let Some(resolved) = resolve_media_reference_for_remote_api(ctx, &path).await? {
            refs.push(resolved);
        }
    }
    Ok(refs)
}
