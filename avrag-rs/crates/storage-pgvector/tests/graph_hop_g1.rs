//! G1: pgvector `search_graph` hop contract (no LLM).
//!
//! Spec: `avrag-rs/docs/engineering/2026-08-04-pgvector-graph-hop-g1-spec.md`
//!
//! Requires `DATABASE_URL` with pgvector + migration 0060 (`rag_kg_*` tables).
//! Soft-skips when the env is missing or schema is not ready (same pattern as
//! `storage-pg` live tests) so default `cargo test` stays green offline.
//!
//! ```bash
//! # from avrag-rs, with .env loaded or DATABASE_URL set:
//! cargo test -p avrag-storage-pgvector --test graph_hop_g1 -- --nocapture
//! ```

use std::collections::HashSet;
use std::env;

use avrag_retrieval_data_plane::{
    DocumentIndexBatch, EntityIndexRecord, GraphSearchRequest, RelationIndexRecord,
    RelationPathCandidate, RetrievalDataPlane, RetrievalReadPort, TextChunkIndexRecord,
};
use avrag_storage_pgvector::{PgvectorConfig, PgvectorDataPlane};
use contracts::auth_runtime::{AuthContext, SubjectKind, UserId};
use sqlx::postgres::PgPoolOptions;
use uuid::Uuid;

const DIM: usize = 1024;

type Triple = (String, String, String);

fn unit_vec(hotspot: usize) -> Vec<f32> {
    let mut v = vec![0.0; DIM];
    if hotspot < DIM {
        v[hotspot] = 1.0;
    }
    v
}

fn triple(p: &RelationPathCandidate) -> Triple {
    (
        p.subject.clone(),
        p.predicate.clone(),
        p.object.clone(),
    )
}

fn set_of(paths: &[RelationPathCandidate]) -> HashSet<Triple> {
    paths.iter().map(triple).collect()
}

fn t(s: &str, p: &str, o: &str) -> Triple {
    (s.to_string(), p.to_string(), o.to_string())
}

struct Live {
    plane: PgvectorDataPlane,
}

async fn try_live() -> Option<Live> {
    let database_url = match env::var("DATABASE_URL") {
        Ok(u) if !u.trim().is_empty() => u,
        _ => {
            eprintln!("graph_hop_g1: skip — DATABASE_URL not set");
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
            eprintln!("graph_hop_g1: skip — connect failed: {e}");
            return None;
        }
    };
    let plane = PgvectorDataPlane::new(
        pool,
        PgvectorConfig {
            text_vector_dim: DIM,
            multimodal_vector_dim: DIM,
            hnsw_ef_search: Some(40),
        },
    );
    if let Err(e) = plane.ensure_schema().await {
        eprintln!("graph_hop_g1: skip — ensure_schema failed: {e}");
        return None;
    }
    Some(Live { plane })
}

struct ChainIds {
    owner: Uuid,
    doc: Uuid,
    other_owner: Uuid,
    other_doc: Uuid,
    parse: Uuid,
    chunk: Uuid,
    r_ab: Uuid,
    r_bc: Uuid,
    r_cd: Uuid,
}

impl ChainIds {
    fn fresh() -> Self {
        Self {
            owner: Uuid::new_v4(),
            doc: Uuid::new_v4(),
            other_owner: Uuid::new_v4(),
            other_doc: Uuid::new_v4(),
            parse: Uuid::new_v4(),
            chunk: Uuid::new_v4(),
            r_ab: Uuid::new_v4(),
            r_bc: Uuid::new_v4(),
            r_cd: Uuid::new_v4(),
        }
    }

    fn auth(&self) -> AuthContext {
        AuthContext::new(UserId::from(self.owner), SubjectKind::System)
    }

    fn auth_other(&self) -> AuthContext {
        AuthContext::new(UserId::from(self.other_owner), SubjectKind::System)
    }
}

/// Alpha → Beta → Gamma → Delta
fn chain_batch(ids: &ChainIds, with_branches: bool) -> DocumentIndexBatch {
    let mut entities = vec![
        entity("Alpha", "alpha", 1, ids.chunk),
        entity("Beta", "beta", 2, ids.chunk),
        entity("Gamma", "gamma", 3, ids.chunk),
        entity("Delta", "delta", 4, ids.chunk),
    ];
    let mut relations = vec![
        rel(ids.r_ab, "Alpha", "depends_on", "Beta", 10, ids.chunk),
        rel(ids.r_bc, "Beta", "uses", "Gamma", 11, ids.chunk),
        rel(ids.r_cd, "Gamma", "owns", "Delta", 12, ids.chunk),
    ];

    if with_branches {
        for i in 1..=5 {
            let name = format!("Z{i}");
            entities.push(entity(&name, &name.to_lowercase(), 20 + i, ids.chunk));
            relations.push(rel(
                Uuid::new_v4(),
                "Beta",
                "branches_to",
                &name,
                30 + i,
                ids.chunk,
            ));
        }
    }

    DocumentIndexBatch {
        owner_user_id: UserId::from(ids.owner),
        workspace_id: None,
        document_id: ids.doc,
        parse_run_id: ids.parse,
        doc_version: 1,
        text_chunks: vec![TextChunkIndexRecord {
            chunk_id: ids.chunk,
            content: "Alpha depends on Beta uses Gamma owns Delta".into(),
            vector: unit_vec(3),
            page: Some(1),
            chunk_type: "text".into(),
            parser_backend: Some("g1".into()),
            source_locator: None,
        }],
        multimodal_chunks: vec![],
        entities,
        relations,
        graph_passages: vec![],
    }
}

fn entity(name: &str, normalized: &str, hotspot: usize, chunk: Uuid) -> EntityIndexRecord {
    EntityIndexRecord {
        entity_id: Uuid::new_v4(),
        name: name.into(),
        normalized_name: normalized.into(),
        entity_type: None,
        vector: unit_vec(hotspot),
        supporting_chunk_ids: vec![chunk],
        metadata: None,
    }
}

fn rel(
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

fn graph_req(
    entity_names: Vec<String>,
    query_entities: Vec<String>,
    query_entity_vectors: Vec<Vec<f32>>,
    hop_limit: usize,
    fan_out_limit: usize,
    relation_limit: usize,
    doc_ids: Option<Vec<Uuid>>,
    owner: Uuid,
) -> GraphSearchRequest {
    GraphSearchRequest {
        auth: AuthContext::new(UserId::from(owner), SubjectKind::System),
        doc_ids,
        entity_names,
        relation_hints: vec![],
        relation_limit,
        supporting_chunk_limit: 50,
        query_entities,
        query_entity_vectors,
        hop_limit,
        fan_out_limit,
        owner_user_id: owner.to_string(),
    }
}

async fn index_chain(live: &Live, ids: &ChainIds, with_branches: bool) {
    live.plane
        .replace_document_index(chain_batch(ids, with_branches))
        .await
        .expect("replace_document_index");
}

async fn cleanup(live: &Live, ids: &ChainIds) {
    let _ = live
        .plane
        .delete_document_index(&ids.auth(), ids.doc)
        .await;
    let _ = live
        .plane
        .delete_document_index(&ids.auth(), ids.other_doc)
        .await;
    let _ = live
        .plane
        .delete_document_index(&ids.auth_other(), ids.other_doc)
        .await;
}

// ── H: hop semantics ────────────────────────────────────────────────────────

#[tokio::test]
async fn g1_h1_hop1_only_direct_neighbor() {
    let Some(live) = try_live().await else {
        return;
    };
    let ids = ChainIds::fresh();
    index_chain(&live, &ids, false).await;

    let out = live
        .plane
        .search_graph(graph_req(
            vec!["Alpha".into()],
            vec![],
            vec![],
            1,
            50,
            50,
            Some(vec![ids.doc]),
            ids.owner,
        ))
        .await
        .expect("search_graph");

    let got = set_of(&out.relation_paths);
    assert_eq!(
        got,
        HashSet::from([t("Alpha", "depends_on", "Beta")]),
        "hop=1 must only see R_AB; got {got:?}"
    );
    assert!(!out.supporting_chunks.is_empty());
    assert!(!got.contains(&t("Beta", "uses", "Gamma")));
    assert!(!got.contains(&t("Gamma", "owns", "Delta")));

    cleanup(&live, &ids).await;
}

#[tokio::test]
async fn g1_h2_hop2_reaches_second_hop() {
    let Some(live) = try_live().await else {
        return;
    };
    let ids = ChainIds::fresh();
    index_chain(&live, &ids, false).await;

    let out = live
        .plane
        .search_graph(graph_req(
            vec!["Alpha".into()],
            vec![],
            vec![],
            2,
            50,
            50,
            Some(vec![ids.doc]),
            ids.owner,
        ))
        .await
        .expect("search_graph");

    let got = set_of(&out.relation_paths);
    assert!(
        got.contains(&t("Alpha", "depends_on", "Beta")),
        "missing R_AB: {got:?}"
    );
    assert!(
        got.contains(&t("Beta", "uses", "Gamma")),
        "missing R_BC (VGRAG hop=2): {got:?}"
    );
    assert!(
        !got.contains(&t("Gamma", "owns", "Delta")),
        "hop=2 must not include R_CD: {got:?}"
    );

    cleanup(&live, &ids).await;
}

#[tokio::test]
async fn g1_h3_hop3_full_chain() {
    let Some(live) = try_live().await else {
        return;
    };
    let ids = ChainIds::fresh();
    index_chain(&live, &ids, false).await;

    let out = live
        .plane
        .search_graph(graph_req(
            vec!["Alpha".into()],
            vec![],
            vec![],
            3,
            50,
            50,
            Some(vec![ids.doc]),
            ids.owner,
        ))
        .await
        .expect("search_graph");

    let got = set_of(&out.relation_paths);
    for edge in [
        t("Alpha", "depends_on", "Beta"),
        t("Beta", "uses", "Gamma"),
        t("Gamma", "owns", "Delta"),
    ] {
        assert!(got.contains(&edge), "missing {edge:?} in {got:?}");
    }

    cleanup(&live, &ids).await;
}

#[tokio::test]
async fn g1_h0_hop_limit_zero_empty() {
    let Some(live) = try_live().await else {
        return;
    };
    let ids = ChainIds::fresh();
    index_chain(&live, &ids, false).await;

    let out = live
        .plane
        .search_graph(graph_req(
            vec!["Alpha".into()],
            vec![],
            vec![],
            0,
            50,
            50,
            Some(vec![ids.doc]),
            ids.owner,
        ))
        .await
        .expect("search_graph");

    assert!(
        out.relation_paths.is_empty(),
        "hop_limit=0 must not expand: {:?}",
        out.relation_paths
    );

    cleanup(&live, &ids).await;
}

#[tokio::test]
async fn g1_h2_mid_seed_beta_undirected() {
    let Some(live) = try_live().await else {
        return;
    };
    let ids = ChainIds::fresh();
    index_chain(&live, &ids, false).await;

    let out = live
        .plane
        .search_graph(graph_req(
            vec!["Beta".into()],
            vec![],
            vec![],
            2,
            50,
            50,
            Some(vec![ids.doc]),
            ids.owner,
        ))
        .await
        .expect("search_graph");

    let got = set_of(&out.relation_paths);
    // Beta touches R_AB (as object) and R_BC (as subject) at hop 1;
    // hop 2 can reach R_CD via Gamma.
    assert!(
        got.contains(&t("Alpha", "depends_on", "Beta")),
        "missing reverse-adjacent R_AB: {got:?}"
    );
    assert!(
        got.contains(&t("Beta", "uses", "Gamma")),
        "missing R_BC: {got:?}"
    );

    cleanup(&live, &ids).await;
}

// ── S: seeds ────────────────────────────────────────────────────────────────

#[tokio::test]
async fn g1_s1_entity_names_surface() {
    let Some(live) = try_live().await else {
        return;
    };
    let ids = ChainIds::fresh();
    index_chain(&live, &ids, false).await;

    let out = live
        .plane
        .search_graph(graph_req(
            vec!["Alpha".into()],
            vec![],
            vec![],
            1,
            50,
            50,
            Some(vec![ids.doc]),
            ids.owner,
        ))
        .await
        .expect("search_graph");

    assert!(set_of(&out.relation_paths).contains(&t("Alpha", "depends_on", "Beta")));
    cleanup(&live, &ids).await;
}

#[tokio::test]
async fn g1_s2_query_entities_lowercase_matches_surface_edges() {
    // Spec option (a): lowercase query_entities must expand against "Alpha" edges.
    let Some(live) = try_live().await else {
        return;
    };
    let ids = ChainIds::fresh();
    index_chain(&live, &ids, false).await;

    let out = live
        .plane
        .search_graph(graph_req(
            vec![],
            vec!["alpha".into()],
            vec![],
            1,
            50,
            50,
            Some(vec![ids.doc]),
            ids.owner,
        ))
        .await
        .expect("search_graph");

    let got = set_of(&out.relation_paths);
    assert!(
        got.contains(&t("Alpha", "depends_on", "Beta")),
        "G1-S2: query_entities=[\"alpha\"] must match subject Alpha; got {got:?}"
    );

    cleanup(&live, &ids).await;
}

#[tokio::test]
async fn g1_s3_entity_ann_seed() {
    let Some(live) = try_live().await else {
        return;
    };
    let ids = ChainIds::fresh();
    index_chain(&live, &ids, false).await;

    let out = live
        .plane
        .search_graph(graph_req(
            vec![],
            vec![],
            vec![unit_vec(1)], // hotspot of Alpha
            1,
            50,
            50,
            Some(vec![ids.doc]),
            ids.owner,
        ))
        .await
        .expect("search_graph");

    let got = set_of(&out.relation_paths);
    assert!(
        got.contains(&t("Alpha", "depends_on", "Beta")),
        "ANN seed near Alpha should expand R_AB; got {got:?}"
    );

    cleanup(&live, &ids).await;
}

#[tokio::test]
async fn g1_s4_empty_seeds() {
    let Some(live) = try_live().await else {
        return;
    };
    let ids = ChainIds::fresh();
    index_chain(&live, &ids, false).await;

    let out = live
        .plane
        .search_graph(graph_req(
            vec![],
            vec![],
            vec![],
            2,
            50,
            50,
            Some(vec![ids.doc]),
            ids.owner,
        ))
        .await
        .expect("search_graph");

    assert!(out.relation_paths.is_empty());
    assert!(out.supporting_chunks.is_empty());
    cleanup(&live, &ids).await;
}

#[tokio::test]
async fn g1_s5_multi_seed() {
    let Some(live) = try_live().await else {
        return;
    };
    let ids = ChainIds::fresh();
    index_chain(&live, &ids, false).await;

    let out = live
        .plane
        .search_graph(graph_req(
            vec!["Alpha".into(), "Gamma".into()],
            vec![],
            vec![],
            1,
            50,
            50,
            Some(vec![ids.doc]),
            ids.owner,
        ))
        .await
        .expect("search_graph");

    let got = set_of(&out.relation_paths);
    assert!(got.contains(&t("Alpha", "depends_on", "Beta")), "{got:?}");
    assert!(got.contains(&t("Gamma", "owns", "Delta")), "{got:?}");
    cleanup(&live, &ids).await;
}

// ── L: limits ───────────────────────────────────────────────────────────────

#[tokio::test]
async fn g1_l1_relation_limit() {
    let Some(live) = try_live().await else {
        return;
    };
    let ids = ChainIds::fresh();
    index_chain(&live, &ids, true).await;

    let out = live
        .plane
        .search_graph(graph_req(
            vec!["Alpha".into()],
            vec![],
            vec![],
            2,
            50,
            2,
            Some(vec![ids.doc]),
            ids.owner,
        ))
        .await
        .expect("search_graph");

    assert!(
        out.relation_paths.len() <= 2,
        "relation_limit=2, got {}",
        out.relation_paths.len()
    );
    cleanup(&live, &ids).await;
}

#[tokio::test]
async fn g1_l2_fan_out_limit() {
    let Some(live) = try_live().await else {
        return;
    };
    let ids = ChainIds::fresh();
    index_chain(&live, &ids, true).await;

    let out = live
        .plane
        .search_graph(graph_req(
            vec!["Beta".into()],
            vec![],
            vec![],
            1,
            2,
            50,
            Some(vec![ids.doc]),
            ids.owner,
        ))
        .await
        .expect("search_graph");

    assert!(
        out.relation_paths.len() <= 2,
        "fan_out_limit=2, got {}",
        out.relation_paths.len()
    );
    cleanup(&live, &ids).await;
}

#[tokio::test]
async fn g1_l3_relation_dedupe() {
    let Some(live) = try_live().await else {
        return;
    };
    let ids = ChainIds::fresh();
    // Minimal: only R_AB
    let batch = DocumentIndexBatch {
        owner_user_id: UserId::from(ids.owner),
        workspace_id: None,
        document_id: ids.doc,
        parse_run_id: ids.parse,
        doc_version: 1,
        text_chunks: vec![TextChunkIndexRecord {
            chunk_id: ids.chunk,
            content: "Alpha Beta".into(),
            vector: unit_vec(3),
            page: None,
            chunk_type: "text".into(),
            parser_backend: None,
            source_locator: None,
        }],
        multimodal_chunks: vec![],
        entities: vec![
            entity("Alpha", "alpha", 1, ids.chunk),
            entity("Beta", "beta", 2, ids.chunk),
        ],
        relations: vec![rel(
            ids.r_ab,
            "Alpha",
            "depends_on",
            "Beta",
            10,
            ids.chunk,
        )],
        graph_passages: vec![],
    };
    live.plane
        .replace_document_index(batch)
        .await
        .expect("index");

    let out = live
        .plane
        .search_graph(graph_req(
            vec!["Alpha".into(), "Beta".into()],
            vec![],
            vec![],
            2,
            50,
            50,
            Some(vec![ids.doc]),
            ids.owner,
        ))
        .await
        .expect("search_graph");

    let ab_count = out
        .relation_paths
        .iter()
        .filter(|p| triple(p) == t("Alpha", "depends_on", "Beta"))
        .count();
    assert_eq!(ab_count, 1, "R_AB must appear once, got {ab_count}");
    cleanup(&live, &ids).await;
}

// ── I: isolation ────────────────────────────────────────────────────────────

#[tokio::test]
async fn g1_i1_owner_isolation() {
    let Some(live) = try_live().await else {
        return;
    };
    let ids = ChainIds::fresh();
    index_chain(&live, &ids, false).await;

    let out = live
        .plane
        .search_graph(graph_req(
            vec!["Alpha".into()],
            vec![],
            vec![],
            2,
            50,
            50,
            None,
            ids.other_owner,
        ))
        .await
        .expect("search_graph");

    assert!(
        !set_of(&out.relation_paths).contains(&t("Alpha", "depends_on", "Beta")),
        "other owner must not see OWNER edges"
    );
    cleanup(&live, &ids).await;
}

#[tokio::test]
async fn g1_i2_doc_ids_filter() {
    let Some(live) = try_live().await else {
        return;
    };
    let ids = ChainIds::fresh();
    index_chain(&live, &ids, false).await;

    // Other doc on same owner: Foo → Bar
    let foo_chunk = Uuid::new_v4();
    let other = DocumentIndexBatch {
        owner_user_id: UserId::from(ids.owner),
        workspace_id: None,
        document_id: ids.other_doc,
        parse_run_id: Uuid::new_v4(),
        doc_version: 1,
        text_chunks: vec![TextChunkIndexRecord {
            chunk_id: foo_chunk,
            content: "Foo Bar".into(),
            vector: unit_vec(5),
            page: None,
            chunk_type: "text".into(),
            parser_backend: None,
            source_locator: None,
        }],
        multimodal_chunks: vec![],
        entities: vec![
            entity("Foo", "foo", 6, foo_chunk),
            entity("Bar", "bar", 7, foo_chunk),
        ],
        relations: vec![rel(
            Uuid::new_v4(),
            "Foo",
            "links",
            "Bar",
            8,
            foo_chunk,
        )],
        graph_passages: vec![],
    };
    live.plane
        .replace_document_index(other)
        .await
        .expect("other doc");

    let out = live
        .plane
        .search_graph(graph_req(
            vec!["Alpha".into(), "Foo".into()],
            vec![],
            vec![],
            2,
            50,
            50,
            Some(vec![ids.doc]),
            ids.owner,
        ))
        .await
        .expect("search_graph");

    let got = set_of(&out.relation_paths);
    assert!(got.contains(&t("Alpha", "depends_on", "Beta")), "{got:?}");
    assert!(
        !got.contains(&t("Foo", "links", "Bar")),
        "doc filter must exclude other_doc: {got:?}"
    );

    cleanup(&live, &ids).await;
}

#[tokio::test]
async fn g1_i3_empty_doc_ids() {
    let Some(live) = try_live().await else {
        return;
    };
    let ids = ChainIds::fresh();
    index_chain(&live, &ids, false).await;

    let out = live
        .plane
        .search_graph(graph_req(
            vec!["Alpha".into()],
            vec![],
            vec![],
            2,
            50,
            50,
            Some(vec![]),
            ids.owner,
        ))
        .await
        .expect("search_graph");

    assert!(out.relation_paths.is_empty());
    cleanup(&live, &ids).await;
}

#[tokio::test]
async fn g1_i3b_none_doc_ids_fail_closed() {
    let Some(live) = try_live().await else {
        return;
    };
    let ids = ChainIds::fresh();
    index_chain(&live, &ids, false).await;

    let scoped = live
        .plane
        .search_graph(graph_req(
            vec!["Alpha".into()],
            vec![],
            vec![],
            1,
            50,
            50,
            Some(vec![ids.doc]),
            ids.owner,
        ))
        .await
        .expect("search_graph scoped");
    assert!(
        !scoped.relation_paths.is_empty(),
        "scoped search should hit indexed chain"
    );

    let out = live
        .plane
        .search_graph(graph_req(
            vec!["Alpha".into()],
            vec![],
            vec![],
            1,
            50,
            50,
            None,
            ids.owner,
        ))
        .await
        .expect("search_graph none");
    assert!(
        out.relation_paths.is_empty(),
        "doc_ids=None must fail closed, got {:?}",
        out.relation_paths
    );
    cleanup(&live, &ids).await;
}

#[tokio::test]
async fn g1_i4_unknown_seed() {
    let Some(live) = try_live().await else {
        return;
    };
    let ids = ChainIds::fresh();
    index_chain(&live, &ids, false).await;

    let out = live
        .plane
        .search_graph(graph_req(
            vec!["NoSuchEntity".into()],
            vec![],
            vec![],
            2,
            50,
            50,
            Some(vec![ids.doc]),
            ids.owner,
        ))
        .await
        .expect("search_graph");

    assert!(out.relation_paths.is_empty());
    cleanup(&live, &ids).await;
}
