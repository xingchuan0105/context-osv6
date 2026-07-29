//! 2026-07-29 markitdown 换血灌库（spec: `docs/plans/2026-07-29-markitdown-grep-toolcall-spec.md`）。
//!
//! 只替换 5 个缓存 pin 文档在 `rag_text_chunks` 的行（文本 + dense 向量）：
//! summary/profile 行、`document_toc`、`rag_kg_*` triplet 一律不动；doc_id /
//! workspace / owner / doc_version 保持原值 → 同 workspace 检索同时可见新
//! 向量与既有 sidecar。
//!
//! Run:
//! `cargo test -p app --test product_e2e markitdown_reingest --features product-e2e -- --ignored --test-threads=1 --nocapture`

use std::collections::BTreeMap;

use ingestion::chunker::{ChunkPolicy, build_ir_chunk_plan};
use ingestion::ir::{
    BlockIr, BlockModality, BlockType, DocumentIr, DocumentType, ParseBackend, SourceLocator,
};
use sqlx::PgPool;
use uuid::Uuid;

use super::load_env_from_repo_dotenv;

const CORPUS_CACHE: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/tests/e2e_output/realistic_corpus_cache.json"
);
const MD_DIR: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/tests/e2e_output/markitdown_out"
);

/// (语料缓存文件名, markitdown 产出物文件名)——原件解析，非导出 txt。
const DOCS: &[(&str, &str)] = &[
    (
        "huawei_ipd_370_activities.txt",
        "huawei_ipd_370_activities.xlsx.md",
    ),
    ("baiyao_it_planning.txt", "baiyao_it_planning.pdf.md"),
    ("consulting_rbf_drc.txt", "consulting_rbf_drc.docx.md"),
    (
        "consulting_platform_network_effects.txt",
        "consulting_platform_network_effects.docx.md",
    ),
    (
        "thesis_y_refrigeration.txt",
        "thesis_y_refrigeration.docx.md",
    ),
];

const EMBED_BATCH: usize = 8;

fn database_url() -> String {
    std::env::var("RAG_QUALITY_SMOKE_DATABASE_URL").unwrap_or_else(|_| {
        "postgres://avrag:avrag@127.0.0.1:5432/avrag_rs_e2e_smoke".to_string()
    })
}

fn embedding_client() -> avrag_llm::EmbeddingClient {
    let cfg = app_core::ModelProviderConfig {
        base_url: std::env::var("EMBEDDING_BASE_URL").expect("EMBEDDING_BASE_URL"),
        api_key: std::env::var("EMBEDDING_API_KEY").expect("EMBEDDING_API_KEY"),
        model: std::env::var("EMBEDDING_MODEL").expect("EMBEDDING_MODEL"),
        timeout_ms: std::env::var("EMBEDDING_TIMEOUT_MS")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(15_000),
        temperature: None,
        api_style: None,
        dimensions: Some(1024),
        enable_thinking: None,
        enable_cache: None,
        rpm_limit: None,
        tpm_limit: None,
    };
    avrag_llm::EmbeddingClient::new(cfg.to_llm_config().expect("embedding llm config"))
}

/// markitdown markdown → Heading/Paragraph blocks（不触发 T1 管道表重检测：
/// block_type 只给 Heading/Paragraph，表格臂不会点火）。
fn blocks_from_markdown(md: &str) -> Vec<BlockIr> {
    let mut blocks: Vec<BlockIr> = Vec::new();
    let mut buf: Vec<&str> = Vec::new();
    let mut idx = 0usize;
    let mut push = |blocks: &mut Vec<BlockIr>, idx: &mut usize, block_type: BlockType, text: String| {
        if text.trim().is_empty() {
            return;
        }
        blocks.push(BlockIr {
            block_id: format!("b{idx}"),
            page: None,
            block_type,
            modality: BlockModality::TextOnly,
            text,
            alt_text: None,
            asset_refs: Vec::new(),
            caption: None,
            section_path: Vec::new(),
            source_locator: SourceLocator::default(),
            parser_backend: ParseBackend::TextLocal,
            metadata: BTreeMap::new(),
        });
        *idx += 1;
    };
    for line in md.lines() {
        let trimmed = line.trim_start();
        if trimmed.starts_with('#') {
            let pending = buf.join("\n");
            push(&mut blocks, &mut idx, BlockType::Paragraph, pending);
            buf.clear();
            let heading = trimmed.trim_start_matches('#').trim().to_string();
            push(&mut blocks, &mut idx, BlockType::Heading, heading);
        } else {
            buf.push(line);
        }
    }
    let pending = buf.join("\n");
    push(&mut blocks, &mut idx, BlockType::Paragraph, pending);
    blocks
}

fn vector_literal(v: &[f32]) -> String {
    let mut s = String::with_capacity(v.len() * 12);
    s.push('[');
    for (i, f) in v.iter().enumerate() {
        if i > 0 {
            s.push(',');
        }
        s.push_str(&f.to_string());
    }
    s.push(']');
    s
}

#[tokio::test]
#[ignore = "surgical corpus re-ingest; run explicitly"]
async fn markitdown_reingest() {
    load_env_from_repo_dotenv();
    let cache: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(CORPUS_CACHE).expect("read corpus cache"),
    )
    .expect("parse corpus cache");
    let workspace_id = Uuid::parse_str(
        cache["workspace_id"].as_str().expect("cache workspace_id"),
    )
    .expect("workspace uuid");

    let pool = PgPool::connect(&database_url()).await.expect("pg connect");
    let client = embedding_client();

    for (cache_name, md_name) in DOCS {
        let doc_id = Uuid::parse_str(
            cache["docs"][cache_name]
                .as_str()
                .unwrap_or_else(|| panic!("cache missing {cache_name}")),
        )
        .expect("doc uuid");
        // Existing provenance (owner/doc_version) rides over; new parse_run_id.
        let (owner, doc_version): (Uuid, i32) =
            sqlx::query_as("SELECT owner_user_id, doc_version FROM rag_text_chunks WHERE doc_id = $1 LIMIT 1")
                .bind(doc_id)
                .fetch_one(&pool)
                .await
                .unwrap_or_else(|_| panic!("no existing rows for {cache_name} ({doc_id})"));
        let old_count: i64 = sqlx::query_scalar(
            "SELECT count(*) FROM rag_text_chunks WHERE owner_user_id = $1 AND doc_id = $2",
        )
        .bind(owner)
        .bind(doc_id)
        .fetch_one(&pool)
        .await
        .expect("count old rows");
        assert!(old_count > 0, "{cache_name}: refusing to replace 0 rows");

        let md = std::fs::read_to_string(format!("{MD_DIR}/{md_name}"))
            .unwrap_or_else(|_| panic!("read {md_name}"));
        let mut ir = DocumentIr::new(
            doc_id.to_string(),
            cache_name.to_string(),
            DocumentType::Text,
            ParseBackend::TextLocal,
        );
        ir.blocks = blocks_from_markdown(&md);
        let plan = build_ir_chunk_plan(&ir, "corpus.md", &ChunkPolicy::default());
        assert!(
            !plan.text_chunks.is_empty(),
            "{cache_name}: chunker produced no text chunks"
        );

        // Embed in small batches (DashScope batch ceiling).
        let mut vectors: Vec<Vec<f32>> = Vec::with_capacity(plan.text_chunks.len());
        for batch in plan.text_chunks.chunks(EMBED_BATCH) {
            let refs: Vec<&str> = batch.iter().map(|c| c.text.as_str()).collect();
            let mut got = client.embed(&refs).await.expect("embed batch");
            vectors.append(&mut got);
        }
        assert_eq!(vectors.len(), plan.text_chunks.len());
        assert_eq!(vectors[0].len(), 1024, "embedding dim drift");

        let parse_run_id = Uuid::new_v4();
        let mut tx = pool.begin().await.expect("tx begin");
        sqlx::query("DELETE FROM rag_text_chunks WHERE owner_user_id = $1 AND doc_id = $2")
            .bind(owner)
            .bind(doc_id)
            .execute(&mut *tx)
            .await
            .expect("delete old rows");
        for (item, vector) in plan.text_chunks.iter().zip(vectors.iter()) {
            let chunk_id = Uuid::new_v4();
            sqlx::query(
                r#"INSERT INTO rag_text_chunks (
                    id, owner_user_id, workspace_id, doc_id, chunk_id, parse_run_id,
                    doc_version, page, text, text_dense, chunk_type, parser_backend, source_locator
                ) VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10::vector,$11,$12,NULL)"#,
            )
            .bind(chunk_id.to_string())
            .bind(owner)
            .bind(workspace_id)
            .bind(doc_id)
            .bind(chunk_id)
            .bind(parse_run_id)
            .bind(doc_version)
            .bind(item.page.map(|p| p as i64))
            .bind(&item.text)
            .bind(vector_literal(vector))
            .bind(item.block_type.as_str())
            .bind("markitdown")
            .execute(&mut *tx)
            .await
            .expect("insert chunk");
        }
        tx.commit().await.expect("tx commit");
        eprintln!(
            "[markitdown-reingest] {cache_name}: {old_count} -> {} chunks (doc {doc_id})",
            plan.text_chunks.len()
        );
    }

    let total: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM rag_text_chunks WHERE parser_backend = 'markitdown'",
    )
    .fetch_one(&pool)
    .await
    .expect("count markitdown rows");
    eprintln!("[markitdown-reingest] done, markitdown rows total: {total}");
}
