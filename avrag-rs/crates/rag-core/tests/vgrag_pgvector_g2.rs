//! G2: VGRAG orchestration against live **pgvector** data plane (no LLM chat).
//!
//! Spec: `docs/engineering/2026-08-04-pgvector-graph-hop-g1-spec.md` §5.1
//!
//! - Index the G1 chain fixture (Alpha→Beta→Gamma→Delta) into Postgres+pgvector
//! - Run `graph_augment_from_terms` with hop=2
//! - Run product `fuse_vgrag_into_dense` and assert `relation_n` / `graph_n` > 0
//!
//! Soft-skips without `DATABASE_URL` + migration 0060 (same as G1).
//!
//! ```bash
//! cd avrag-rs && set -a && source .env && set +a
//! cargo test -p avrag-rag-core --test vgrag_pgvector_g2 -- --nocapture
//! ```

use std::env;
use std::sync::Arc;

use async_trait::async_trait;
use avrag_rag_core::runtime::tools::graph_augment::{
    graph_augment_from_terms, GraphAugmentConfig,
};
use avrag_rag_core::runtime::tools::vgrag::{self, fuse_vgrag_into_dense};
use avrag_rag_core::RagConfig;
use avrag_rag_core::RagRuntime;
use avrag_rag_core_ports::{
    EmbeddingPort, MultiModalEmbeddingInput, MultiModalRerankDocument, RerankPort, RerankResult,
};
use avrag_retrieval_data_plane::{
    DocumentIndexBatch, EntityIndexRecord, RelationIndexRecord, RetrievalDataPlane,
    ScoredChunk, TextChunkIndexRecord,
};
use avrag_storage_pgvector::{PgvectorConfig, PgvectorDataPlane};
use contracts::auth_runtime::{AuthContext, SubjectKind, UserId};
use sqlx::postgres::PgPoolOptions;
use uuid::Uuid;

const DIM: usize = 1024;

fn unit_vec(hotspot: usize) -> Vec<f32> {
    let mut v = vec![0.0; DIM];
    if hotspot < DIM {
        v[hotspot] = 1.0;
    }
    v
}

/// 1024-d fixture embedder aligned with entity hotspots in the chain fixture.
struct FixtureEmbedding;

#[async_trait]
impl EmbeddingPort for FixtureEmbedding {
    async fn embed(&self, texts: &[&str]) -> anyhow::Result<Vec<Vec<f32>>> {
        Ok(texts
            .iter()
            .map(|t| {
                let lower = t.to_ascii_lowercase();
                if lower.contains("alpha") {
                    unit_vec(1)
                } else if lower.contains("beta") {
                    unit_vec(2)
                } else if lower.contains("gamma") {
                    unit_vec(3)
                } else if lower.contains("delta") {
                    unit_vec(4)
                } else {
                    let mut v = vec![0.05_f32; DIM];
                    if let Some(b) = t.as_bytes().first() {
                        v[0] = (*b as f32) / 255.0;
                    }
                    v
                }
            })
            .collect())
    }

    async fn embed_multimodal_fused(
        &self,
        input: &MultiModalEmbeddingInput,
        _dimension: Option<usize>,
    ) -> anyhow::Result<Vec<f32>> {
        let text = input.text.as_deref().unwrap_or("");
        self.embed(&[text]).await.map(|mut v| v.pop().unwrap_or_else(|| unit_vec(0)))
    }
}

#[derive(Default)]
struct NoopRerank;

#[async_trait]
impl RerankPort for NoopRerank {
    async fn rerank(
        &self,
        _query: &str,
        documents: &[&str],
    ) -> anyhow::Result<Vec<RerankResult>> {
        Ok(documents
            .iter()
            .enumerate()
            .map(|(index, _)| RerankResult {
                index,
                score: 1.0 - index as f32 * 0.01,
            })
            .collect())
    }

    async fn rerank_multimodal_text_query(
        &self,
        _query: &str,
        documents: &[MultiModalRerankDocument],
        top_n: usize,
    ) -> anyhow::Result<Vec<RerankResult>> {
        Ok(documents
            .iter()
            .enumerate()
            .take(top_n.max(1))
            .map(|(index, _)| RerankResult {
                index,
                score: 1.0 - index as f32 * 0.01,
            })
            .collect())
    }
}

struct Fixture {
    plane: PgvectorDataPlane,
    runtime: RagRuntime,
    auth: AuthContext,
    owner: Uuid,
    doc: Uuid,
    chunk: Uuid,
    r_ab: Uuid,
    r_bc: Uuid,
    r_cd: Uuid,
}

async fn try_fixture() -> Option<Fixture> {
    let database_url = match env::var("DATABASE_URL") {
        Ok(u) if !u.trim().is_empty() => u,
        _ => {
            eprintln!("vgrag_pgvector_g2: skip — DATABASE_URL not set");
            return None;
        }
    };
    let pool = match PgPoolOptions::new()
        .max_connections(2)
        .connect(&database_url)
        .await
    {
        Ok(p) => p,
        Err(e) => {
            eprintln!("vgrag_pgvector_g2: skip — connect failed: {e}");
            return None;
        }
    };
    let plane = PgvectorDataPlane::new(
        pool,
        PgvectorConfig {
            text_vector_dim: DIM,
            multimodal_vector_dim: DIM,
            hnsw_ef_search: Some(64),
        },
    );
    if let Err(e) = plane.ensure_schema().await {
        eprintln!("vgrag_pgvector_g2: skip — ensure_schema: {e}");
        return None;
    }

    let owner = Uuid::new_v4();
    let doc = Uuid::new_v4();
    let chunk = Uuid::new_v4();
    let parse = Uuid::new_v4();
    let r_ab = Uuid::new_v4();
    let r_bc = Uuid::new_v4();
    let r_cd = Uuid::new_v4();

    let batch = DocumentIndexBatch {
        owner_user_id: UserId::from(owner),
        workspace_id: None,
        document_id: doc,
        parse_run_id: parse,
        doc_version: 1,
        text_chunks: vec![TextChunkIndexRecord {
            chunk_id: chunk,
            content: "Alpha depends on Beta. Beta uses Gamma. Gamma owns Delta.".into(),
            vector: unit_vec(3),
            page: Some(1),
            chunk_type: "text".into(),
            parser_backend: Some("g2".into()),
            source_locator: None,
            cursor: None,
            member_chunk_ids: vec![],
        }],
        multimodal_chunks: vec![],
        entities: vec![
            ent("Alpha", "alpha", 1, chunk),
            ent("Beta", "beta", 2, chunk),
            ent("Gamma", "gamma", 3, chunk),
            ent("Delta", "delta", 4, chunk),
        ],
        relations: vec![
            edge(r_ab, "Alpha", "depends_on", "Beta", 10, chunk),
            edge(r_bc, "Beta", "uses", "Gamma", 11, chunk),
            edge(r_cd, "Gamma", "owns", "Delta", 12, chunk),
        ],
        graph_passages: vec![],
    };
    if let Err(e) = plane.replace_document_index(batch).await {
        eprintln!("vgrag_pgvector_g2: skip — index failed: {e}");
        return None;
    }

    let config = RagConfig::new_for_data_plane(Arc::new(FixtureEmbedding), None)
        .with_reranker(Arc::new(NoopRerank));
    let data_plane: Arc<dyn avrag_retrieval_data_plane::RetrievalReadPort> =
        Arc::new(plane.clone());
    let runtime = RagRuntime::with_data_plane(config, data_plane);
    let auth = AuthContext::new(UserId::from(owner), SubjectKind::System);

    Some(Fixture {
        plane,
        runtime,
        auth,
        owner,
        doc,
        chunk,
        r_ab,
        r_bc,
        r_cd,
    })
}

fn ent(name: &str, norm: &str, hotspot: usize, chunk: Uuid) -> EntityIndexRecord {
    EntityIndexRecord {
        entity_id: Uuid::new_v4(),
        name: name.into(),
        normalized_name: norm.into(),
        entity_type: None,
        vector: unit_vec(hotspot),
        supporting_chunk_ids: vec![chunk],
        metadata: None,
    }
}

fn edge(
    id: Uuid,
    subject: &str,
    predicate: &str,
    object: &str,
    hotspot: usize,
    chunk: Uuid,
) -> RelationIndexRecord {
    RelationIndexRecord {
        relation_id: id,
        subject: subject.into(),
        predicate: predicate.into(),
        object: object.into(),
        relation_text: format!("{subject} {predicate} {object}"),
        vector: unit_vec(hotspot),
        supporting_chunk_ids: vec![chunk],
        metadata: None,
    }
}

async fn cleanup(fx: &Fixture) {
    let _ = fx
        .plane
        .delete_document_index(&fx.auth, fx.doc)
        .await;
}

/// G2-A: lexical/graph_augment path with product-like hop=2 on pgvector.
#[tokio::test]
async fn g2_graph_augment_hop2_nonempty_on_pgvector() {
    let Some(fx) = try_fixture().await else {
        return;
    };

    let cfg = GraphAugmentConfig {
        enabled: true,
        max_relations: 10,
        seed_limit: 8,
        hops: vgrag::VGRAG_HOPS, // 2
        margin_abs: 0.08,
        margin_rel: 0.90,
        evidence_max_k: 3,
        dense_seed: false, // terms-only; isolate hop expansion from embed ANN
        l_eval_rrf: false,
    };
    let terms = vec!["Alpha".to_string()];
    let doc_scope = vec![fx.doc.to_string()];
    let ctx = graph_augment_from_terms(&fx.runtime, &fx.auth, &terms, &doc_scope, &cfg).await;

    assert!(
        !ctx.is_empty(),
        "graph_augment hop=2 from Alpha must yield graph_context; got empty (owner={} doc={})",
        fx.owner,
        fx.doc
    );

    let subjects_objects: Vec<(String, String)> = ctx
        .iter()
        .filter_map(|g| {
            Some((
                g.get("subject")?.as_str()?.to_string(),
                g.get("object")?.as_str()?.to_string(),
            ))
        })
        .collect();
    assert!(
        subjects_objects
            .iter()
            .any(|(s, o)| s == "Alpha" && o == "Beta"),
        "expected Alpha→Beta in context: {subjects_objects:?}"
    );
    // hop=2 should also reach Beta→Gamma
    assert!(
        subjects_objects
            .iter()
            .any(|(s, o)| s == "Beta" && o == "Gamma"),
        "hop=2 expected Beta→Gamma: {subjects_objects:?}"
    );

    let hop_limits: Vec<u64> = ctx
        .iter()
        .filter_map(|g| g.get("expansion_hop_limit")?.as_u64())
        .collect();
    assert!(
        hop_limits.iter().any(|&h| h >= 2),
        "expansion_hop_limit should be >=2: {hop_limits:?}"
    );

    // Cite-safe evidence present
    let evidence_n: usize = ctx
        .iter()
        .filter_map(|g| g.get("evidence_chunks")?.as_array())
        .map(|a| a.len())
        .sum();
    assert!(evidence_n > 0, "expected cite-safe evidence_chunks");

    cleanup(&fx).await;
}

/// G2-B: product VGRAG fuse path (`DENSE_BACKEND=vgrag` internal) on pgvector.
#[tokio::test]
async fn g2_fuse_vgrag_relation_n_and_graph_n_on_pgvector() {
    let Some(fx) = try_fixture().await else {
        return;
    };

    let query = "Alpha Beta dependency chain";
    let pure_dense = vec![ScoredChunk::new_text(
        fx.chunk,
        fx.doc,
        "Alpha depends on Beta. Beta uses Gamma.".into(),
        0.9,
        "dense".into(),
        Some(1),
    )];
    let doc_scope = vec![fx.doc.to_string()];

    let (fused, stats) =
        fuse_vgrag_into_dense(&fx.runtime, &fx.auth, query, &doc_scope, pure_dense).await;

    assert!(
        stats.relation_n > 0,
        "VGRAG on pgvector must surface relation_n>0; stats={stats:?} (empty graph = silent miss)"
    );
    assert!(
        stats.graph_n > 0,
        "VGRAG on pgvector must fuse cite-safe graph evidence graph_n>0; stats={stats:?} \
         (relation_n={} evidence_raw_n={} dropped={})",
        stats.relation_n,
        stats.evidence_raw_n,
        stats.evidence_dropped
    );
    assert!(
        !fused.is_empty(),
        "fused dense list must be non-empty after VGRAG"
    );
    // Fused list should still be cite-safe UUIDs
    for c in &fused {
        assert!(!c.chunk_id.is_nil(), "nil chunk_id in fused list");
        assert!(!c.doc_id.is_nil(), "nil doc_id in fused list");
    }

    eprintln!(
        "g2_fuse_vgrag: relation_n={} graph_n={} evidence_raw_n={} dropped={} fused_len={}",
        stats.relation_n,
        stats.graph_n,
        stats.evidence_raw_n,
        stats.evidence_dropped,
        fused.len()
    );

    // Silence unused field warnings if we keep ids for debugging
    let _ = (fx.r_ab, fx.r_bc, fx.r_cd);

    cleanup(&fx).await;
}

/// G2-C: dense_seed=true (product default) still works with 1024-d fixture embedder.
#[tokio::test]
async fn g2_graph_augment_dense_seed_ann_on_pgvector() {
    let Some(fx) = try_fixture().await else {
        return;
    };

    let cfg = GraphAugmentConfig {
        enabled: true,
        max_relations: 10,
        seed_limit: 8,
        hops: 2,
        margin_abs: 0.08,
        margin_rel: 0.90,
        evidence_max_k: 3,
        dense_seed: true,
        l_eval_rrf: false,
    };
    // Query terms that match fixture entity vectors via FixtureEmbedding hotspots.
    let terms = vec!["Alpha".to_string(), "uses".to_string()];
    let doc_scope = vec![fx.doc.to_string()];
    let ctx = graph_augment_from_terms(&fx.runtime, &fx.auth, &terms, &doc_scope, &cfg).await;

    assert!(
        !ctx.is_empty(),
        "dense_seed+ANN path must not empty graph_context on pgvector"
    );
    let evidence_n: usize = ctx
        .iter()
        .filter_map(|g| g.get("evidence_chunks")?.as_array())
        .map(|a| a.len())
        .sum();
    assert!(evidence_n > 0);

    cleanup(&fx).await;
}
