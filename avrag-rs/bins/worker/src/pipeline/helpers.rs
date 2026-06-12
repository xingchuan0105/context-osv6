use anyhow::Result;
use avrag_auth::{ActorId, AuthContext, OrgId, SubjectKind};
use avrag_cache_redis::DocumentLock;
use avrag_llm::{ChatMessage, SummaryGenerator};
use avrag_retrieval_data_plane::{
    DocumentIndexBatch, EntityIndexRecord, GraphPassageIndexRecord, MultimodalChunkIndexRecord,
    RelationIndexRecord, RetrievalDataPlane, TextChunkIndexRecord,
};
use crate::indexing::{
    build_multimodal_index_records, env_flag_enabled, maybe_enrich_visual_multimodal_summaries,
    record_multimodal_degrade, resolve_visual_chunk_image_refs, MediaResolveContext,
    StoredMultimodalChunk,
};
use avrag_storage_milvus::{MilvusConfig as StorageMilvusConfig, MilvusDataPlane};
use avrag_storage_pg::{
    DocumentCleanupTask, NotificationCreateParams, ObjectStoreHandle, PgAppRepository, S3ObjectStore,
};
use app_core::{load_prompt_template, AppConfig};
use ingestion::chunker::ChunkPolicy;
use ingestion::parser::{
    CodeParser, DocumentParser, ExternalParseKind, HtmlParser, LocalParseKind, MineruClient,
    MineruConfig, OfficeDocType, OfficeParserServiceClient, OfficeParserServiceConfig,
    ParsePlan, ParseRouter, PdfPageBackend, PdfParser, PdfRendererServiceClient,
    PdfRendererServiceConfig, TextParser, VisualPdfParser, normalize_parsed_document,
};
use ingestion::{
    AssetIr, BlockIr, BlockModality, BlockType, DocumentIr, DocumentIrValidationOptions,
    DocumentType, IngestionError, IngestionTask, PageIr, ParseBackend, SourceLocator,
    sanitize_and_validate_document_ir,
};
use sha2::{Digest, Sha256};
use std::convert::TryFrom;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::time::Duration;
use tracing::{info, warn};
use uuid::Uuid;

use crate::ingestion_guard::{
    ensure_ingestion_side_effects_allowed, spawn_ingestion_task_lock_heartbeat,
    stop_ingestion_task_lock_heartbeat, verify_uploaded_object_bytes, worker_task_kind,
};
use crate::pdf;

use super::document_pipeline::ParseRunState;
use super::processor::PgTaskProcessor;
pub(crate) fn estimate_token_count(text: &str) -> i64 {
    common::estimate_token_count(text)
}

pub(crate) fn enrich_multimodal_source_locator(
    source_locator: &SourceLocator,
    metadata: &std::collections::BTreeMap<String, String>,
) -> serde_json::Value {
    let mut value =
        serde_json::to_value(source_locator).unwrap_or_else(|_| serde_json::json!({}));
    let Some(obj) = value.as_object_mut() else {
        return value;
    };
    for key in [
        "page_range_start",
        "page_range_end",
        "ingest_route",
        "page_numbers",
    ] {
        if let Some(entry) = metadata.get(key) {
            obj.insert(key.to_string(), serde_json::Value::String(entry.clone()));
        }
    }
    value
}

pub(crate) async fn resolve_mineru_source_url(
    processor: &PgTaskProcessor,
    object_path: &str,
) -> Result<Option<String>, IngestionError> {
    if object_path.trim().is_empty() {
        return Ok(None);
    }
    if common::is_remote_url(object_path) {
        return Ok(Some(object_path.to_string()));
    }

    let presigned = processor
        .object_store
        .presigned_get_url(object_path, processor.asset_url_ttl_secs.max(300))
        .await
        .map_err(|error| IngestionError::StateSink(error.to_string()))?;
    Ok(Some(presigned))
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct ExtractedTriplet {
    pub(crate) subject: String,
    pub(crate) predicate: String,
    pub(crate) object: String,
    pub(crate) supporting_chunk_ids: Vec<Uuid>,
    pub(crate) source: String,
    pub(crate) confidence: f32,
}

#[derive(Debug, Clone)]
struct TripletExtractionBatch {
    chunk_ids: Vec<Uuid>,
    payload: serde_json::Value,
}

#[derive(Debug, Default)]
pub(crate) struct TripletExtractionOutput {
    pub(crate) triplets: Vec<ExtractedTriplet>,
    pub(crate) total_tokens: u32,
}

#[derive(Debug, Default)]
pub(crate) struct GraphIndexRecords {
    pub(crate) entities: Vec<EntityIndexRecord>,
    pub(crate) relations: Vec<RelationIndexRecord>,
    pub(crate) passages: Vec<GraphPassageIndexRecord>,
}

#[derive(Debug, Default, Clone)]
pub(crate) struct ParseRunOutputs {
    pub(crate) block_count: usize,
    pub(crate) asset_count: usize,
    pub(crate) text_chunk_count: usize,
    pub(crate) multimodal_chunk_count: usize,
    pub(crate) mirrored_asset_count: usize,
    pub(crate) text_vector_count: usize,
    pub(crate) multimodal_vector_count: usize,
    pub(crate) entity_count: usize,
    pub(crate) relation_count: usize,
    pub(crate) graph_passage_count: usize,
    pub(crate) graph_degrade_count: usize,
    pub(crate) graph_degrade_reasons: Vec<String>,
    pub(crate) multimodal_degrade_count: usize,
    pub(crate) multimodal_degrade_reasons: Vec<String>,
}


pub(crate) async fn execute_local_parse(
    bytes: &[u8],
    filename: &str,
    document_id: Uuid,
    kind: &LocalParseKind,
) -> Result<DocumentIr, IngestionError> {
    let (doc_type, backend, parser): (DocumentType, ParseBackend, Box<dyn DocumentParser>) =
        match kind {
            LocalParseKind::Text => (
                DocumentType::Text,
                ParseBackend::TextLocal,
                Box::new(TextParser),
            ),
            LocalParseKind::Html => (
                DocumentType::Html,
                ParseBackend::HtmlLocal,
                Box::new(HtmlParser),
            ),
            LocalParseKind::Code => (
                DocumentType::Code,
                ParseBackend::CodeLocal,
                Box::new(CodeParser),
            ),
        };

    let parsed = parser.parse(bytes, filename).await.map_err(|error| {
        IngestionError::StateSink(format!("local parse failed for {filename}: {error}"))
    })?;
    Ok(pdf::document_ir_from_parsed_document(
        document_id,
        filename,
        doc_type,
        backend,
        parsed,
    ))
}

pub(crate) async fn execute_external_parse(
    processor: &PgTaskProcessor,
    bytes: &[u8],
    filename: &str,
    object_path: &str,
    document_id: Uuid,
    kind: &ExternalParseKind,
) -> Result<DocumentIr, IngestionError> {
    let mineru = processor.mineru_client.as_ref().ok_or_else(|| {
        IngestionError::StateSink(format!(
            "external parse selected for {filename}, but MINERU is not configured"
        ))
    })?;
    let source_url = resolve_mineru_source_url(processor, object_path).await?;

    match kind {
        ExternalParseKind::MineruImage => {
            let normalized = mineru
                .parse(bytes, filename, source_url.as_deref())
                .await
                .map_err(|error| {
                    IngestionError::StateSink(format!(
                        "MinerU precise parse failed for {filename}: {error}"
                    ))
                })?;
            let doc_type = DocumentType::from_filename(filename);
            Ok(DocumentIr::from_normalized_document(
                document_id.to_string(),
                doc_type,
                ParseBackend::MineruImage,
                &normalized,
            ))
        }
    }
}

pub(crate) async fn execute_office_parse(
    processor: &PgTaskProcessor,
    bytes: &[u8],
    filename: &str,
    document_id: Uuid,
    doc_type: &OfficeDocType,
) -> Result<DocumentIr, IngestionError> {
    let client = processor.office_parser_client.as_ref().ok_or_else(|| {
        IngestionError::StateSink(format!(
            "office parse selected for {filename}, but OFFICE_PARSER_BASE_URL is not configured"
        ))
    })?;

    let response = match doc_type {
        OfficeDocType::Docx => {
            client
                .parse_docx(bytes, filename, &document_id.to_string())
                .await
        }
        OfficeDocType::Xlsx => {
            client
                .parse_xlsx(bytes, filename, &document_id.to_string())
                .await
        }
        OfficeDocType::Ppt => {
            client
                .parse_ppt(bytes, filename, &document_id.to_string())
                .await
        }
        OfficeDocType::Pptx => {
            client
                .parse_pptx(bytes, filename, &document_id.to_string())
                .await
        }
        OfficeDocType::Doc => {
            client
                .parse_doc(bytes, filename, &document_id.to_string())
                .await
        }
        OfficeDocType::Xls => {
            client
                .parse_xls(bytes, filename, &document_id.to_string())
                .await
        }
    }
    .map_err(|error| {
        IngestionError::StateSink(format!("office parse failed for {filename}: {error}"))
    })?;

    let mut document_ir = response.document_ir;
    document_ir.document_id = document_id.to_string();
    if document_ir.title.trim().is_empty() {
        document_ir.title = filename.to_string();
    }
    document_ir.warnings.extend(response.warnings);
    Ok(document_ir)
}


pub(crate) fn build_parse_backend_summary(
    route_decision: &ingestion::parser::ParseRouteDecision,
    document_ir: Option<&DocumentIr>,
    outputs: &ParseRunOutputs,
) -> serde_json::Value {
    let page_backends = document_ir
        .map(|document| {
            document
                .pages
                .iter()
                .map(|page| {
                    serde_json::json!({
                        "page": page.page_number,
                        "backend": page.backend.as_str(),
                    })
                })
                .collect::<Vec<_>>()
        })
        .unwrap_or_else(|| match &route_decision.plan {
            ParsePlan::Pdf(plan) => plan
                .pages
                .iter()
                .map(|page| {
                    serde_json::json!({
                        "page": page.page_number,
                        "backend": match page.backend {
                            PdfPageBackend::EdgeParse => ParseBackend::EdgeParsePdf.as_str(),
                            PdfPageBackend::PaddleOcr => ParseBackend::PaddleOcrPdf.as_str(),
                            PdfPageBackend::VisualRaster => ParseBackend::VisualRasterPdf.as_str(),
                        },
                    })
                })
                .collect::<Vec<_>>(),
            _ => Vec::new(),
        });

    let page_status: Option<serde_json::Value> = document_ir
        .and_then(|document| document.metadata.get("page_status"))
        .and_then(|raw| serde_json::from_str::<serde_json::Value>(raw).ok());

    serde_json::json!({
        "route": &route_decision.route,
        "reason": &route_decision.reason,
        "plan": &route_decision.plan,
        "probe_result": &route_decision.probe_result,
        "page_backends": page_backends,
        "page_status": page_status,
        "outputs": {
            "primary_backend": document_ir.map(|document| document.primary_backend.as_str()),
            "block_count": outputs.block_count,
            "asset_count": outputs.asset_count,
            "text_chunk_count": outputs.text_chunk_count,
            "multimodal_chunk_count": outputs.multimodal_chunk_count,
            "mirrored_asset_count": outputs.mirrored_asset_count,
            "text_vector_count": outputs.text_vector_count,
            "multimodal_vector_count": outputs.multimodal_vector_count,
            "entity_count": outputs.entity_count,
            "relation_count": outputs.relation_count,
            "graph_passage_count": outputs.graph_passage_count,
            "graph_degrade_count": outputs.graph_degrade_count,
            "graph_degrade_reasons": outputs.graph_degrade_reasons,
            "multimodal_degrade_count": outputs.multimodal_degrade_count,
            "multimodal_degrade_reasons": outputs.multimodal_degrade_reasons,
        },
    })
}

pub(crate) fn build_parse_warning_payload(
    document_ir: Option<&DocumentIr>,
    validation_warnings: &[ingestion::DocumentIrValidationIssue],
) -> serde_json::Value {
    let parse_warnings = document_ir
        .map(|document| {
            document
                .warnings
                .iter()
                .map(|warning| {
                    serde_json::json!({
                        "code": warning.code,
                        "message": warning.message,
                        "page": warning.page,
                        "backend": warning.backend.as_str(),
                    })
                })
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    let validation_warnings = validation_warnings
        .iter()
        .map(|warning| {
            serde_json::json!({
                "code": warning.code,
                "message": warning.message,
                "block_id": warning.block_id,
                "asset_id": warning.asset_id,
                "page": warning.page,
            })
        })
        .collect::<Vec<_>>();
    serde_json::json!({
        "parse_warnings": parse_warnings,
        "validation_warnings": validation_warnings,
    })
}

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

pub(crate) async fn build_text_index_records(
    processor: &PgTaskProcessor,
    chunks: &[avrag_storage_pg::IndexedChunk],
) -> Result<Vec<TextChunkIndexRecord>, IngestionError> {
    let texts = chunks
        .iter()
        .map(|chunk| chunk.content.as_str())
        .collect::<Vec<_>>();
    let vectors = embed_text_vectors(processor, &texts).await?;

    chunks
        .iter()
        .zip(vectors)
        .map(|(chunk, vector)| {
            let chunk_id = Uuid::parse_str(&chunk.chunk_id)
                .map_err(|error| IngestionError::StateSink(format!("invalid chunk id: {error}")))?;
            Ok(TextChunkIndexRecord {
                chunk_id,
                content: chunk.content.clone(),
                vector,
                page: chunk.page,
                chunk_type: metadata_string(&chunk.metadata, "block_type")
                    .or_else(|| metadata_string(&chunk.metadata, "kind"))
                    .unwrap_or_else(|| "body".to_string()),
                parser_backend: metadata_string(&chunk.metadata, "parser_backend"),
                source_locator: metadata_value(&chunk.metadata, "source_locator"),
            })
        })
        .collect()
}

pub(crate) async fn embed_text_vectors(
    processor: &PgTaskProcessor,
    texts: &[&str],
) -> Result<Vec<Vec<f32>>, IngestionError> {
    if texts.is_empty() {
        return Ok(Vec::new());
    }
    if processor.embedding_client.is_none() {
        return Err(IngestionError::StateSink(format!(
            "text embedding client not configured (expected dim {})",
            processor.embedding_dim
        )));
    }
    embed_text_vectors_with_client(processor.embedding_client.as_ref(), texts).await
}

pub(crate) async fn embed_text_vectors_with_client(
    client: Option<&avrag_llm::EmbeddingClient>,
    texts: &[&str],
) -> Result<Vec<Vec<f32>>, IngestionError> {
    if texts.is_empty() {
        return Ok(Vec::new());
    }
    let Some(client) = client else {
        return Err(IngestionError::StateSink(
            "text embedding client not configured".to_string(),
        ));
    };
    client
        .embed(texts)
        .await
        .map_err(|error| IngestionError::StateSink(format!("embedding failed: {error}")))
}

pub(crate) fn merge_extracted_triplets(
    mut base: Vec<ExtractedTriplet>,
    extra: Vec<ExtractedTriplet>,
) -> Vec<ExtractedTriplet> {
    let mut triplet_map: std::collections::HashMap<(String, String, String), ExtractedTriplet> =
        std::collections::HashMap::new();
    for triplet in base.drain(..).chain(extra) {
        let key = (
            triplet.subject.to_lowercase(),
            triplet.predicate.to_lowercase(),
            triplet.object.to_lowercase(),
        );
        if let Some(existing) = triplet_map.get_mut(&key) {
            for chunk_id in triplet.supporting_chunk_ids {
                if !existing.supporting_chunk_ids.contains(&chunk_id) {
                    existing.supporting_chunk_ids.push(chunk_id);
                }
            }
            if triplet.confidence > existing.confidence {
                existing.confidence = triplet.confidence;
            }
        } else {
            triplet_map.insert(key, triplet);
        }
    }
    triplet_map.into_values().collect()
}

pub(crate) async fn extract_visual_triplets_for_index(
    processor: &PgTaskProcessor,
    document_id: Uuid,
    multimodal_chunks: &[StoredMultimodalChunk],
    parse_run_state: &mut ParseRunState,
) -> TripletExtractionOutput {
    let Some(llm) = processor.triplet_llm.clone() else {
        return TripletExtractionOutput::default();
    };

    let visual_chunks: Vec<&StoredMultimodalChunk> = multimodal_chunks
        .iter()
        .filter(|chunk| chunk.chunk_type == "page_raster")
        .collect();
    if visual_chunks.is_empty() {
        return TripletExtractionOutput::default();
    }

    let media_ctx = MediaResolveContext {
        object_store: processor.object_store.clone(),
        asset_url_ttl_secs: processor.asset_url_ttl_secs,
    };
    let mut output = TripletExtractionOutput::default();
    for chunk in visual_chunks {
        let image_refs = match resolve_visual_chunk_image_refs(&media_ctx, chunk).await {
            Ok(refs) => refs,
            Err(error) => {
                record_graph_degrade(
                    &mut parse_run_state.outputs,
                    format!(
                        "chunk {}: visual triplet skipped (image resolve failed): {error}",
                        chunk.chunk_id
                    ),
                );
                continue;
            }
        };
        if image_refs.is_empty() {
            record_graph_degrade(
                &mut parse_run_state.outputs,
                format!(
                    "chunk {}: visual triplet skipped (no resolvable page images)",
                    chunk.chunk_id
                ),
            );
            continue;
        }
        let caption = chunk
            .caption
            .clone()
            .unwrap_or_else(|| "PDF page raster".to_string());
        let image_list = image_refs.join(", ");
        let prompt = format!(
            "Extract up to 3 factual subject-predicate-object triplets from this page image. \
             Return JSON: {{\"triplets\":[{{\"chunk_id\":\"{}\",\"subject\":\"...\",\"predicate\":\"...\",\"object\":\"...\",\"confidence\":0.0-1.0,\"source\":\"vlm_page_summary\"}}]}}. \
             Caption: {caption}. Image URL(s): {image_list}",
            chunk.chunk_id
        );
        let messages = vec![
            ChatMessage::system(
                "Extract knowledge-graph triplets from document page images. JSON only.",
            ),
            ChatMessage::user(prompt),
        ];
        match llm.complete(&messages, Some(0.1)).await {
            Ok(response) => {
                output.total_tokens = output
                    .total_tokens
                    .saturating_add(response.usage.total_tokens);
                match parse_triplet_response(&response.content, &[chunk.chunk_id]) {
                    Ok(mut triplets) => {
                        for triplet in &mut triplets {
                            if triplet.source.is_empty() {
                                triplet.source = "vlm_page_summary".to_string();
                            }
                        }
                        output.triplets.extend(triplets);
                    }
                    Err(error) => {
                        let reason = format!("visual triplet extraction failed: {error}");
                        record_graph_degrade(&mut parse_run_state.outputs, reason.clone());
                        info!(document_id = %document_id, error = %reason, "visual triplet extraction degraded");
                    }
                }
            }
            Err(error) => {
                let reason = format!("visual triplet extraction failed: {error}");
                record_graph_degrade(&mut parse_run_state.outputs, reason.clone());
                info!(document_id = %document_id, error = %reason, "visual triplet extraction degraded");
            }
        }
    }
    output
}

pub(crate) async fn extract_triplets_for_index(
    processor: &PgTaskProcessor,
    document_id: Uuid,
    text_chunks: &[TextChunkIndexRecord],
    parse_run_state: &mut ParseRunState,
) -> TripletExtractionOutput {
    let Some(llm) = processor.triplet_llm.clone() else {
        return TripletExtractionOutput::default();
    };

    let batches = build_triplet_extraction_batches(text_chunks);
    if batches.is_empty() {
        return TripletExtractionOutput::default();
    }

    let semaphore = Arc::new(tokio::sync::Semaphore::new(4));
    let mut handles = Vec::with_capacity(batches.len());
    for batch in batches {
        let llm = llm.clone();
        let sem = semaphore.clone();
        handles.push(tokio::spawn(async move {
            let _permit = sem
                .acquire_owned()
                .await
                .map_err(|e| anyhow::anyhow!("{e}"))?;
            let messages = build_triplet_extraction_messages(&batch);
            let response = llm.complete(&messages, Some(0.1)).await?;
            let raw_triplets = parse_triplet_response(&response.content, &batch.chunk_ids)?;
            Ok::<_, anyhow::Error>((raw_triplets, response.usage.total_tokens))
        }));
    }

    let mut output = TripletExtractionOutput::default();
    let mut triplet_map: std::collections::HashMap<(String, String, String), ExtractedTriplet> =
        std::collections::HashMap::new();
    for handle in handles {
        match handle.await {
            Ok(Ok((triplets, total_tokens))) => {
                output.total_tokens = output.total_tokens.saturating_add(total_tokens);
                for triplet in triplets {
                    let key = (
                        triplet.subject.to_lowercase(),
                        triplet.predicate.to_lowercase(),
                        triplet.object.to_lowercase(),
                    );
                    if let Some(existing) = triplet_map.get_mut(&key) {
                        for cid in triplet.supporting_chunk_ids {
                            if !existing.supporting_chunk_ids.contains(&cid) {
                                existing.supporting_chunk_ids.push(cid);
                            }
                        }
                    } else {
                        triplet_map.insert(key, triplet);
                    }
                }
            }
            Ok(Err(error)) => {
                let reason = format!("triplet extraction failed: {error}");
                record_graph_degrade(&mut parse_run_state.outputs, reason.clone());
                info!(document_id = %document_id, error = %reason, "triplet extraction degraded");
            }
            Err(error) => {
                let reason = format!("triplet extraction task join failed: {error}");
                record_graph_degrade(&mut parse_run_state.outputs, reason.clone());
                info!(document_id = %document_id, error = %reason, "triplet extraction degraded");
            }
        }
    }
    output.triplets = triplet_map.into_values().collect();

    output
}

fn build_triplet_extraction_batches(
    text_chunks: &[TextChunkIndexRecord],
) -> Vec<TripletExtractionBatch> {
    const TOKEN_BUDGET: i64 = 3_000;

    let mut batches = Vec::new();
    let mut current_ids = Vec::new();
    let mut current_chunks = Vec::new();
    let mut current_tokens = 0i64;

    for chunk in text_chunks {
        let chunk_tokens = estimate_token_count(&chunk.content).max(1);
        if !current_chunks.is_empty() && current_tokens + chunk_tokens > TOKEN_BUDGET {
            batches.push(TripletExtractionBatch {
                chunk_ids: std::mem::take(&mut current_ids),
                payload: serde_json::json!({ "chunks": std::mem::take(&mut current_chunks) }),
            });
            current_tokens = 0;
        }

        current_ids.push(chunk.chunk_id);
        current_chunks.push(serde_json::json!({
            "chunk_id": chunk.chunk_id.to_string(),
            "text": &chunk.content,
        }));
        current_tokens += chunk_tokens;
    }

    if !current_chunks.is_empty() {
        batches.push(TripletExtractionBatch {
            chunk_ids: current_ids,
            payload: serde_json::json!({ "chunks": current_chunks }),
        });
    }

    batches
}

const TRIPLET_EXTRACTION_SYSTEM_PROMPT: &str =
    include_str!("../../../../prompts/pipeline/triplet-extraction.system.md");

fn build_triplet_extraction_messages(batch: &TripletExtractionBatch) -> Vec<ChatMessage> {
    let valid_chunk_ids: Vec<String> = batch.chunk_ids.iter().map(|id| id.to_string()).collect();
    vec![
        ChatMessage::system(TRIPLET_EXTRACTION_SYSTEM_PROMPT),
        ChatMessage::user(format!(
            "Valid chunk IDs: {}\n\nChunks:\n{}\n\nExtract triplets with chunk_id:",
            valid_chunk_ids.join(", "),
            batch.payload
        )),
    ]
}

pub(crate) fn parse_triplet_response(
    content: &str,
    valid_chunk_ids: &[Uuid],
) -> Result<Vec<ExtractedTriplet>> {
    let value: serde_json::Value = serde_json::from_str(content)
        .map_err(|e| anyhow::anyhow!("Failed to parse triplet JSON: {}", e))?;

    let triplets = value
        .get("triplets")
        .and_then(serde_json::Value::as_array)
        .ok_or_else(|| anyhow::anyhow!("triplet response missing triplets array"))?;

    let mut parsed = Vec::new();
    for item in triplets {
        // 严格对象格式：{"chunk_id": "...", "subject": "...", "predicate": "...", "object": "..."}
        let Some(chunk_id_str) = item.get("chunk_id").and_then(|v| v.as_str()) else {
            continue; // chunk_id 缺失，丢弃
        };
        let Ok(chunk_id) = Uuid::parse_str(chunk_id_str) else {
            continue; // chunk_id 无法解析，丢弃
        };
        if !valid_chunk_ids.contains(&chunk_id) {
            continue; // chunk_id 不在当前 batch 内，丢弃
        }

        let Some(subject) = item
            .get("subject")
            .and_then(|v| v.as_str())
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
        else {
            continue;
        };
        let Some(predicate) = item
            .get("predicate")
            .and_then(|v| v.as_str())
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
        else {
            continue;
        };
        let Some(object) = item
            .get("object")
            .and_then(|v| v.as_str())
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
        else {
            continue;
        };

        let confidence = item
            .get("confidence")
            .and_then(|v| v.as_f64())
            .map(|v| v as f32)
            .unwrap_or(1.0);
        let source = item
            .get("source")
            .and_then(|v| v.as_str())
            .unwrap_or("text_chunk")
            .to_string();

        parsed.push(ExtractedTriplet {
            subject,
            predicate,
            object,
            supporting_chunk_ids: vec![chunk_id],
            source,
            confidence,
        });
    }
    Ok(parsed)
}

const VISUAL_TRIPLET_MIN_CONFIDENCE: f32 = 0.6;

pub(crate) fn triplet_extraction_enabled() -> bool {
    env_flag_enabled("INGESTION_TRIPLET_ENABLED", true)
}

pub(crate) fn visual_triplet_extraction_enabled() -> bool {
    env_flag_enabled("INGESTION_VLM_TRIPLET_ENABLED", false)
}

fn triplet_confidence_threshold() -> f32 {
    std::env::var("INGESTION_TRIPLET_MIN_CONFIDENCE")
        .ok()
        .and_then(|value| value.parse().ok())
        .unwrap_or(VISUAL_TRIPLET_MIN_CONFIDENCE)
}

pub(crate) async fn build_graph_index_records(
    processor: &PgTaskProcessor,
    triplets: Vec<ExtractedTriplet>,
    parse_run_state: &mut ParseRunState,
) -> GraphIndexRecords {
    let min_confidence = triplet_confidence_threshold();
    let triplets: Vec<ExtractedTriplet> = triplets
        .into_iter()
        .filter(|triplet| triplet.confidence >= min_confidence)
        .collect();

    if triplets.is_empty() {
        return GraphIndexRecords::default();
    }

    let mut entity_map: std::collections::BTreeMap<String, (String, Vec<Uuid>)> =
        std::collections::BTreeMap::new();
    for triplet in &triplets {
        for name in [&triplet.subject, &triplet.object] {
            let normalized = name.to_lowercase();
            let entry = entity_map
                .entry(normalized)
                .or_insert_with(|| (name.clone(), Vec::new()));
            for chunk_id in &triplet.supporting_chunk_ids {
                if !entry.1.contains(chunk_id) {
                    entry.1.push(*chunk_id);
                }
            }
        }
    }

    let entity_entries = entity_map.into_iter().collect::<Vec<_>>();
    let entity_texts = entity_entries
        .iter()
        .map(|(_, (name, _))| name.as_str())
        .collect::<Vec<_>>();
    let entity_vectors = match embed_text_vectors(processor, &entity_texts).await {
        Ok(vectors) => vectors,
        Err(error) => {
            record_graph_degrade(
                &mut parse_run_state.outputs,
                format!("graph entity embedding failed: {error}"),
            );
            return GraphIndexRecords::default();
        }
    };

    let relation_texts = triplets
        .iter()
        .map(|triplet| {
            format!(
                "{} {} {}",
                triplet.subject, triplet.predicate, triplet.object
            )
        })
        .collect::<Vec<_>>();
    let relation_text_refs = relation_texts
        .iter()
        .map(String::as_str)
        .collect::<Vec<_>>();
    let relation_vectors = match embed_text_vectors(processor, &relation_text_refs).await {
        Ok(vectors) => vectors,
        Err(error) => {
            record_graph_degrade(
                &mut parse_run_state.outputs,
                format!("graph relation embedding failed: {error}"),
            );
            return GraphIndexRecords::default();
        }
    };

    let entities = entity_entries
        .into_iter()
        .zip(entity_vectors)
        .map(
            |((normalized_name, (name, supporting_chunk_ids)), vector)| EntityIndexRecord {
                entity_id: Uuid::new_v4(),
                name,
                normalized_name,
                entity_type: None,
                vector,
                supporting_chunk_ids,
                metadata: Some(serde_json::json!({ "source": "worker_triplet_extraction" })),
            },
        )
        .collect::<Vec<_>>();

    let mut relations = Vec::with_capacity(triplets.len());
    let mut passages = Vec::with_capacity(triplets.len());
    for ((triplet, relation_text), vector) in triplets
        .into_iter()
        .zip(relation_texts)
        .zip(relation_vectors)
    {
        let relation_id = Uuid::new_v4();
        relations.push(RelationIndexRecord {
            relation_id,
            subject: triplet.subject.clone(),
            predicate: triplet.predicate.clone(),
            object: triplet.object.clone(),
            relation_text: relation_text.clone(),
            vector: vector.clone(),
            supporting_chunk_ids: triplet.supporting_chunk_ids.clone(),
            metadata: Some(serde_json::json!({ "source": "worker_triplet_extraction" })),
        });
        // GraphPassageIndexRecord.chunk_id 只能来自合并后的真实 supporting chunk；
        // 如果没有 supporting chunk，不写该 graph passage。
        if let Some(chunk_id) = triplet.supporting_chunk_ids.first().copied() {
            passages.push(GraphPassageIndexRecord {
                passage_id: Uuid::new_v4(),
                chunk_id: Some(chunk_id),
                text: relation_text,
                vector,
                relation_ids: vec![relation_id],
                metadata: Some(serde_json::json!({ "source": "worker_triplet_extraction" })),
            });
        }
    }

    GraphIndexRecords {
        entities,
        relations,
        passages,
    }
}

pub(crate) fn build_document_index_batch(
    context: &AuthContext,
    workspace_id: Option<Uuid>,
    document_id: Uuid,
    parse_run_id: Uuid,
    text_chunks: Vec<TextChunkIndexRecord>,
    multimodal_chunks: Vec<MultimodalChunkIndexRecord>,
    graph_records: GraphIndexRecords,
) -> DocumentIndexBatch {
    DocumentIndexBatch {
        org_id: context.org_id(),
        workspace_id,
        document_id,
        parse_run_id,
        doc_version: 1,
        text_chunks,
        multimodal_chunks,
        entities: graph_records.entities,
        relations: graph_records.relations,
        graph_passages: graph_records.passages,
    }
}

fn metadata_string(metadata: &serde_json::Value, key: &str) -> Option<String> {
    metadata
        .get(key)
        .and_then(serde_json::Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
}

fn metadata_value(metadata: &serde_json::Value, key: &str) -> Option<serde_json::Value> {
    metadata.get(key).cloned().filter(|value| !value.is_null())
}

pub(crate) fn record_graph_degrade(outputs: &mut ParseRunOutputs, reason: String) {
    outputs.graph_degrade_count += 1;
    outputs.graph_degrade_reasons.push(reason);
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

// load_prompt_template moved to avrag_app::lib_impl::prompt_loader

// load_prompt_template moved to avrag_app::lib_impl::prompt_loader

pub(crate) async fn maybe_enrich_toc_with_llm(
    processor: &PgTaskProcessor,
    document_ir: &ingestion::DocumentIr,
    chunks: &[avrag_storage_pg::IndexedChunk],
    filename: &str,
    toc_entries: Vec<avrag_storage_pg::TocEntry>,
) -> Vec<avrag_storage_pg::TocEntry> {
    let heading_blocks = document_ir
        .blocks
        .iter()
        .filter(|b| matches!(b.block_type, ingestion::BlockType::Heading))
        .count();
    let force_llm = std::env::var("INGESTION_LLM_SECTION_INDEX")
        .ok()
        .is_some_and(|v| v == "1" || v.eq_ignore_ascii_case("true"));
    let sparse = toc_entries.is_empty() || heading_blocks == 0;
    if !force_llm && !sparse {
        return toc_entries;
    }
    let Some(generator) = processor.section_index_generator.as_ref() else {
        return toc_entries;
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
        return toc_entries;
    }
    match generator
        .generate(&document_ir.title, filename, &index_chunks)
        .await
    {
        Ok(output) if !output.sections.is_empty() => {
            info!(
                sections = output.sections.len(),
                "LLM section index generated for document"
            );
            toc_entries_from_llm_sections(&output)
        }
        Ok(_) => toc_entries,
        Err(error) => {
            info!(error = %error, "LLM section index failed; keeping heuristic toc");
            toc_entries
        }
    }
}

fn toc_entries_from_llm_sections(
    output: &avrag_llm::SectionIndexOutput,
) -> Vec<avrag_storage_pg::TocEntry> {
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
            entries.push(avrag_storage_pg::TocEntry {
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

pub(crate) fn build_toc_entries(
    document_ir: &ingestion::DocumentIr,
    chunks: &[avrag_storage_pg::IndexedChunk],
) -> Vec<avrag_storage_pg::TocEntry> {
    let mut block_id_to_chunk_id = std::collections::HashMap::new();
    for chunk in chunks {
        if let Ok(chunk_uuid) = Uuid::parse_str(&chunk.chunk_id)
            && let Some(block_id) = chunk.metadata.get("block_id").and_then(|v| v.as_str())
        {
            block_id_to_chunk_id.insert(block_id.to_string(), chunk_uuid);
        }
    }

    let mut entries = Vec::new();
    let mut heading_stack: Vec<(usize, Uuid)> = Vec::new();

    for (rank, block) in document_ir
        .blocks
        .iter()
        .filter(|b| matches!(b.block_type, ingestion::BlockType::Heading))
        .enumerate()
    {
        let heading_level = block
            .metadata
            .get("heading_level")
            .and_then(|v| v.parse::<i32>().ok())
            .unwrap_or(1);

        let title = if block.text.trim().is_empty() {
            document_ir.title.clone()
        } else {
            block.text.trim().to_string()
        };

        let page = block.page.map(|p| p as i32);
        let chunk_id = block_id_to_chunk_id.get(&block.block_id).copied();
        let entry_id = Uuid::new_v4();

        let parent_id = {
            while let Some(&(top_level, _)) = heading_stack.last() {
                if top_level < heading_level as usize {
                    break;
                }
                heading_stack.pop();
            }
            heading_stack.last().map(|&(_, id)| id)
        };

        entries.push(avrag_storage_pg::TocEntry {
            id: entry_id,
            parent_id,
            title,
            heading_level,
            page,
            chunk_id,
            rank: rank as i32,
        });

        heading_stack.push((heading_level as usize, entry_id));
    }

    entries
}
