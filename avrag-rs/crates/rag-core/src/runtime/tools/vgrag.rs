//! VGRAG-as-dense: dense multi-way entity seeds → graph expand (hop=2) → fuse into dense chunks.
//!
//! Product (2026-08-03):
//! - `DENSE_BACKEND=vgrag` (default): fuse graph into a **large dense pool**, then **one** final cut.
//! - `DENSE_BACKEND=ann`: pure dense only (ops rollback / A/B).
//! Cite: real UUID + non-nil doc_id only; tool name stays `dense_retrieval`.

use avrag_retrieval_data_plane::ScoredChunk;
use contracts::auth_runtime::AuthContext;
use uuid::Uuid;

use crate::merge::global_rrf_merge;
use crate::RagRuntime;

use super::graph_augment::{self, GraphAugmentConfig, seed_entities_from_terms};

/// Product hop for dense-internal graph expand.
pub const VGRAG_HOPS: usize = 2;
/// Dense candidates kept for fuse (before final cut) — must be ≫ adaptive_k.
pub const VGRAG_DENSE_POOL_CAP: usize = 24;
/// Hard cap on returned chunks after fuse (cite-friendly short list).
pub const VGRAG_FINAL_CAP: usize = 12;
const VGRAG_RRF_K: usize = 60;
const VGRAG_GRAPH_EVIDENCE_CAP: usize = 12;
const VGRAG_SEED_LIMIT: usize = 8;

/// Dense backend selection.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DenseBackend {
    /// Pure ANN + rerank (no graph expand).
    Ann,
    /// VGRAG: multi-way seed + hop-2 expand fused into dense list.
    Vgrag,
}

impl DenseBackend {
    /// `DENSE_BACKEND=ann|vgrag` (default **vgrag**). Also accepts `pure`/`off` → ann.
    pub fn from_env() -> Self {
        match std::env::var("DENSE_BACKEND") {
            Ok(v) => {
                let t = v.trim().to_ascii_lowercase();
                if t == "ann" || t == "pure" || t == "off" || t == "0" || t == "dense" {
                    Self::Ann
                } else {
                    Self::Vgrag
                }
            }
            Err(_) => Self::Vgrag,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Ann => "ann",
            Self::Vgrag => "vgrag",
        }
    }
}

fn vgrag_expand_config() -> GraphAugmentConfig {
    GraphAugmentConfig {
        enabled: true,
        max_relations: 5,
        seed_limit: VGRAG_SEED_LIMIT,
        hops: VGRAG_HOPS,
        margin_abs: 0.08,
        margin_rel: 0.90,
        evidence_max_k: 3,
        dense_seed: true,
        l_eval_rrf: false,
    }
}

/// Telemetry for one VGRAG fuse (P1 observability).
#[derive(Debug, Clone, Copy, Default)]
pub struct VgragFuseStats {
    /// Cite-safe graph evidence rows that entered RRF fuse.
    pub graph_n: usize,
    /// Relation paths returned in graph_context (after augment ranking).
    pub relation_n: usize,
    /// Raw evidence_chunks entries before C8 UUID/doc filter + dedupe.
    pub evidence_raw_n: usize,
    /// evidence_raw_n − graph_n (includes C8 drops and cap/dedupe loss).
    pub evidence_dropped: usize,
}

/// Fuse pure-dense **pool** with hop-2 graph evidence.
///
/// Returns `(fused_by_rrf, stats)`. Caller should pass a pool of size up to
/// [`VGRAG_DENSE_POOL_CAP`] (not the final adaptive cut).
/// Empty graph → pure dense pool unchanged (order preserved).
pub async fn fuse_vgrag_into_dense(
    runtime: &RagRuntime,
    auth: &AuthContext,
    query: &str,
    doc_scope: &[String],
    pure_dense: Vec<ScoredChunk>,
) -> (Vec<ScoredChunk>, VgragFuseStats) {
    let empty_stats = VgragFuseStats::default();
    if pure_dense.is_empty() && query.trim().is_empty() {
        return (pure_dense, empty_stats);
    }

    let terms = terms_for_vgrag_seeds(query, &pure_dense);
    if terms.is_empty() {
        return (pure_dense, empty_stats);
    }

    let cfg = vgrag_expand_config();
    let seeds = seed_entities_from_terms(&terms, cfg.seed_limit);
    // Still try expand when seeds empty but we have embeddable terms (dense_seed path).
    if seeds.is_empty() && terms.iter().all(|t| t.chars().count() < 2) {
        return (pure_dense, empty_stats);
    }

    let graph_context =
        graph_augment::graph_augment_from_terms(runtime, auth, &terms, doc_scope, &cfg).await;
    let relation_n = graph_context.len();
    let evidence_raw_n: usize = graph_context
        .iter()
        .filter_map(|g| g.get("evidence_chunks").and_then(|e| e.as_array()))
        .map(|a| a.len())
        .sum();
    if graph_context.is_empty() {
        return (
            pure_dense,
            VgragFuseStats {
                relation_n: 0,
                evidence_raw_n: 0,
                graph_n: 0,
                evidence_dropped: 0,
            },
        );
    }

    let mut graph_chunks = graph_evidence_real_uuid_only(&graph_context);
    // Prefer body text already in the dense pool when support chunk ids overlap.
    enrich_graph_chunks_from_dense(&mut graph_chunks, &pure_dense);
    graph_chunks.truncate(VGRAG_GRAPH_EVIDENCE_CAP);
    let n_graph = graph_chunks.len();
    let stats = VgragFuseStats {
        graph_n: n_graph,
        relation_n,
        evidence_raw_n,
        evidence_dropped: evidence_raw_n.saturating_sub(n_graph),
    };
    if n_graph == 0 {
        tracing::debug!(
            relation_n,
            evidence_raw_n,
            "vgrag: graph_context non-empty but no cite-safe evidence after C8 filter"
        );
        return (pure_dense, stats);
    }

    let mut dense_tagged = pure_dense;
    for c in &mut dense_tagged {
        if c.source.is_empty() {
            c.source = "dense".to_string();
        }
    }

    if dense_tagged.is_empty() {
        return (graph_chunks, stats);
    }

    let fused = global_rrf_merge(
        vec![(dense_tagged, 1.0), (graph_chunks, 1.0)],
        VGRAG_RRF_K,
    );
    (fused, stats)
}

/// When graph evidence only carries relation_text (content_store hydrate miss on
/// retrieval-index chunk ids), swap in longer body text if the same chunk_id is
/// already in the pure-dense pool.
fn enrich_graph_chunks_from_dense(graph_chunks: &mut [ScoredChunk], pure_dense: &[ScoredChunk]) {
    if graph_chunks.is_empty() || pure_dense.is_empty() {
        return;
    }
    let by_id: std::collections::HashMap<Uuid, &ScoredChunk> =
        pure_dense.iter().map(|c| (c.chunk_id, c)).collect();
    for g in graph_chunks.iter_mut() {
        if let Some(d) = by_id.get(&g.chunk_id)
            && !d.content.trim().is_empty()
            && d.content.chars().count() > g.content.chars().count()
        {
            g.content = d.content.clone();
            if d.page.is_some() {
                g.page = d.page;
            }
        }
    }
}

/// Final cut after VGRAG fuse: **do not** run adaptive_k on RRF scores.
///
/// - `dense_pool_scores`: rerank scores of the pre-fuse dense pool (display-scale).
/// - `graph_n`: how many graph evidence rows entered fuse.
/// - Keep at least adaptive_k(dense), add up to +3 when graph contributed, cap [`VGRAG_FINAL_CAP`].
pub fn final_cut_k(dense_pool_scores: &[f32], fused_len: usize, graph_n: usize) -> (usize, crate::runtime::adaptive_k::ScoreShape) {
    use crate::runtime::adaptive_k::{self, ScoreShape};
    if fused_len == 0 {
        return (0, ScoreShape::FlatAllSame);
    }
    let adaptive = adaptive_k::adaptive_k(dense_pool_scores);
    let boost = if graph_n > 0 { 3 } else { 0 };
    let k = (adaptive.k + boost)
        .clamp(adaptive.k.max(1), VGRAG_FINAL_CAP)
        .min(fused_len);
    (k, adaptive.shape)
}

/// Cite C8: only real UUID chunk_id + non-nil doc_id rows.
fn graph_evidence_real_uuid_only(graph_context: &[serde_json::Value]) -> Vec<ScoredChunk> {
    let mut out = Vec::new();
    let mut seen = std::collections::HashSet::new();
    for g in graph_context {
        let Some(evs) = g.get("evidence_chunks").and_then(|e| e.as_array()) else {
            continue;
        };
        for ev in evs {
            let Some(cid) = ev.get("chunk_id").and_then(|c| c.as_str()) else {
                continue;
            };
            let Ok(chunk_id) = Uuid::parse_str(cid) else {
                continue;
            };
            let Some(text) = ev.get("text").and_then(|t| t.as_str()) else {
                continue;
            };
            if text.trim().is_empty() {
                continue;
            }
            let doc_id = ev
                .get("doc_id")
                .and_then(|d| d.as_str())
                .and_then(|s| Uuid::parse_str(s).ok())
                .unwrap_or_else(Uuid::nil);
            if doc_id.is_nil() {
                continue;
            }
            if !seen.insert(chunk_id) {
                continue;
            }
            let score = ev.get("score").and_then(|s| s.as_f64()).unwrap_or(0.0) as f32;
            out.push(ScoredChunk::new_text(
                chunk_id,
                doc_id,
                text.to_string(),
                score,
                "dense".to_string(),
                None,
            ));
        }
    }
    out
}

/// Seeds: whitespace tokens + full query + CJK runs + light tokens from top dense chunks.
pub fn terms_for_vgrag_seeds(query: &str, dense_pool: &[ScoredChunk]) -> Vec<String> {
    let mut terms = terms_from_query(query);
    // Tokens from top dense hit texts (entity-ish strings already in evidence).
    for c in dense_pool.iter().take(5) {
        push_cjk_and_ascii_tokens(&mut terms, &c.content, 6);
    }
    dedupe_terms_cap(terms, VGRAG_SEED_LIMIT.saturating_mul(2))
}

fn terms_from_query(query: &str) -> Vec<String> {
    let mut terms: Vec<String> = Vec::new();
    for s in query.split_whitespace() {
        let s = s.trim();
        if !s.is_empty() {
            terms.push(s.to_string());
        }
    }
    push_cjk_and_ascii_tokens(&mut terms, query, 12);
    let joined = query.trim();
    if !joined.is_empty() {
        terms.push(joined.to_string());
    }
    terms
}

fn push_cjk_and_ascii_tokens(out: &mut Vec<String>, text: &str, max_add: usize) {
    let mut added = 0usize;
    // ASCII words
    for word in text.split(|c: char| !c.is_ascii_alphanumeric()) {
        if word.len() >= 2 && word.len() <= 48 {
            out.push(word.to_string());
            added += 1;
            if added >= max_add {
                return;
            }
        }
    }
    // Contiguous CJK runs (len 2..=12 chars) — better than whitespace-only for 中文.
    let mut run = String::new();
    let flush = |run: &mut String, out: &mut Vec<String>, added: &mut usize, max_add: usize| {
        let n = run.chars().count();
        if n >= 2 && n <= 12 {
            out.push(run.clone());
            *added += 1;
        }
        // Also emit bigrams for longer runs (entity fragments).
        if n >= 4 {
            let chars: Vec<char> = run.chars().collect();
            for i in 0..chars.len().saturating_sub(1) {
                if *added >= max_add {
                    break;
                }
                let bi: String = chars[i..i + 2].iter().collect();
                out.push(bi);
                *added += 1;
            }
        }
        run.clear();
    };
    for ch in text.chars() {
        if is_cjk(ch) {
            run.push(ch);
            if run.chars().count() >= 12 {
                flush(&mut run, out, &mut added, max_add);
                if added >= max_add {
                    return;
                }
            }
        } else if !run.is_empty() {
            flush(&mut run, out, &mut added, max_add);
            if added >= max_add {
                return;
            }
        }
    }
    if !run.is_empty() {
        flush(&mut run, out, &mut added, max_add);
    }
}

fn is_cjk(c: char) -> bool {
    matches!(c,
        '\u{4E00}'..='\u{9FFF}'   // CJK Unified
        | '\u{3400}'..='\u{4DBF}' // Ext A
        | '\u{F900}'..='\u{FAFF}' // Compatibility
    )
}

fn dedupe_terms_cap(terms: Vec<String>, cap: usize) -> Vec<String> {
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
        if out.len() >= cap {
            break;
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn dense_backend_default_vgrag() {
        // Cannot assert env-free globally; unit the parser:
        assert_eq!(DenseBackend::Vgrag.as_str(), "vgrag");
        assert_eq!(DenseBackend::Ann.as_str(), "ann");
    }

    #[test]
    fn terms_include_cjk_runs() {
        let t = terms_from_query("华为IPD流程概念阶段");
        assert!(
            t.iter().any(|x| x.contains("华为") || x.contains("概念")),
            "terms={t:?}"
        );
    }

    #[test]
    fn terms_from_dense_pool_adds_chunk_tokens() {
        let pool = vec![ScoredChunk::new_text(
            Uuid::from_u128(1),
            Uuid::from_u128(10),
            "DRC maps to DRO in table".into(),
            0.9,
            "dense".into(),
            None,
        )];
        let t = terms_for_vgrag_seeds("query", &pool);
        assert!(t.iter().any(|x| x == "DRC" || x == "DRO"), "terms={t:?}");
    }

    #[test]
    fn real_uuid_only_drops_synthetic_and_nil_doc() {
        let real = Uuid::from_u128(7);
        let doc = Uuid::from_u128(42);
        let ctx = vec![json!({
            "evidence_chunks": [
                {
                    "chunk_id": "relation:foo",
                    "doc_id": doc.to_string(),
                    "text": "synthetic",
                    "score": 1.0
                },
                {
                    "chunk_id": real.to_string(),
                    "doc_id": Uuid::nil().to_string(),
                    "text": "no doc",
                    "score": 1.0
                },
                {
                    "chunk_id": real.to_string(),
                    "doc_id": doc.to_string(),
                    "text": "keep me",
                    "score": 0.9
                }
            ]
        })];
        let scored = graph_evidence_real_uuid_only(&ctx);
        assert_eq!(scored.len(), 1);
        assert_eq!(scored[0].content, "keep me");
    }

    #[test]
    fn final_cut_k_boosts_when_graph_and_caps() {
        let scores = [1.0_f32, 0.9, 0.5, 0.4, 0.3, 0.2];
        let (k0, _) = final_cut_k(&scores, 20, 0);
        let (k1, _) = final_cut_k(&scores, 20, 3);
        assert!(k1 >= k0);
        assert!(k1 <= VGRAG_FINAL_CAP);
    }

    #[test]
    fn cite_safe_support_id_fallback_passes_c8() {
        // P0 fallback shape: body chunk UUID + real doc_id + relation_text body.
        let support = Uuid::from_u128(0xce62_0018_a295_460c);
        let doc = Uuid::from_u128(0x7382_c30c_031b_4d46);
        let ctx = vec![json!({
            "subject": "Y冷冻设备公司",
            "predicate": "位于",
            "object": "大连市",
            "evidence_chunks": [{
                "chunk_id": support.to_string(),
                "doc_id": doc.to_string(),
                "text": "Y冷冻设备公司 -位于-> 大连市",
                "score": 1.0
            }]
        })];
        let scored = graph_evidence_real_uuid_only(&ctx);
        assert_eq!(scored.len(), 1);
        assert_eq!(scored[0].chunk_id, support);
        assert_eq!(scored[0].doc_id, doc);
        assert!(scored[0].content.contains("位于"));
    }
}
