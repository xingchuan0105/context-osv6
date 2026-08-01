//! Lexical-forced 1-hop graph augment (canonical: 2026-07-23-lexical-graph-augment-scoring-design).
//!
//! - Trigger: only from lexical_search bridge path when `RETRIEVAL_GRAPH_AUGMENT=1`.
//! - Seeds: this-hop **terms** (not full user-query embed / dense ANN).
//! - Hop: forced to 1.
//! - Evidence: score vs terms + TOP1 score-gap cut (得分落差).
//! - Does **not** register as a ToolCatalog tool; does **not** fake graph_retrieval calls.

use avrag_retrieval_data_plane::GraphSearchRequest;
use contracts::auth_runtime::AuthContext;
use serde_json::{Value, json};
use uuid::Uuid;

use crate::RagRuntime;

/// Env-driven config for lexical graph augment.
#[derive(Debug, Clone)]
pub struct GraphAugmentConfig {
    pub enabled: bool,
    pub max_relations: usize,
    pub seed_limit: usize,
    /// Forced augment always clamps to 1 regardless of env.
    pub hops: usize,
    pub margin_abs: f32,
    pub margin_rel: f32,
    pub evidence_max_k: usize,
}

impl Default for GraphAugmentConfig {
    fn default() -> Self {
        Self {
            // Product default on (P2); explicit env 0/false/off disables.
            enabled: true,
            max_relations: 5,
            seed_limit: 8,
            hops: 1,
            margin_abs: 0.08,
            margin_rel: 0.90,
            evidence_max_k: 3,
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
        // Unset → product default on; explicit 0/false/off → off.
        c.enabled = match std::env::var("RETRIEVAL_GRAPH_AUGMENT") {
            Ok(v) => env_truthy_str(&v),
            Err(_) => true,
        };
        if let Some(v) = env_usize("GRAPH_AUGMENT_MAX_RELATIONS") {
            c.max_relations = v.max(1);
        }
        if let Some(v) = env_usize("GRAPH_AUGMENT_SEED_LIMIT") {
            c.seed_limit = v.max(1);
        }
        // Forced path is always 1 hop; read env only for telemetry/docs consistency.
        let _ = env_usize("GRAPH_AUGMENT_HOPS");
        c.hops = 1;
        if let Some(v) = env_f32("GRAPH_EVIDENCE_MARGIN_ABS") {
            c.margin_abs = v.max(0.0);
        }
        if let Some(v) = env_f32("GRAPH_EVIDENCE_MARGIN_REL") {
            c.margin_rel = v.clamp(0.0, 1.0);
        }
        if let Some(v) = env_usize("GRAPH_EVIDENCE_MAX_K") {
            c.evidence_max_k = v.max(1);
        }
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
            query_entity_vectors: Vec::new(), // no dense ANN seed on force path
            hop_limit: 1,
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

    // Map relation_id-as-chunk → content from supporting_chunks (storage uses relation_id as chunk_id).
    let mut chunk_by_id: std::collections::HashMap<Uuid, &avrag_retrieval_data_plane::ScoredChunk> =
        std::collections::HashMap::new();
    for c in &output.supporting_chunks {
        chunk_by_id.insert(c.chunk_id, c);
    }

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

        // Candidate evidence: supporting chunk texts for this path + relation short text.
        let mut candidates: Vec<(String, String, f32)> = Vec::new(); // (chunk_id, text, score)

        // Prefer explicit supporting_chunk_ids when present; fall back to relation-as-chunk.
        if path.supporting_chunk_ids.is_empty() {
            // Find relation chunk by matching content or any supporting chunk with same ends in text.
            let mut found = false;
            for c in &output.supporting_chunks {
                if c.content.contains(&path.subject) || c.content.contains(&path.object) {
                    let s = evidence_score_term_coverage(&c.content, terms);
                    candidates.push((c.chunk_id.to_string(), c.content.clone(), s));
                    found = true;
                    break;
                }
            }
            if !found {
                let s = evidence_score_term_coverage(&relation_text, terms);
                candidates.push((
                    format!("relation:{}", relation_text),
                    relation_text.clone(),
                    s,
                ));
            }
        } else {
            for id in &path.supporting_chunk_ids {
                if let Some(c) = chunk_by_id.get(id) {
                    let s = evidence_score_term_coverage(&c.content, terms);
                    candidates.push((c.chunk_id.to_string(), c.content.clone(), s));
                }
            }
            if candidates.is_empty() {
                let s = evidence_score_term_coverage(&relation_text, terms);
                candidates.push((
                    format!("relation:{}", relation_text),
                    relation_text.clone(),
                    s,
                ));
            }
        }

        candidates.sort_by(|a, b| b.2.partial_cmp(&a.2).unwrap_or(std::cmp::Ordering::Equal));
        let scores: Vec<f32> = candidates.iter().map(|c| c.2).collect();
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
                let (cid, text, s) = &candidates[i];
                let gap = s1 - *s;
                let reason = if rank == 0 { "top1" } else { "within_margin" };
                json!({
                    "chunk_id": cid,
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
            "relation_text": relation_text,
            "hop": 1,
            "seed_terms_hit": hits,
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
