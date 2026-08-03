use anyhow::Result;
use avrag_storage_pg::{ObjectStoreHandle, TocEntry};
use common::SummaryMetadata;
use contracts::auth_runtime::AuthContext;
use ingestion::DocumentIr;
use std::path::Path;
use tracing::{info, warn};
use uuid::Uuid;

use super::document_pipeline::ParseRunState;
use super::ingestion_session::{DocumentIngestionSession, INTERACTION_SESSION_SYSTEM};
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
    parse_run_state: &mut ParseRunState,
) -> DocumentProfileLlmResult {
    let Some(llm) = processor.llm.ingestion_llm.clone() else {
        info!(document_id = %document_id, "ingestion llm not configured; skipping profile");
        return DocumentProfileLlmResult {
            toc_entries: Vec::new(),
            profile_metadata: None,
        };
    };

    // 会话 seed 使用全文 chunks（不截断）——整个会话链（summary/profile/triplet）
    // 依赖此轮把文档全文载入上下文并命中 provider 侧会话缓存。若用 1200B preview
    // 截断，后续 summary 轮将看不到正文（"文档已在上下文"为假），质量回退。
    // chunk_id 由存储层生成合法 UUID，故 index_chunks 实际恒非空；此处仍防御——
    // 即使解析失败也建会话 seed 原文，避免 summary 被 profile 的解析成功连带跳过。
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
        // 防御：无合法 chunk_id 时仍建会话并 seed 文档全文（summary 依赖会话正文），
        // profile 返回空。materialize 已拒绝零 chunk，此路径理论不可达。
        let session = parse_run_state
            .session
            .get_or_insert_with(|| DocumentIngestionSession::new(llm));
        let fallback_text = chunks
            .iter()
            .map(|c| c.content.as_str())
            .collect::<Vec<_>>()
            .join("\n\n");
        let fallback = avrag_llm::build_session_seed_user_message(
            &document_ir.title,
            filename,
            &[avrag_llm::SectionIndexChunk {
                chunk_id: Uuid::new_v4(),
                text: fallback_text,
            }],
        );
        if let Ok(message) = fallback {
            let _ = session
                .seed(
                    &[INTERACTION_SESSION_SYSTEM, avrag_llm::section_index_system_prompt()],
                    &message,
                    Some(super::helpers::PROFILE_SEED_TEMPERATURE),
                )
                .await;
        }
        return DocumentProfileLlmResult {
            toc_entries: Vec::new(),
            profile_metadata: None,
        };
    }

    let user_message = match avrag_llm::build_session_seed_user_message(
        &document_ir.title,
        filename,
        &index_chunks,
    ) {
        Ok(message) => message,
        Err(error) => {
            info!(error = %error, "failed to build session seed user message");
            return DocumentProfileLlmResult {
                toc_entries: Vec::new(),
                profile_metadata: None,
            };
        }
    };

    let session = parse_run_state
        .session
        .get_or_insert_with(|| DocumentIngestionSession::new(llm));
    let chunk_ids: Vec<String> = index_chunks
        .iter()
        .map(|chunk| chunk.chunk_id.to_string())
        .collect();
    let turn = match session
        .seed(
            &[INTERACTION_SESSION_SYSTEM, avrag_llm::section_index_system_prompt()],
            &user_message,
            Some(super::helpers::PROFILE_SEED_TEMPERATURE),
        )
        .await
    {
        Ok(turn) => turn,
        Err(error) => {
            info!(error = %error, "LLM document profile index failed");
            return DocumentProfileLlmResult {
                toc_entries: Vec::new(),
                profile_metadata: None,
            };
        }
    };

    match avrag_llm::parse_section_index_response(&turn.content, &chunk_ids) {
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
            info!(error = %error, "LLM document profile parse failed");
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
