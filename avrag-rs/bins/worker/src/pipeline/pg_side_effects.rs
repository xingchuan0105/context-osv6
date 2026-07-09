use anyhow::Result;
use contracts::auth_runtime::AuthContext;
use avrag_storage_pg::{ObjectStoreHandle, TocEntry};
use common::SummaryMetadata;
use ingestion::DocumentIr;
use std::path::Path;
use tracing::{info, warn};
use uuid::Uuid;

use super::processor::PgTaskProcessor;

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
    notebook_id: &str,
    document_id: &str,
    asset_id: Uuid,
    source_path: &str,
) -> String {
    let extension = infer_asset_extension(source_path).unwrap_or("bin");
    format!(
        "{}/{}/{}/assets/{}.{}",
        context.org_id(),
        notebook_id,
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
    notebook_id: &str,
    document_id: &str,
    asset_id: Uuid,
    source_path: &str,
    ttl_secs: u64,
) -> Result<Option<String>> {
    if source_path.trim().is_empty() {
        return Ok(None);
    }

    let object_key =
        build_asset_object_key(context, notebook_id, document_id, asset_id, source_path);
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

pub(crate) struct DocumentProfileLlmResult {
    pub toc_entries: Vec<TocEntry>,
    pub profile_metadata: Option<SummaryMetadata>,
}

pub(crate) async fn generate_document_profile_with_llm(
    processor: &PgTaskProcessor,
    document_id: Uuid,
    document_ir: &DocumentIr,
    chunks: &[avrag_storage_pg::IndexedChunk],
    filename: &str,
) -> DocumentProfileLlmResult {
    let Some(generator) = processor.section_index_generator.as_ref() else {
        info!(document_id = %document_id, "section index generator not configured; skipping profile");
        return DocumentProfileLlmResult {
            toc_entries: Vec::new(),
            profile_metadata: None,
        };
    };

    let index_chunks: Vec<avrag_llm::SectionIndexChunk> = chunks
        .iter()
        .filter_map(|c| {
            Uuid::parse_str(&c.chunk_id)
                .ok()
                .map(|chunk_id| avrag_llm::SectionIndexChunk {
                    chunk_id,
                    text: c.content.clone(),
                })
        })
        .collect();
    if index_chunks.is_empty() {
        return DocumentProfileLlmResult {
            toc_entries: Vec::new(),
            profile_metadata: None,
        };
    }

    match generator
        .generate(&document_ir.title, filename, &index_chunks)
        .await
    {
        Ok(output) if !output.sections.is_empty() => {
            info!(
                sections = output.sections.len(),
                "LLM document profile index generated"
            );
            let profile_metadata = Some(avrag_llm::build_profile_metadata(
                &document_id.to_string(),
                &document_ir.title,
                filename,
                &output.document_metadata,
            ));
            DocumentProfileLlmResult {
                toc_entries: toc_entries_from_llm_sections(&output),
                profile_metadata,
            }
        }
        Ok(_) => DocumentProfileLlmResult {
            toc_entries: Vec::new(),
            profile_metadata: None,
        },
        Err(error) => {
            info!(error = %error, "LLM document profile index failed");
            DocumentProfileLlmResult {
                toc_entries: Vec::new(),
                profile_metadata: None,
            }
        }
    }
}

fn toc_entries_from_llm_sections(output: &avrag_llm::SectionIndexOutput) -> Vec<TocEntry> {
    let mut entries = Vec::new();
    let mut heading_stack: Vec<(i32, Uuid)> = Vec::new();

    for section in &output.sections {
        let heading_level = section.heading_level.clamp(1, 6);
        let entry_id = Uuid::new_v4();
        let parent_id = {
            while let Some(&(top_level, _)) = heading_stack.last() {
                if top_level < heading_level {
                    break;
                }
                heading_stack.pop();
            }
            heading_stack.last().map(|&(_, id)| id)
        };

        for chunk_id_str in &section.chunk_ids {
            let Ok(chunk_id) = Uuid::parse_str(chunk_id_str) else {
                continue;
            };
            entries.push(TocEntry {
                id: Uuid::new_v4(),
                parent_id,
                title: section.title.clone(),
                heading_level,
                page: section.page,
                chunk_id: Some(chunk_id),
                rank: section.rank,
            });
        }

        heading_stack.push((heading_level, entry_id));
    }

    entries
}
