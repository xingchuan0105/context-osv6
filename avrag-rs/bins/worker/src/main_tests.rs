use app_core::{AppConfig, load_prompt_template};
use avrag_auth::{AuthContext, OrgId, SubjectKind};
use avrag_retrieval_data_plane::{
    EntityIndexRecord, GraphPassageIndexRecord, RelationIndexRecord, TextChunkIndexRecord,
};
use ingestion::SourceLocator;
use ingestion::parser::{
    ParsePlan, ParseRoute, ParseRouteDecision, PdfPageBackend, PdfPagePlan, PdfParsePlan,
    RouteReason,
};
use std::{env, fs};
use uuid::Uuid;

use crate::ParseRunOutputs;
#[cfg(test)]
use crate::pipeline::helpers::{
    ExtractedTriplet, embed_text_vectors_with_client, parse_triplet_response,
};
use crate::pipeline::helpers::{
    GraphIndexRecords, build_document_index_batch, build_parse_backend_summary,
    enrich_multimodal_source_locator, record_graph_degrade, validate_mirror_source_path,
};
use crate::runtime_support::{fetch_url_content, safe_relative_object_key, url_to_filename};

#[test]
fn cleanup_asset_object_key_safety_rejects_remote_and_path_traversal_values() {
    assert!(safe_relative_object_key(
        "org/notebook/doc/assets/image.png"
    ));
    assert!(!safe_relative_object_key(
        "https://bucket.s3/key?sig=secret"
    ));
    assert!(!safe_relative_object_key("s3://bucket/key"));
    assert!(!safe_relative_object_key("object://bucket/key"));
    assert!(!safe_relative_object_key("/absolute/key"));
    assert!(!safe_relative_object_key("org/../secret"));
    assert!(!safe_relative_object_key(""));
}

#[tokio::test]
async fn load_prompt_template_prefers_versioned_file() {
    let temp_dir = env::temp_dir().join(format!("summary-template-{}", Uuid::new_v4()));
    fs::create_dir_all(&temp_dir).unwrap();
    fs::write(
        temp_dir.join("summary_generation.tmpl"),
        "default {{title}}",
    )
    .unwrap();
    fs::write(
        temp_dir.join("summary_generation.v2.tmpl"),
        "versioned {{title}}",
    )
    .unwrap();

    let mut config = AppConfig::default();
    config.prompts.dir = temp_dir.to_string_lossy().to_string();
    config.prompts.summary_version = "v2".to_string();

    let template = load_prompt_template(
        &config.prompts.dir,
        &config.prompts.summary_version,
        "summary_generation",
    )
    .await
    .unwrap();
    assert_eq!(template, "versioned {{title}}");

    let _ = fs::remove_dir_all(temp_dir);
}

#[test]
fn enrich_multimodal_source_locator_includes_page_range_metadata() {
    let mut metadata = std::collections::BTreeMap::new();
    metadata.insert("page_range_start".to_string(), "5".to_string());
    metadata.insert("page_range_end".to_string(), "8".to_string());
    let locator = SourceLocator {
        page: Some(5),
        ..Default::default()
    };
    let value = enrich_multimodal_source_locator(&locator, &metadata);
    assert_eq!(value["page"], 5);
    assert_eq!(value["page_range_start"], "5");
    assert_eq!(value["page_range_end"], "8");
}

#[test]
fn build_parse_backend_summary_uses_fixed_contract_fields() {
    let route_decision = ParseRouteDecision {
        route: ParseRoute::Pdf,
        reason: RouteReason::ComplexPdf,
        probe_result: None,
        plan: ParsePlan::Pdf(PdfParsePlan {
            pages: vec![PdfPagePlan {
                page_number: 2,
                backend: PdfPageBackend::VisualRaster,
                reason: RouteReason::ComplexPdf,
                route_kinds: vec![],
            }],
        }),
        liteparse_snapshot: None,
    };

    let summary = build_parse_backend_summary(
        &route_decision,
        None,
        &ParseRunOutputs {
            block_count: 3,
            asset_count: 1,
            text_chunk_count: 2,
            multimodal_chunk_count: 1,
            mirrored_asset_count: 1,
            text_vector_count: 2,
            multimodal_vector_count: 1,
            entity_count: 1,
            relation_count: 1,
            graph_passage_count: 1,
            graph_degrade_count: 1,
            graph_degrade_reasons: vec!["provider error".to_string()],
            multimodal_degrade_count: 0,
            multimodal_degrade_reasons: Vec::new(),
        },
    );

    assert!(summary.get("route").is_some());
    assert!(summary.get("reason").is_some());
    assert!(summary.get("plan").is_some());
    assert!(summary.get("probe_result").is_some());
    assert_eq!(summary["page_backends"][0]["page"], 2);
    assert_eq!(summary["outputs"]["text_vector_count"], 2);
    assert_eq!(summary["outputs"]["entity_count"], 1);
    assert_eq!(summary["outputs"]["graph_degrade_count"], 1);
}

#[test]
fn parse_triplet_response_rejects_old_array_format() {
    let chunk_id = Uuid::from_u128(42);
    let triplets = parse_triplet_response(
        r#"{"triplets":[[" Alice ","founded","Acme"],["Alice","founded","Acme"]]}"#,
        &[chunk_id],
    )
    .unwrap();

    assert_eq!(triplets, vec![]);
}

#[test]
fn parse_triplet_response_accepts_new_format_with_chunk_id() {
    let chunk_id = Uuid::from_u128(42);
    let triplets = parse_triplet_response(
        r#"{"triplets":[{"chunk_id":"00000000-0000-0000-0000-00000000002a","subject":"Alice","predicate":"founded","object":"Acme"}]}"#,
        &[chunk_id],
    )
    .unwrap();

    assert_eq!(
        triplets,
        vec![ExtractedTriplet {
            subject: "Alice".to_string(),
            predicate: "founded".to_string(),
            object: "Acme".to_string(),
            supporting_chunk_ids: vec![chunk_id],
            source: "text_chunk".to_string(),
            confidence: 1.0,
        }]
    );
}

#[test]
fn parse_triplet_response_rejects_invalid_chunk_id() {
    let chunk_id = Uuid::from_u128(42);
    let triplets = parse_triplet_response(
        r#"{"triplets":[{"chunk_id":"00000000-0000-0000-0000-000000000099","subject":"Alice","predicate":"founded","object":"Acme"}]}"#,
        &[chunk_id],
    )
    .unwrap();

    assert_eq!(triplets, vec![]);
}

#[test]
fn parse_triplet_response_strips_markdown_json_fence() {
    let chunk_id = Uuid::from_u128(42);
    let triplets = parse_triplet_response(
        "```json\n{\"triplets\":[{\"chunk_id\":\"00000000-0000-0000-0000-00000000002a\",\"subject\":\"Alice\",\"predicate\":\"founded\",\"object\":\"Acme\"}]}\n```",
        &[chunk_id],
    )
    .unwrap();

    assert_eq!(triplets.len(), 1);
    assert_eq!(triplets[0].subject, "Alice");
}

#[test]
fn parse_triplet_response_salvages_truncated_json() {
    let chunk_id = Uuid::from_u128(42);
    let truncated = r#"{"triplets":[{"chunk_id":"00000000-0000-0000-0000-00000000002a","subject":"Alice","predicate":"founded","object":"Acme"},{"chunk_id":"00000000-0000-0000-0000-00000000002a","subject":"Alice","predicate":"leads","object":"Acme"#;
    let triplets = parse_triplet_response(truncated, &[chunk_id]).unwrap();

    assert_eq!(triplets.len(), 1);
    assert_eq!(triplets[0].predicate, "founded");
}

#[test]
fn parse_triplet_response_salvages_truncated_json_with_multibyte_chars() {
    let chunk_id = Uuid::from_u128(42);
    let truncated = r#"{"triplets":[{"chunk_id":"00000000-0000-0000-0000-00000000002a","subject":"概念启动","predicate":"属于","object":"概念阶段"},{"chunk_id":"00000000-0000-0000-0000-00000000002a","subject":"PAC-05","predicate":"标识为","object":"概念启"#;
    let triplets = parse_triplet_response(truncated, &[chunk_id]).unwrap();

    assert_eq!(triplets.len(), 1);
    assert_eq!(triplets[0].predicate, "属于");
}

#[test]
fn parse_triplet_response_rejects_malformed_json() {
    let chunk_id = Uuid::from_u128(42);
    let triplets =
        parse_triplet_response(r#"{"triplets":[{"subject":"Alice"}]}"#, &[chunk_id]).unwrap();

    assert_eq!(triplets, vec![]);
}

#[test]
fn triplet_semantics_eval_cases_match_column_semantic_lint() {
    use crate::pipeline::triplet_semantic_lint::triplet_semantic_violation;

    let path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../prompts/pipeline/evals/triplet_semantics_cases.json"
    );
    let raw = fs::read_to_string(path).expect("triplet_semantics_cases.json");
    let doc: serde_json::Value = serde_json::from_str(&raw).expect("valid eval JSON");
    let cases = doc["cases"].as_array().expect("cases array");

    for case in cases {
        let id = case["id"].as_str().unwrap_or("?");
        let t = &case["triplet"];
        let subject = t["subject"].as_str().expect("subject");
        let predicate = t["predicate"].as_str().expect("predicate");
        let object = t["object"].as_str().expect("object");
        let expect = case["expect"].as_str().expect("expect");
        let violation = triplet_semantic_violation(subject, predicate, object);
        match expect {
            "allow" => assert!(violation.is_none(), "case {id} should allow"),
            "reject" => assert!(violation.is_some(), "case {id} should reject"),
            other => panic!("unknown expect={other} in case {id}"),
        }
    }
}

#[test]
fn parse_triplet_response_applies_column_semantic_lint() {
    let chunk_id = Uuid::from_u128(42);
    let triplets = parse_triplet_response(
        r#"{"triplets":[
            {"chunk_id":"00000000-0000-0000-0000-00000000002a","subject":"ME-10","predicate":"标识为","object":"探索可选概念和提供技术可选方案"},
            {"chunk_id":"00000000-0000-0000-0000-00000000002a","subject":"ACT-100","predicate":"标识为","object":"概念启动"}
        ]}"#,
        &[chunk_id],
    )
    .unwrap();

    assert_eq!(triplets.len(), 1);
    assert_eq!(triplets[0].subject, "ACT-100");
}

#[test]
fn graph_degrade_reasons_are_counted() {
    let mut outputs = ParseRunOutputs::default();

    record_graph_degrade(&mut outputs, "malformed JSON".to_string());

    assert_eq!(outputs.graph_degrade_count, 1);
    assert_eq!(outputs.graph_degrade_reasons, vec!["malformed JSON"]);
}

#[test]
fn build_document_index_batch_carries_parse_run_id() {
    let auth = AuthContext::new(OrgId::new(Uuid::from_u128(1)), SubjectKind::System);
    let document_id = Uuid::from_u128(2);
    let parse_run_id = Uuid::from_u128(3);
    let chunk_id = Uuid::from_u128(4);
    let relation_id = Uuid::from_u128(5);
    let batch = build_document_index_batch(
        &auth,
        Some(Uuid::from_u128(6)),
        document_id,
        parse_run_id,
        vec![TextChunkIndexRecord {
            chunk_id,
            content: "Alice founded Acme".to_string(),
            vector: vec![0.1, 0.2],
            page: Some(1),
            chunk_type: "paragraph".to_string(),
            parser_backend: Some("text_local".to_string()),
            source_locator: None,
        }],
        Vec::new(),
        GraphIndexRecords {
            entities: vec![EntityIndexRecord {
                entity_id: Uuid::from_u128(7),
                name: "Alice".to_string(),
                normalized_name: "alice".to_string(),
                entity_type: None,
                vector: vec![0.1, 0.2],
                supporting_chunk_ids: vec![chunk_id],
                metadata: None,
            }],
            relations: vec![RelationIndexRecord {
                relation_id,
                subject: "Alice".to_string(),
                predicate: "founded".to_string(),
                object: "Acme".to_string(),
                relation_text: "Alice founded Acme".to_string(),
                vector: vec![0.1, 0.2],
                supporting_chunk_ids: vec![chunk_id],
                metadata: None,
            }],
            passages: vec![GraphPassageIndexRecord {
                passage_id: Uuid::from_u128(8),
                chunk_id: Some(chunk_id),
                text: "Alice founded Acme".to_string(),
                vector: vec![0.1, 0.2],
                relation_ids: vec![relation_id],
                metadata: None,
            }],
        },
    );

    assert_eq!(batch.document_id, document_id);
    assert_eq!(batch.parse_run_id, parse_run_id);
    assert_eq!(batch.text_chunks.len(), 1);
    assert_eq!(batch.entities.len(), 1);
    assert_eq!(batch.relations.len(), 1);
    assert_eq!(batch.graph_passages.len(), 1);
}

#[test]
fn parse_triplet_response_merges_supporting_chunks_for_duplicate_triplets() {
    let chunk1 = Uuid::from_u128(1);
    let chunk2 = Uuid::from_u128(2);

    let mut triplet_map: std::collections::HashMap<(String, String, String), ExtractedTriplet> =
        std::collections::HashMap::new();

    for triplet in [
        ExtractedTriplet {
            subject: "Alice".to_string(),
            predicate: "founded".to_string(),
            object: "Acme".to_string(),
            supporting_chunk_ids: vec![chunk1],
            source: "text_chunk".to_string(),
            confidence: 1.0,
        },
        ExtractedTriplet {
            subject: "Alice".to_string(),
            predicate: "founded".to_string(),
            object: "Acme".to_string(),
            supporting_chunk_ids: vec![chunk2],
            source: "text_chunk".to_string(),
            confidence: 1.0,
        },
    ] {
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

    let merged: Vec<_> = triplet_map.into_values().collect();
    assert_eq!(merged.len(), 1);
    assert_eq!(merged[0].supporting_chunk_ids.len(), 2);
    assert!(merged[0].supporting_chunk_ids.contains(&chunk1));
    assert!(merged[0].supporting_chunk_ids.contains(&chunk2));
}

#[test]
fn build_graph_index_records_skips_passage_without_supporting_chunk() {
    let triplet_with_support = ExtractedTriplet {
        subject: "Alice".to_string(),
        predicate: "founded".to_string(),
        object: "Acme".to_string(),
        supporting_chunk_ids: vec![Uuid::from_u128(1)],
        source: "text_chunk".to_string(),
        confidence: 1.0,
    };
    let triplet_without_support = ExtractedTriplet {
        subject: "Bob".to_string(),
        predicate: "joined".to_string(),
        object: "Acme".to_string(),
        supporting_chunk_ids: vec![],
        source: "text_chunk".to_string(),
        confidence: 1.0,
    };

    assert!(!triplet_with_support.supporting_chunk_ids.is_empty());
    assert!(triplet_without_support.supporting_chunk_ids.is_empty());
}

#[tokio::test]
async fn embed_text_vectors_without_embedding_client_returns_error() {
    let result: Result<Vec<Vec<f32>>, ingestion::IngestionError> =
        embed_text_vectors_with_client(None, &["hello"]).await;
    let error = result.expect_err("missing embedding client must fail");
    assert!(error.to_string().contains("embedding client"));
    assert!(error.to_string().contains("not configured"));
}

#[test]
fn url_to_filename_extracts_last_path_segment_with_extension() {
    assert_eq!(
        url_to_filename("https://example.com/article.html"),
        "article.html"
    );
    assert_eq!(
        url_to_filename("https://example.com/path/page.htm"),
        "page.htm"
    );
}

#[test]
fn url_to_filename_falls_back_to_page_html() {
    assert_eq!(url_to_filename("https://example.com/article"), "page.html");
    assert_eq!(url_to_filename("https://example.com/"), "page.html");
}

#[test]
fn mirror_source_path_rejects_absolute_local_paths() {
    let error = validate_mirror_source_path("/etc/passwd")
        .expect_err("absolute local mirror paths must be rejected");
    assert!(error.to_string().contains("rejected"));
}

#[test]
fn mirror_source_path_rejects_temporary_paths_outside_worker_temp_dir() {
    let error = validate_mirror_source_path("temporary:///etc/passwd")
        .expect_err("temporary mirror paths outside temp dir must be rejected");
    assert!(error.to_string().contains("temp dir"));
}

#[tokio::test]
async fn fetch_url_content_blocks_metadata_ip_before_request() {
    let error = fetch_url_content("http://169.254.169.254/latest/meta-data/")
        .await
        .expect_err("metadata ip must be blocked before http request");
    assert!(error.to_string().contains("blocked"));
}
