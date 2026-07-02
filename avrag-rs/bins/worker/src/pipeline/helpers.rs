use ingestion::parser::{ParsePlan, PdfPageBackend};
use ingestion::{DocumentIr, ParseBackend, SourceLocator};

pub(crate) use super::graph_index::{
    GraphIndexRecords, build_document_index_batch, build_graph_index_records,
};
pub(crate) use super::index_dispatch::build_text_index_records;
pub(crate) use super::parse_route::{
    execute_external_parse, execute_local_parse, execute_office_parse,
};
pub(crate) use super::pg_side_effects::{
    build_asset_object_key, build_document_block_rows, build_document_chunk_rows,
    collect_document_text, generate_document_profile_with_llm,
};
use anyhow::{Result, anyhow};
use avrag_auth::AuthContext;
use avrag_storage_pg::ObjectStoreHandle;
use std::path::{Component, Path};
use uuid::Uuid;

pub(crate) use super::triplet_extraction::{
    extract_triplets_for_index, extract_visual_triplets_for_index, merge_extracted_triplets,
    triplet_extraction_enabled, visual_triplet_extraction_enabled,
};
use crate::runtime_support::safe_relative_object_key;

#[cfg(test)]
pub(crate) use super::index_dispatch::embed_text_vectors_with_client;
#[cfg(test)]
pub(crate) use super::triplet_extraction::{ExtractedTriplet, parse_triplet_response};

pub(crate) fn estimate_token_count(text: &str) -> i64 {
    common::estimate_token_count(text)
}

pub(crate) fn validate_mirror_source_path(source_path: &str) -> Result<()> {
    if source_path.trim().is_empty() {
        return Ok(());
    }
    if common::is_remote_url(source_path) {
        return common::validate_http_url_with_dns(source_path, true)
            .map_err(|error| anyhow!("mirror remote asset blocked: {error}"));
    }
    if let Some(local_path) = source_path.strip_prefix("temporary://") {
        return validate_temporary_mirror_path(local_path);
    }
    if !safe_relative_object_key(source_path) {
        return Err(anyhow!("mirror local asset path rejected: {source_path}"));
    }
    Ok(())
}

fn validate_temporary_mirror_path(local_path: &str) -> Result<()> {
    let candidate = Path::new(local_path);
    let temp_dir = std::env::temp_dir()
        .canonicalize()
        .map_err(|error| anyhow!("worker temp dir unavailable: {error}"))?;
    if candidate.is_absolute() {
        let canonical = candidate
            .canonicalize()
            .map_err(|_| anyhow!("temporary mirror path not found or inaccessible"))?;
        if !canonical.starts_with(&temp_dir) {
            return Err(anyhow!("temporary mirror path outside worker temp dir"));
        }
        return Ok(());
    }

    let mut resolved = temp_dir.clone();
    for component in candidate.components() {
        match component {
            Component::CurDir => {}
            Component::Normal(segment) => resolved.push(segment),
            Component::ParentDir | Component::RootDir | Component::Prefix(_) => {
                return Err(anyhow!("temporary mirror path outside worker temp dir"));
            }
        }
    }
    if !resolved.starts_with(&temp_dir) {
        return Err(anyhow!("temporary mirror path outside worker temp dir"));
    }
    Ok(())
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
    validate_mirror_source_path(source_path)?;
    super::pg_side_effects::mirror_document_asset(
        object_store,
        context,
        notebook_id,
        document_id,
        asset_id,
        source_path,
        ttl_secs,
    )
    .await
}

pub(crate) fn enrich_multimodal_source_locator(
    source_locator: &SourceLocator,
    metadata: &std::collections::BTreeMap<String, String>,
) -> serde_json::Value {
    let mut value = serde_json::to_value(source_locator).unwrap_or_else(|_| serde_json::json!({}));
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
                            // Wire plan: `edge_parse` (= LiteParse text); metadata uses canonical backend.
                            PdfPageBackend::EdgeParse => ParseBackend::LiteParsePdf.as_str(),
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

    let ingest_routing = document_ir.map(|document| {
        let keys = [
            "pdf_route_mode",
            "paddle_jobs_count",
            "paddle_jobs_used",
            "ocr_backend",
            "paddle_jobs_budget_skipped",
            "paddle_cache_hits",
            "ingest_route_version",
        ];
        let mut routing = serde_json::Map::new();
        for key in keys {
            if let Some(value) = document.metadata.get(key) {
                routing.insert(key.to_string(), serde_json::Value::String(value.clone()));
            }
        }
        let paddle_warnings: Vec<_> = document
            .warnings
            .iter()
            .filter(|w| w.code.starts_with("paddle_job_"))
            .map(|w| {
                serde_json::json!({
                    "code": w.code,
                    "page": w.page,
                })
            })
            .collect();
        if !paddle_warnings.is_empty() {
            routing.insert(
                "paddle_warnings".to_string(),
                serde_json::Value::Array(paddle_warnings),
            );
        }
        serde_json::Value::Object(routing)
    });

    serde_json::json!({
        "route": &route_decision.route,
        "reason": &route_decision.reason,
        "plan": &route_decision.plan,
        "probe_result": &route_decision.probe_result,
        "page_backends": page_backends,
        "page_status": page_status,
        "ingest_routing": ingest_routing,
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

pub(crate) fn record_graph_degrade(outputs: &mut ParseRunOutputs, reason: String) {
    outputs.graph_degrade_count += 1;
    outputs.graph_degrade_reasons.push(reason);
}
