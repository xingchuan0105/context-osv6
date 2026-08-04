//! Lexical-forced graph augment (canonical: 2026-07-23-lexical-graph-augment-scoring-design).
//!
//! - Trigger: lexical_search bridge path when `RETRIEVAL_GRAPH_AUGMENT=1`.
//! - Seeds (default): this-hop **terms** (exact / substring match).
//! - Seeds (eval B4): `RETRIEVAL_GRAPH_SEED=dense_multiway` embeds terms → `query_entity_vectors` ANN.
//! - Hop: default 1; `GRAPH_AUGMENT_HOPS` (1..=3) for eval (e.g. B3 hop=3).
//! - L-eval RRF: `GRAPH_L_EVAL_RRF=1` merges BM25 chunks + graph evidence into fused `chunks`.
//! - Evidence: score vs terms + TOP1 score-gap cut.
//! - Does **not** register as a ToolCatalog tool; telemetry may emit graph_retrieval + degrade_reason=graph_augment.

use avrag_retrieval_data_plane::{GraphSearchRequest, ScoredChunk};
use contracts::auth_runtime::AuthContext;
use serde_json::{Value, json};
use uuid::Uuid;

use crate::merge::global_rrf_merge;
use crate::RagRuntime;

/// Env-driven config for lexical graph augment.
#[derive(Debug, Clone)]
pub struct GraphAugmentConfig {
    pub enabled: bool,
    pub max_relations: usize,
    pub seed_limit: usize,
    /// Subgraph expansion hops (product default 1; eval may set 2–3).
    pub hops: usize,
    pub margin_abs: f32,
    pub margin_rel: f32,
    pub evidence_max_k: usize,
    /// When true, embed terms and pass entity ANN seeds (eval B4).
    pub dense_seed: bool,
    /// When true, RRF-merge BM25 chunks with graph evidence into observation `chunks` (L-eval).
    pub l_eval_rrf: bool,
}

impl Default for GraphAugmentConfig {
    fn default() -> Self {
        Self {
            // Product default off: graph expand lives inside dense (VGRAG), not lexical side-car.
            enabled: false,
            max_relations: 5,
            seed_limit: 8,
            hops: 1,
            margin_abs: 0.08,
            margin_rel: 0.90,
            evidence_max_k: 3,
            dense_seed: false,
            l_eval_rrf: false,
        }
    }
}

/// Telemetry marker on `ToolResult.trace.degrade_reason` for forced lexical graph augment.
/// Eval must treat this as **not** an explicit agent `graph_search` call.
pub const GRAPH_AUGMENT_DEGRADE_REASON: &str = "graph_augment";

/// True when this is a graph_retrieval side-car from lexical force-augment (not agent-called).
pub fn is_graph_augment_result(result: &contracts::ToolResult) -> bool {
    result.tool == "graph_retrieval"
        && result
            .trace
            .as_ref()
            .and_then(|t| t.degrade_reason.as_deref())
            == Some(GRAPH_AUGMENT_DEGRADE_REASON)
}

/// True when this is an agent-explicit graph_retrieval (not augment side-car).
pub fn is_graph_explicit_result(result: &contracts::ToolResult) -> bool {
    result.tool == "graph_retrieval" && !is_graph_augment_result(result)
}

pub fn graph_augment_hit(results: &[contracts::ToolResult]) -> bool {
    results
        .iter()
        .any(|r| r.status == contracts::ToolStatus::Ok && is_graph_augment_result(r))
}

pub fn graph_explicit_called(results: &[contracts::ToolResult]) -> bool {
    results
        .iter()
        .any(|r| r.status == contracts::ToolStatus::Ok && is_graph_explicit_result(r))
}

/// Build a capture-only ToolResult for eval/telemetry (does not replace bridge JSON).
pub fn telemetry_tool_result(graph_context: &[Value], elapsed_ms: u64) -> contracts::ToolResult {
    use contracts::{ToolResult, ToolStatus, ToolTrace};
    ToolResult {
        tool: "graph_retrieval".to_string(),
        version: "1.0".to_string(),
        status: ToolStatus::Ok,
        data: Some(json!({
            "graph_context": graph_context,
            "source": "graph_augment",
        })),
        trace: Some(ToolTrace {
            elapsed_ms: Some(elapsed_ms),
            raw_hit_count: Some(graph_context.len()),
            hydrated_hit_count: Some(graph_context.len()),
            degrade_reason: Some(GRAPH_AUGMENT_DEGRADE_REASON.to_string()),
        }),
    }
}

impl GraphAugmentConfig {
    pub fn from_env() -> Self {
        let mut c = Self::default();
        // Unset → off (VGRAG is dense-internal). Explicit 1/true/on → lexical side-car (eval only).
        c.enabled = match std::env::var("RETRIEVAL_GRAPH_AUGMENT") {
            Ok(v) => env_truthy_str(&v),
            Err(_) => false,
        };
        if let Some(v) = env_usize("GRAPH_AUGMENT_MAX_RELATIONS") {
            c.max_relations = v.max(1);
        }
        if let Some(v) = env_usize("GRAPH_AUGMENT_SEED_LIMIT") {
            c.seed_limit = v.max(1);
        }
        // Product default hop=1. hops>1 only when eval/baseline flags set (B3).
        let hops_requested = env_usize("GRAPH_AUGMENT_HOPS").unwrap_or(1).clamp(1, 3);
        c.hops = if hops_requested > 1 && graph_eval_mode_enabled() {
            hops_requested
        } else {
            1
        };
        if let Some(v) = env_f32("GRAPH_EVIDENCE_MARGIN_ABS") {
            c.margin_abs = v.max(0.0);
        }
        if let Some(v) = env_f32("GRAPH_EVIDENCE_MARGIN_REL") {
            c.margin_rel = v.clamp(0.0, 1.0);
        }
        if let Some(v) = env_usize("GRAPH_EVIDENCE_MAX_K") {
            c.evidence_max_k = v.max(1);
        }
        // B4: dense multi-way entity ANN seeds (unset / terms → lexical terms only).
        c.dense_seed = match std::env::var("RETRIEVAL_GRAPH_SEED") {
            Ok(v) => {
                let t = v.trim().to_ascii_lowercase();
                t == "dense_multiway" || t == "dense" || t == "ann"
            }
            Err(_) => false,
        };
        // L-eval: fuse graph evidence into observation chunks via RRF.
        c.l_eval_rrf = match std::env::var("GRAPH_L_EVAL_RRF") {
            Ok(v) => env_truthy_str(&v),
            Err(_) => false,
        };
        c
    }

    /// Production + tests: env by default; tests may install a process-local override
    /// via [`install_test_config`] to avoid parallel `std::env` races.
    pub fn resolve() -> Self {
        if let Ok(guard) = TEST_CONFIG_OVERRIDE.lock() {
            if let Some(cfg) = guard.as_ref() {
                return cfg.clone();
            }
        }
        Self::from_env()
    }
}

static TEST_CONFIG_OVERRIDE: std::sync::Mutex<Option<GraphAugmentConfig>> =
    std::sync::Mutex::new(None);

/// Serializes bridge augment tests that install a process-local config override.
#[cfg(test)]
pub static TEST_CONFIG_SERIAL: std::sync::Mutex<()> = std::sync::Mutex::new(());

/// Test helper: force config for the current process (pair with [`clear_test_config`]).
/// Hold [`TEST_CONFIG_SERIAL`] for the whole test that uses this.
#[cfg(test)]
pub fn install_test_config(cfg: GraphAugmentConfig) {
    *TEST_CONFIG_OVERRIDE.lock().expect("test config mutex") = Some(cfg);
}

#[cfg(test)]
pub fn clear_test_config() {
    *TEST_CONFIG_OVERRIDE.lock().expect("test config mutex") = None;
}

fn env_truthy_str(v: &str) -> bool {
    let t = v.trim();
    t == "1"
        || t.eq_ignore_ascii_case("true")
        || t.eq_ignore_ascii_case("on")
        || t.eq_ignore_ascii_case("yes")
}

fn env_usize(key: &str) -> Option<usize> {
    std::env::var(key).ok()?.trim().parse().ok()
}

fn env_f32(key: &str) -> Option<f32> {
    std::env::var(key).ok()?.trim().parse().ok()
}

/// Build seed entity strings from lexical terms (trim, drop empty, cap).
pub fn seed_entities_from_terms(terms: &[String], seed_limit: usize) -> Vec<String> {
    let mut out = Vec::new();
    let mut seen = std::collections::HashSet::new();
    for t in terms {
        let s = t.trim();
        if s.is_empty() {
            continue;
        }
        let key = s.to_lowercase();
        if !seen.insert(key) {
            continue;
        }
        out.push(s.to_string());
        if out.len() >= seed_limit {
            break;
        }
    }
    out
}

/// Absolute + relative TOP1 margin check (得分落差).
pub fn score_gap_ok(score: f32, top1: f32, margin_abs: f32, margin_rel: f32) -> bool {
    if !score.is_finite() || !top1.is_finite() {
        return false;
    }
    score >= top1 - margin_abs && score >= margin_rel * top1
}

/// Keep indices into a descending-sorted score list under TOP1 gap + K_max.
/// Returns kept indices in score order. Index 0 (TOP1) always kept when non-empty.
pub fn keep_evidence_by_gap(
    scores_desc: &[f32],
    margin_abs: f32,
    margin_rel: f32,
    k_max: usize,
) -> Vec<usize> {
    if scores_desc.is_empty() || k_max == 0 {
        return Vec::new();
    }
    let s1 = scores_desc[0];
    let mut kept = vec![0];
    for (i, &s) in scores_desc.iter().enumerate().skip(1) {
        if kept.len() >= k_max {
            break;
        }
        if score_gap_ok(s, s1, margin_abs, margin_rel) {
            kept.push(i);
        } else {
            break;
        }
    }
    kept
}

/// Term coverage evidence score in [0, 1]: fraction of terms that appear in text (case-insensitive).
pub fn evidence_score_term_coverage(text: &str, terms: &[String]) -> f32 {
    if terms.is_empty() {
        return 0.0;
    }
    let lower = text.to_lowercase();
    let mut hits = 0usize;
    for t in terms {
        let tt = t.trim();
        if tt.is_empty() {
            continue;
        }
        if lower.contains(&tt.to_lowercase()) {
            hits += 1;
        }
    }
    let denom = terms.iter().filter(|t| !t.trim().is_empty()).count().max(1);
    hits as f32 / denom as f32
}

fn seed_terms_hit(subject: &str, object: &str, seeds: &[String]) -> Vec<String> {
    let s_l = subject.to_lowercase();
    let o_l = object.to_lowercase();
    seeds
        .iter()
        .filter(|t| {
            let tl = t.to_lowercase();
            s_l.contains(&tl) || o_l.contains(&tl) || tl.contains(&s_l) || tl.contains(&o_l)
        })
        .cloned()
        .collect()
}

fn relation_term_hits(subject: &str, object: &str, terms: &[String]) -> usize {
    seed_terms_hit(subject, object, terms).len()
}

/// Run 1-hop graph augment from lexical terms. Empty on off/disabled/no seeds/errors.
pub async fn graph_augment_from_terms(
    runtime: &RagRuntime,
    auth: &AuthContext,
    terms: &[String],
    doc_scope: &[String],
    config: &GraphAugmentConfig,
) -> Vec<Value> {
    let started = std::time::Instant::now();
    if !config.enabled {
        return Vec::new();
    }
    let seeds = seed_entities_from_terms(terms, config.seed_limit);
    if seeds.is_empty() {
        tracing::debug!(
            terms = terms.len(),
            "graph_augment skipped: no seeds from terms"
        );
        return Vec::new();
    }

    let doc_ids = if doc_scope.is_empty() {
        None
    } else {
        Some(
            doc_scope
                .iter()
                .filter_map(|id| Uuid::parse_str(id).ok())
                .collect::<Vec<_>>(),
        )
    };

    // entity_names: original case; query_entities: lowercased for exact-name fallback in storage.
    let entity_names = seeds.clone();
    let query_entities: Vec<String> = seeds.iter().map(|s| s.to_lowercase()).collect();

    // B4 / dense_multiway: embed terms (+ joined string) for entity ANN seeds.
    let query_entity_vectors = if config.dense_seed {
        let mut texts: Vec<&str> = terms.iter().map(String::as_str).collect();
        let joined = terms.join(" ");
        if !joined.is_empty() && !terms.iter().any(|t| t == &joined) {
            texts.push(joined.as_str());
        }
        match runtime.config.embedding_client.embed(&texts).await {
            Ok(vectors) => vectors,
            Err(e) => {
                tracing::warn!(error = %e, "graph_augment dense_seed embed failed; terms-only fallback");
                Vec::new()
            }
        }
    } else {
        Vec::new()
    };

    let hop_limit = config.hops.max(1);
    let output = match runtime
        .data_plane
        .search_graph(GraphSearchRequest {
            auth: auth.clone(),
            doc_ids,
            entity_names,
            relation_hints: Vec::new(),
            relation_limit: config.max_relations,
            supporting_chunk_limit: config.max_relations.saturating_mul(config.evidence_max_k),
            query_entities,
            query_entity_vectors,
            hop_limit,
            fan_out_limit: config.max_relations.max(10),
            owner_user_id: auth.user_id().to_string(),
        })
        .await
    {
        Ok(o) => o,
        Err(e) => {
            tracing::warn!(error = %e, "graph_augment search_graph failed");
            return Vec::new();
        }
    };

    if output.relation_paths.is_empty() {
        tracing::debug!(
            seed_count = seeds.len(),
            seeds = ?seeds,
            "graph_augment: no 1-hop relations"
        );
        return Vec::new();
    }

    let relation_path_count = output.relation_paths.len();

    // Storage puts relation_id as supporting_chunks[].chunk_id (not body chunk ids).
    // Index those rows by content touch for weak fallback only.
    let relation_proxy_chunks: &[avrag_retrieval_data_plane::ScoredChunk] =
        output.supporting_chunks.as_slice();

    // Rank relations: more term hits on ends first, then existing path score.
    let mut ranked: Vec<_> = output.relation_paths.into_iter().collect();
    ranked.sort_by(|a, b| {
        let ha = relation_term_hits(&a.subject, &a.object, &seeds);
        let hb = relation_term_hits(&b.subject, &b.object, &seeds);
        hb.cmp(&ha).then_with(|| {
            b.score
                .partial_cmp(&a.score)
                .unwrap_or(std::cmp::Ordering::Equal)
        })
    });
    ranked.truncate(config.max_relations);

    // Best-effort hydrate via control-plane ContentStore (`chunks` table).
    // Graph supporting_chunk_ids usually point at retrieval-index ids (`rag_text_chunks`);
    // those often miss here — cite-safe fallback below still emits UUID+doc_id+relation_text.
    let mut hydrated: std::collections::HashMap<Uuid, common::IndexedChunk> =
        std::collections::HashMap::new();
    if let Some(store) = runtime.content_store() {
        let mut ids: Vec<Uuid> = ranked
            .iter()
            .flat_map(|p| p.supporting_chunk_ids.iter().copied())
            .collect();
        ids.sort_unstable();
        ids.dedup();
        if !ids.is_empty() {
            match store.get_chunks_by_ids(auth, &ids).await {
                Ok(map) => {
                    tracing::debug!(
                        requested = ids.len(),
                        hydrated = map.len(),
                        "graph_augment content_store hydrate"
                    );
                    hydrated = map;
                }
                Err(e) => {
                    tracing::warn!(
                        error = %e,
                        n_ids = ids.len(),
                        "graph_augment content_store hydrate failed; cite-safe relation fallback"
                    );
                }
            }
        }
    }

    let mut graph_context = Vec::new();
    for path in ranked {
        let hits = seed_terms_hit(&path.subject, &path.object, &seeds);
        let relation_text = format!(
            "{} -{}-> {}",
            if path.subject.trim().is_empty() {
                "?"
            } else {
                &path.subject
            },
            if path.predicate.trim().is_empty() {
                "?"
            } else {
                &path.predicate
            },
            if path.object.trim().is_empty() {
                "?"
            } else {
                &path.object
            },
        );

        // Candidate evidence: (chunk_id, doc_id, text, score) — prefer cite-safe UUIDs.
        let mut candidates: Vec<(String, String, String, f32)> = Vec::new();

        for id in &path.supporting_chunk_ids {
            if let Some(ic) = hydrated.get(id) {
                let s = evidence_score_term_coverage(&ic.content, terms);
                let doc = Uuid::parse_str(&ic.doc_id).unwrap_or(path.doc_id);
                if doc.is_nil() {
                    continue;
                }
                candidates.push((id.to_string(), doc.to_string(), ic.content.clone(), s));
            }
        }

        // Cite-safe fallback when hydrate miss but we know body chunk ids + relation doc.
        if candidates.is_empty()
            && !path.supporting_chunk_ids.is_empty()
            && !path.doc_id.is_nil()
        {
            for id in &path.supporting_chunk_ids {
                let s = evidence_score_term_coverage(&relation_text, terms);
                // Prefer relation_text as body when chunk text unavailable (still citeable id).
                candidates.push((
                    id.to_string(),
                    path.doc_id.to_string(),
                    relation_text.clone(),
                    s,
                ));
            }
        }

        // No supporting ids: match storage relation-proxy rows (UUID chunk_id + real doc_id).
        if candidates.is_empty() {
            for c in relation_proxy_chunks {
                if c.doc_id.is_nil() {
                    continue;
                }
                if c.content.contains(&path.subject) || c.content.contains(&path.object) {
                    let s = evidence_score_term_coverage(&c.content, terms);
                    candidates.push((
                        c.chunk_id.to_string(),
                        c.doc_id.to_string(),
                        c.content.clone(),
                        s,
                    ));
                    break;
                }
            }
        }

        // Last resort: relation_text with path.doc_id + first support id or stable key only if doc known.
        // Skip nil-doc synthetic rows — VGRAG C8 drops them; do not invent non-UUID chunk_ids.
        if candidates.is_empty() && !path.doc_id.is_nil() {
            if let Some(id) = path.supporting_chunk_ids.first() {
                let s = evidence_score_term_coverage(&relation_text, terms);
                candidates.push((
                    id.to_string(),
                    path.doc_id.to_string(),
                    relation_text.clone(),
                    s,
                ));
            }
        }

        if candidates.is_empty() {
            tracing::debug!(
                subject = %path.subject,
                object = %path.object,
                support_n = path.supporting_chunk_ids.len(),
                "graph_augment: relation has no cite-safe evidence candidates"
            );
            continue;
        }

        candidates.sort_by(|a, b| b.3.partial_cmp(&a.3).unwrap_or(std::cmp::Ordering::Equal));
        let scores: Vec<f32> = candidates.iter().map(|c| c.3).collect();
        let kept_idx = keep_evidence_by_gap(
            &scores,
            config.margin_abs,
            config.margin_rel,
            config.evidence_max_k,
        );

        let s1 = scores.first().copied().unwrap_or(0.0);
        let evidence_chunks: Vec<Value> = kept_idx
            .into_iter()
            .enumerate()
            .map(|(rank, i)| {
                let (cid, doc_id, text, s) = &candidates[i];
                let gap = s1 - *s;
                let reason = if rank == 0 { "top1" } else { "within_margin" };
                json!({
                    "chunk_id": cid,
                    "doc_id": doc_id,
                    "text": text,
                    "score": s,
                    "score_gap_to_top1": gap,
                    "kept_reason": reason,
                })
            })
            .collect();

        graph_context.push(json!({
            "subject": path.subject,
            "predicate": path.predicate,
            "object": path.object,
            "doc_id": path.doc_id.to_string(),
            "relation_text": relation_text,
            // Actual per-edge hop is not tracked in storage BFS; this is the expand limit used.
            "expansion_hop_limit": hop_limit,
            "seed_terms_hit": hits,
            "seed_mode": if config.dense_seed { "dense_multiway" } else { "terms" },
            "evidence_chunks": evidence_chunks,
            "retrieval_hint": "结构/关系补充；主体答案优先依据 chunks",
        }));
    }

    let evidence_kept: usize = graph_context
        .iter()
        .filter_map(|g| g.get("evidence_chunks").and_then(|e| e.as_array()))
        .map(|a| a.len())
        .sum();
    let max_gap = graph_context
        .iter()
        .filter_map(|g| g.get("evidence_chunks").and_then(|e| e.as_array()))
        .flatten()
        .filter_map(|e| e.get("score_gap_to_top1").and_then(|v| v.as_f64()))
        .fold(0.0_f64, f64::max);

    tracing::info!(
        seed_count = seeds.len(),
        seeds = ?seeds,
        relation_paths_raw = relation_path_count,
        graph_context_len = graph_context.len(),
        evidence_kept,
        max_score_gap_to_top1 = max_gap,
        margin_abs = config.margin_abs,
        margin_rel = config.margin_rel,
        evidence_max_k = config.evidence_max_k,
        elapsed_ms = started.elapsed().as_millis() as u64,
        "graph_augment completed"
    );

    graph_context
}

fn stable_uuid_from_key(key: &str) -> Uuid {
    // Deterministic 16-byte id without uuid v5 feature (eval-only synthetic keys).
    let mut bytes = [0u8; 16];
    for (i, b) in key.as_bytes().iter().enumerate() {
        bytes[i % 16] ^= b.wrapping_mul(31).wrapping_add(i as u8);
    }
    // Set RFC4122 variant/version nibbles loosely so parsers accept the id.
    bytes[6] = (bytes[6] & 0x0f) | 0x40;
    bytes[8] = (bytes[8] & 0x3f) | 0x80;
    Uuid::from_bytes(bytes)
}

/// Eval/baseline mode: allow hops>1 and mark L-eval runs.
pub fn graph_eval_mode_enabled() -> bool {
    for key in [
        "GRAPH_EVAL_MODE",
        "GRAPH_L_EVAL_RRF",
        "GRAPH_EVAL_FORCE_REQUIRED_GRAPH",
        "E2E_GRAPH_BASELINE",
    ] {
        if let Ok(v) = std::env::var(key) {
            let t = v.trim().to_ascii_lowercase();
            if t == "1" || t == "true" || t == "yes" || t == "on" || t.starts_with('b') {
                return true;
            }
        }
    }
    if let Ok(v) = std::env::var("RETRIEVAL_GRAPH_SEED") {
        let t = v.trim().to_ascii_lowercase();
        if t == "dense_multiway" || t == "dense" || t == "ann" {
            return true;
        }
    }
    false
}

/// Extract graph evidence texts into ScoredChunks for L-eval RRF (channel source=`graph`).
pub fn graph_evidence_as_scored_chunks(graph_context: &[Value]) -> Vec<ScoredChunk> {
    let mut out = Vec::new();
    let mut seen = std::collections::HashSet::new();
    for g in graph_context {
        let Some(evs) = g.get("evidence_chunks").and_then(|e| e.as_array()) else {
            continue;
        };
        for ev in evs {
            let Some(text) = ev.get("text").and_then(|t| t.as_str()) else {
                continue;
            };
            if text.trim().is_empty() {
                continue;
            }
            let cid_raw = ev
                .get("chunk_id")
                .and_then(|c| c.as_str())
                .unwrap_or_default();
            // Skip synthetic relation-only rows when a real UUID is required for cite stability
            // unless no real chunks exist — still include with stable id.
            let chunk_id = Uuid::parse_str(cid_raw).unwrap_or_else(|_| stable_uuid_from_key(cid_raw));
            if !seen.insert(chunk_id) {
                continue;
            }
            let doc_id = ev
                .get("doc_id")
                .and_then(|d| d.as_str())
                .and_then(|s| Uuid::parse_str(s).ok())
                .unwrap_or_else(Uuid::nil);
            let score = ev.get("score").and_then(|s| s.as_f64()).unwrap_or(0.0) as f32;
            out.push(ScoredChunk::new_text(
                chunk_id,
                doc_id,
                text.to_string(),
                score,
                "graph".to_string(),
                None,
            ));
        }
    }
    out
}

/// L-eval: RRF-merge BM25/lexical chunks with graph evidence chunks.
/// Returns (fused list, graph_chunk_count_in_input).
pub fn l_eval_rrf_fuse(
    bm25_chunks: Vec<ScoredChunk>,
    graph_context: &[Value],
    rrf_k: usize,
) -> (Vec<ScoredChunk>, usize) {
    let graph_chunks = graph_evidence_as_scored_chunks(graph_context);
    l_eval_rrf_three(Vec::new(), bm25_chunks, graph_chunks, rrf_k)
}

/// L-eval three-way: dense ∪ BM25 ∪ graph → RRF.
/// Returns (fused, graph_input_n).
pub fn l_eval_rrf_three(
    dense_chunks: Vec<ScoredChunk>,
    bm25_chunks: Vec<ScoredChunk>,
    graph_chunks: Vec<ScoredChunk>,
    rrf_k: usize,
) -> (Vec<ScoredChunk>, usize) {
    let n_graph = graph_chunks.len();
    let mut lists = Vec::new();
    if !dense_chunks.is_empty() {
        lists.push((dense_chunks, 1.0));
    }
    if !bm25_chunks.is_empty() {
        lists.push((bm25_chunks, 1.0));
    }
    if !graph_chunks.is_empty() {
        lists.push((graph_chunks, 1.0));
    }
    if lists.is_empty() {
        return (Vec::new(), 0);
    }
    if lists.len() == 1 {
        return (lists.remove(0).0, n_graph);
    }
    (global_rrf_merge(lists, rrf_k), n_graph)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn score_gap_a5_keeps_top_two_only() {
        // A5: s=[1.0, 0.95, 0.70], δ=0.08, α=0.9 → only first 2
        let scores = [1.0_f32, 0.95, 0.70];
        let kept = keep_evidence_by_gap(&scores, 0.08, 0.90, 3);
        assert_eq!(kept, vec![0, 1]);
    }

    #[test]
    fn score_gap_relative_can_drop_even_if_abs_ok() {
        // top1=1.0, second=0.85: abs ok with δ=0.2 but rel 0.9 fails
        let scores = [1.0_f32, 0.85];
        let kept = keep_evidence_by_gap(&scores, 0.20, 0.90, 3);
        assert_eq!(kept, vec![0]);
    }

    #[test]
    fn score_gap_k_max_caps_plateau() {
        let scores = [1.0_f32, 0.99, 0.98, 0.97];
        let kept = keep_evidence_by_gap(&scores, 0.08, 0.90, 2);
        assert_eq!(kept, vec![0, 1]);
    }

    #[test]
    fn seed_entities_dedupes_case_and_trims() {
        let terms = vec![
            "  DRC ".into(),
            "drc".into(),
            "DRO".into(),
            "".into(),
            "  ".into(),
        ];
        let seeds = seed_entities_from_terms(&terms, 8);
        assert_eq!(seeds, vec!["DRC".to_string(), "DRO".to_string()]);
    }

    #[test]
    fn l_eval_rrf_three_merges_channels_and_marks_graph() {
        let dense = vec![ScoredChunk::new_text(
            Uuid::from_u128(1),
            Uuid::from_u128(10),
            "d".into(),
            0.9,
            "dense".into(),
            None,
        )];
        let bm25 = vec![ScoredChunk::new_text(
            Uuid::from_u128(2),
            Uuid::from_u128(10),
            "b".into(),
            0.8,
            "bm25".into(),
            None,
        )];
        let graph = vec![ScoredChunk::new_text(
            Uuid::from_u128(3),
            Uuid::from_u128(10),
            "g".into(),
            0.7,
            "graph".into(),
            None,
        )];
        let (fused, n_g) = l_eval_rrf_three(dense, bm25, graph, 60);
        assert_eq!(n_g, 1);
        assert_eq!(fused.len(), 3);
        assert!(fused.iter().any(|c| c.source == "graph"));
        // All three ranks contribute; scores are RRF not original.
        assert!(fused[0].score > 0.0);
    }

    #[test]
    fn graph_eval_mode_and_stable_uuid() {
        // Without eval env keys, mode is off (clamp path: hops>1 → 1 in from_env).
        // Do not assert false globally: parallel tests may set GRAPH_* briefly.
        let _ = graph_eval_mode_enabled();
        assert_eq!(
            stable_uuid_from_key("relation:a"),
            stable_uuid_from_key("relation:a")
        );
        assert_ne!(
            stable_uuid_from_key("relation:a"),
            stable_uuid_from_key("relation:b")
        );
    }

    #[test]
    fn hops_gt_one_clamped_without_eval_mode() {
        // SAFETY: process-local env for this test; serial with TEST_CONFIG_SERIAL.
        let _guard = TEST_CONFIG_SERIAL.lock().expect("serial");
        // Clear eval flags that would unlock hops>1.
        for k in [
            "GRAPH_EVAL_MODE",
            "GRAPH_L_EVAL_RRF",
            "GRAPH_EVAL_FORCE_REQUIRED_GRAPH",
            "E2E_GRAPH_BASELINE",
            "RETRIEVAL_GRAPH_SEED",
        ] {
            unsafe { std::env::remove_var(k) };
        }
        unsafe { std::env::set_var("GRAPH_AUGMENT_HOPS", "3") };
        let cfg = GraphAugmentConfig::from_env();
        assert_eq!(cfg.hops, 1, "product path must clamp hop>1 without eval flags");
        unsafe { std::env::remove_var("GRAPH_AUGMENT_HOPS") };

        unsafe {
            std::env::set_var("GRAPH_EVAL_MODE", "1");
            std::env::set_var("GRAPH_AUGMENT_HOPS", "3");
        }
        let cfg_eval = GraphAugmentConfig::from_env();
        assert_eq!(cfg_eval.hops, 3, "eval mode unlocks hop=3");
        unsafe {
            std::env::remove_var("GRAPH_EVAL_MODE");
            std::env::remove_var("GRAPH_AUGMENT_HOPS");
        }
    }

    #[test]
    fn graph_evidence_preserves_doc_id_and_source() {
        let doc = Uuid::from_u128(42);
        let ctx = vec![json!({
            "subject": "A",
            "object": "B",
            "evidence_chunks": [{
                "chunk_id": Uuid::from_u128(7).to_string(),
                "doc_id": doc.to_string(),
                "text": "edge body",
                "score": 0.95
            }]
        })];
        let scored = graph_evidence_as_scored_chunks(&ctx);
        assert_eq!(scored.len(), 1);
        assert_eq!(scored[0].doc_id, doc);
        assert_eq!(scored[0].source, "graph");
    }

    #[test]
    fn l_eval_graph_evidence_keeps_support_uuid_rows() {
        // After P0, evidence uses real support chunk UUID + doc_id (not relation:… synthetic).
        let support = Uuid::from_u128(99);
        let doc = Uuid::from_u128(11);
        let ctx = vec![json!({
            "evidence_chunks": [{
                "chunk_id": support.to_string(),
                "doc_id": doc.to_string(),
                "text": "A -位于-> B",
                "score": 1.0
            }]
        })];
        let scored = graph_evidence_as_scored_chunks(&ctx);
        assert_eq!(scored.len(), 1);
        assert_eq!(scored[0].chunk_id, support);
        assert_eq!(scored[0].doc_id, doc);
    }

    #[test]
    fn term_coverage_scores_fraction() {
        let s = evidence_score_term_coverage(
            "DRC maps to DRO in table",
            &["DRC".into(), "DRO".into(), "XYZ".into()],
        );
        assert!((s - 2.0 / 3.0).abs() < 1e-5);
    }

    #[test]
    fn telemetry_result_is_augment_not_explicit() {
        use contracts::ToolStatus;
        let tr = telemetry_tool_result(&[json!({"subject": "A"})], 3);
        assert_eq!(tr.status, ToolStatus::Ok);
        assert!(is_graph_augment_result(&tr));
        assert!(!is_graph_explicit_result(&tr));
        assert!(graph_augment_hit(std::slice::from_ref(&tr)));
        assert!(!graph_explicit_called(std::slice::from_ref(&tr)));
    }

    #[test]
    fn explicit_graph_result_not_counted_as_augment() {
        use contracts::{ToolResult, ToolStatus, ToolTrace};
        let tr = ToolResult {
            tool: "graph_retrieval".to_string(),
            version: "1.0".to_string(),
            status: ToolStatus::Ok,
            data: Some(json!([])),
            trace: Some(ToolTrace {
                elapsed_ms: Some(1),
                raw_hit_count: Some(0),
                hydrated_hit_count: Some(0),
                degrade_reason: None,
            }),
        };
        assert!(is_graph_explicit_result(&tr));
        assert!(!is_graph_augment_result(&tr));
    }
}
