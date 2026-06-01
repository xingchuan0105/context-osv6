//! Objective signal evaluator for ReAct iterations.
//!
//! Per `docs/CHAT_GRAPHFLOW_REMOVAL_AND_AGENT_REACT_2026-05-10.md` §4.2, the
//! evaluator is a pure function: it takes signals + budget + accumulated state
//! and returns an [`EvalAdvice`] describing what the loop should do next.
//!
//! This module deviates intentionally from the literal doc signature in one
//! way: the doc shows `evaluate_*_iteration -> LoopDecision`, but
//! `LoopDecision<P>` is generic over agent-specific params. Returning a
//! schema-agnostic [`EvalAdvice`] keeps the evaluator pure and pushes the
//! "fallback must change input" enforcement (decision ⑦) to the agent's
//! call site, where `LoopDecision::Continue { new_params, .. }` is constructed.
//! The compile-time guarantee is preserved — only the responsibility for
//! constructing the params now lives where the agent state is owned.
//!
//! Thresholds are surfaced as `pub const` so they can be flipped via tests
//! today and via env vars later (Risk R1).

use crate::agents::react_loop::{DegradeReason, LoopBudget};
#[cfg(test)]
use crate::agents::react_loop::UserTier;
use avrag_search::SearchResult;
use common::AnswerContextChunk;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

fn env_f32(key: &str, default: f32) -> f32 {
    std::env::var(key)
        .ok()
        .and_then(|v| v.trim().parse().ok())
        .unwrap_or(default)
}

/// Threshold below which `max_score` is considered a weak retrieval signal.
/// Override via `RAG_MIN_MAX_SCORE` env var (default 0.30).
pub fn rag_min_max_score() -> f32 {
    env_f32("RAG_MIN_MAX_SCORE", 0.30)
}

/// Threshold below which `term_coverage` is considered insufficient.
/// Override via `RAG_MIN_TERM_COVERAGE` env var (default 0.50).
pub fn rag_min_term_coverage() -> f32 {
    env_f32("RAG_MIN_TERM_COVERAGE", 0.50)
}

/// Threshold above which we consider the RAG result set "good enough" to
/// short-circuit further fallbacks even before the budget is exhausted.
/// Override via `RAG_GOOD_MAX_SCORE` env var (default 0.65).
pub fn rag_good_max_score() -> f32 {
    env_f32("RAG_GOOD_MAX_SCORE", 0.65)
}

/// Search-side coverage threshold (slightly looser than RAG since web hits
/// surface lots of off-topic noise).
/// Override via `SEARCH_MIN_TERM_COVERAGE` env var (default 0.40).
pub fn search_min_term_coverage() -> f32 {
    env_f32("SEARCH_MIN_TERM_COVERAGE", 0.40)
}

/// Objective signals computed from a single iteration's results.
///
/// These are the *only* inputs the evaluator considers. Subjective LLM
/// self-grading is intentionally absent (decision: see §1.4 in design doc).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct EvaluationSignals {
    /// Total number of distinct results returned from this iteration.
    pub recall_count: usize,
    /// Highest retrieval score across results (0.0 if no results).
    pub max_score: f32,
    /// Fraction of significant query terms that appear in at least one hit.
    /// Range: 0.0 (no overlap) to 1.0 (every term covered).
    pub term_coverage: f32,
    /// Subqueries that returned zero hits — useful for targeted broaden/replan.
    pub zero_hits_per_subquery: Vec<String>,
}

impl EvaluationSignals {
    /// Compute term coverage from a query and a list of result text snippets.
    /// Lower-cases both sides; counts a term as covered if any snippet contains it.
    /// Filters short stop-tokens (length < 3) so coverage isn't dominated by
    /// articles and conjunctions.
    pub fn compute_term_coverage(query: &str, result_texts: &[&str]) -> f32 {
        let terms: Vec<String> = query
            .split_whitespace()
            .map(|t| t.to_lowercase())
            .filter(|t| t.chars().count() >= 3)
            .collect();
        if terms.is_empty() {
            return 1.0; // degenerate case — treat as fully covered.
        }
        let blob: String = result_texts
            .iter()
            .map(|t| t.to_lowercase())
            .collect::<Vec<_>>()
            .join(" ");
        let covered = terms.iter().filter(|t| blob.contains(t.as_str())).count();
        covered as f32 / terms.len() as f32
    }
}

/// Hint returned by the evaluator. Agents map this to a concrete
/// `LoopDecision<P>` by constructing fresh params.
#[derive(Debug, Clone, PartialEq)]
pub enum EvalAdvice {
    /// Stop iterating; hand off to synthesis with what we have.
    Synthesize,
    /// Stop iterating; ask the user for clarification.
    Clarify { question: String },
    /// Stop iterating; emit a degrade trace and return the best partial answer.
    Degrade { reason: DegradeReason },
    /// Re-run the planner with a fresh prompt — typically when term coverage is low.
    Replan { reason: &'static str },
    /// Reuse plan, broaden the query (drop modifiers, add synonyms, switch to BM25).
    BroadenQuery { reason: &'static str },
    /// Search-only: switch Brave vertical (general → news / discussions).
    EscalateVertical { reason: &'static str },
    /// RAG-only: hand off to web search after local recall stays empty.
    EscalateToSearch { reason: &'static str },
    /// Search-only stub (decision ⑤): fetch full page content; not implemented yet.
    FetchFullPage { reason: &'static str },
}

impl EvalAdvice {
    pub fn is_terminal(&self) -> bool {
        matches!(
            self,
            EvalAdvice::Synthesize | EvalAdvice::Clarify { .. } | EvalAdvice::Degrade { .. }
        )
    }
}

/// Cross-iteration accumulator for RAG: dedupes by (doc_id, chunk_id) and
/// keeps the highest score per chunk (Risk R2).
#[derive(Debug, Clone, Default)]
pub struct AccumulatedRagResults {
    chunks: HashMap<RagChunkKey, ScoredChunk>,
    total_tool_calls: u32,
}

#[derive(Debug, Clone, Hash, PartialEq, Eq)]
pub struct RagChunkKey {
    pub doc_id: Option<String>,
    pub chunk_id: String,
}

#[derive(Debug, Clone)]
pub struct ScoredChunk {
    pub chunk: AnswerContextChunk,
    pub score: f32,
    pub iteration: u8,
}

impl AccumulatedRagResults {
    pub fn new() -> Self {
        Self::default()
    }

    /// Merge a single iteration's `(chunk, score)` pairs into the accumulator.
    /// Keeps the highest score per `(doc_id, chunk_id)` and records which
    /// iteration first contributed each chunk.
    pub fn merge_iteration(
        &mut self,
        results: impl IntoIterator<Item = (AnswerContextChunk, f32)>,
        iteration: u8,
    ) {
        for (chunk, score) in results {
            let key = RagChunkKey {
                doc_id: chunk.doc_id.clone(),
                chunk_id: chunk.chunk_id.clone(),
            };
            match self.chunks.get_mut(&key) {
                Some(existing) if existing.score >= score => {}
                _ => {
                    self.chunks.insert(
                        key,
                        ScoredChunk {
                            chunk,
                            score,
                            iteration,
                        },
                    );
                }
            }
        }
    }

    pub fn record_tool_calls(&mut self, n: u32) {
        self.total_tool_calls = self.total_tool_calls.saturating_add(n);
    }

    pub fn unique_chunk_count(&self) -> usize {
        self.chunks.len()
    }

    pub fn total_tool_calls(&self) -> u32 {
        self.total_tool_calls
    }

    pub fn is_empty(&self) -> bool {
        self.chunks.is_empty()
    }

    /// Drop all accumulated evidence. Use to replace accumulator
    /// contents wholesale (e.g. after focus-mode compression).
    pub fn clear(&mut self) {
        self.chunks.clear();
    }

    pub fn max_score(&self) -> f32 {
        self.chunks
            .values()
            .map(|c| c.score)
            .fold(0.0_f32, f32::max)
    }

    /// Borrow all chunk references currently in the accumulator.
    pub fn all_chunks(&self) -> Vec<&AnswerContextChunk> {
        self.chunks.values().map(|sc| &sc.chunk).collect()
    }

    /// Borrow all scores currently in the accumulator.
    pub fn all_scores(&self) -> Vec<f32> {
        self.chunks.values().map(|sc| sc.score).collect()
    }

    /// Take the top-N scored chunks (sorted desc by score) and consume `self`.
    pub fn into_top_n(self, n: usize) -> Vec<AnswerContextChunk> {
        let mut v: Vec<ScoredChunk> = self.chunks.into_values().collect();
        v.sort_by(|a, b| {
            b.score
                .partial_cmp(&a.score)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        v.into_iter().take(n).map(|sc| sc.chunk).collect()
    }
}

/// Evaluate a RAG iteration — see thresholds above and §4.3 in the design doc.
///
/// Decision flow (in priority order):
/// 1. Budget exhausted + accumulator non-empty → `Synthesize`
/// 2. Budget exhausted + accumulator empty → `Degrade(NoResultsAfterAllFallbacks)`
/// 3. Current iteration returned strong recall (`max_score >= rag_good_max_score()`) → `Synthesize`
/// 4. Current iteration returned zero results AND accumulator empty → `EscalateToSearch`
/// 5. Term coverage low → `Replan`
/// 6. Max score low → `BroadenQuery`
/// 7. Otherwise → `Synthesize`
pub fn evaluate_rag_iteration(
    signals: &EvaluationSignals,
    budget: &LoopBudget,
    accumulated: &AccumulatedRagResults,
) -> EvalAdvice {
    if budget.exhausted() {
        if accumulated.is_empty() {
            return EvalAdvice::Degrade {
                reason: DegradeReason::NoResultsAfterAllFallbacks,
            };
        }
        return EvalAdvice::Synthesize;
    }

    if signals.max_score >= rag_good_max_score() && signals.recall_count > 0 {
        return EvalAdvice::Synthesize;
    }

    if signals.recall_count == 0 && accumulated.is_empty() {
        return EvalAdvice::EscalateToSearch {
            reason: "rag_zero_recall",
        };
    }

    if signals.term_coverage < rag_min_term_coverage() {
        return EvalAdvice::Replan {
            reason: "low_term_coverage",
        };
    }

    if signals.max_score < rag_min_max_score() {
        return EvalAdvice::BroadenQuery {
            reason: "low_max_score",
        };
    }

    EvalAdvice::Synthesize
}

/// Evaluate a Search iteration — see thresholds above and §4.4 in the design doc.
///
/// Decision flow (in priority order):
/// 1. Budget exhausted + last_results non-empty → `Synthesize`
/// 2. Budget exhausted + last_results empty → `Degrade(NoResultsAfterAllFallbacks)`
/// 3. recall_count == 0 → `EscalateVertical` (agent decides whether vertical can still escalate)
/// 4. Term coverage low → `BroadenQuery`
/// 5. Otherwise → `Synthesize`
pub fn evaluate_search_iteration(
    signals: &EvaluationSignals,
    budget: &LoopBudget,
    last_results: &[SearchResult],
) -> EvalAdvice {
    if budget.exhausted() {
        if last_results.is_empty() && signals.recall_count == 0 {
            return EvalAdvice::Degrade {
                reason: DegradeReason::NoResultsAfterAllFallbacks,
            };
        }
        return EvalAdvice::Synthesize;
    }

    if signals.recall_count == 0 {
        return EvalAdvice::EscalateVertical {
            reason: "search_zero_recall",
        };
    }

    if signals.term_coverage < search_min_term_coverage() {
        return EvalAdvice::BroadenQuery {
            reason: "low_term_coverage",
        };
    }

    EvalAdvice::Synthesize
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_chunk(chunk_id: &str, doc_id: Option<&str>, text: &str) -> AnswerContextChunk {
        AnswerContextChunk {
            chunk_id: chunk_id.to_string(),
            doc_id: doc_id.map(|s| s.to_string()),
            chunk_type: "text".to_string(),
            page: None,
            text: text.to_string(),
            asset_id: None,
            caption: None,
            image_url: None,
            parser_backend: None,
            source_locator: None,
        }
    }

    fn make_search_result(title: &str, url: &str, snippet: &str) -> SearchResult {
        SearchResult {
            title: title.to_string(),
            url: url.to_string(),
            snippet: snippet.to_string(),
            citation_index: None,
        }
    }

    // ---------------- EvaluationSignals::compute_term_coverage ----------------

    #[test]
    fn term_coverage_is_one_when_all_terms_present() {
        let cov = EvaluationSignals::compute_term_coverage(
            "rust async runtime",
            &["The rust async runtime is fast"],
        );
        assert!((cov - 1.0).abs() < 1e-6);
    }

    #[test]
    fn term_coverage_filters_short_stop_terms() {
        // "is", "of" are filtered (< 3 chars). Terms left: "the", "lord", "rings" — all present.
        let cov = EvaluationSignals::compute_term_coverage(
            "the lord of is rings",
            &["The Lord of the Rings"],
        );
        assert!((cov - 1.0).abs() < 1e-6);
    }

    #[test]
    fn term_coverage_drops_when_terms_missing() {
        let cov = EvaluationSignals::compute_term_coverage(
            "rust async runtime",
            &["python sync interpreter"],
        );
        assert!((cov - 0.0).abs() < 1e-6);
    }

    #[test]
    fn term_coverage_handles_empty_query() {
        let cov = EvaluationSignals::compute_term_coverage("", &["anything"]);
        assert!((cov - 1.0).abs() < 1e-6);
    }

    // ---------------- AccumulatedRagResults ----------------

    #[test]
    fn accumulator_keeps_higher_score_on_duplicate_chunk() {
        let mut acc = AccumulatedRagResults::new();
        let chunk = make_chunk("c1", Some("d1"), "first");
        acc.merge_iteration(vec![(chunk.clone(), 0.4)], 0);
        acc.merge_iteration(vec![(chunk.clone(), 0.7)], 1);
        acc.merge_iteration(vec![(chunk.clone(), 0.2)], 2);
        assert_eq!(acc.unique_chunk_count(), 1);
        assert!((acc.max_score() - 0.7).abs() < 1e-6);
    }

    #[test]
    fn accumulator_dedupes_across_iterations() {
        let mut acc = AccumulatedRagResults::new();
        acc.merge_iteration(
            vec![
                (make_chunk("c1", Some("d1"), ""), 0.5),
                (make_chunk("c2", Some("d1"), ""), 0.4),
            ],
            0,
        );
        acc.merge_iteration(
            vec![
                (make_chunk("c2", Some("d1"), ""), 0.6),
                (make_chunk("c3", Some("d2"), ""), 0.3),
            ],
            1,
        );
        assert_eq!(acc.unique_chunk_count(), 3);
    }

    #[test]
    fn accumulator_top_n_returns_chunks_sorted_by_score() {
        let mut acc = AccumulatedRagResults::new();
        acc.merge_iteration(
            vec![
                (make_chunk("c1", None, ""), 0.1),
                (make_chunk("c2", None, ""), 0.9),
                (make_chunk("c3", None, ""), 0.5),
            ],
            0,
        );
        let top = acc.into_top_n(2);
        assert_eq!(top.len(), 2);
        assert_eq!(top[0].chunk_id, "c2");
        assert_eq!(top[1].chunk_id, "c3");
    }

    // ---------------- evaluate_rag_iteration ----------------

    #[test]
    fn rag_strong_recall_advises_synthesize() {
        let signals = EvaluationSignals {
            recall_count: 5,
            max_score: 0.8,
            term_coverage: 0.9,
            zero_hits_per_subquery: vec![],
        };
        let budget = LoopBudget::rag(UserTier::Pro);
        let acc = AccumulatedRagResults::new();
        assert_eq!(evaluate_rag_iteration(&signals, &budget, &acc), EvalAdvice::Synthesize);
    }

    #[test]
    fn rag_zero_recall_first_iter_escalates_to_search() {
        let signals = EvaluationSignals {
            recall_count: 0,
            max_score: 0.0,
            term_coverage: 0.0,
            zero_hits_per_subquery: vec!["q1".to_string()],
        };
        let budget = LoopBudget::rag(UserTier::Pro);
        let acc = AccumulatedRagResults::new();
        match evaluate_rag_iteration(&signals, &budget, &acc) {
            EvalAdvice::EscalateToSearch { reason } => assert_eq!(reason, "rag_zero_recall"),
            other => panic!("expected EscalateToSearch, got {other:?}"),
        }
    }

    #[test]
    fn rag_low_term_coverage_replans() {
        let signals = EvaluationSignals {
            recall_count: 3,
            max_score: 0.5,
            term_coverage: 0.30,
            zero_hits_per_subquery: vec![],
        };
        let budget = LoopBudget::rag(UserTier::Pro);
        let mut acc = AccumulatedRagResults::new();
        acc.merge_iteration(vec![(make_chunk("c1", None, ""), 0.5)], 0);
        match evaluate_rag_iteration(&signals, &budget, &acc) {
            EvalAdvice::Replan { reason } => assert_eq!(reason, "low_term_coverage"),
            other => panic!("expected Replan, got {other:?}"),
        }
    }

    #[test]
    fn rag_low_max_score_broadens() {
        let signals = EvaluationSignals {
            recall_count: 3,
            max_score: 0.20,
            term_coverage: 0.8,
            zero_hits_per_subquery: vec![],
        };
        let budget = LoopBudget::rag(UserTier::Pro);
        let mut acc = AccumulatedRagResults::new();
        acc.merge_iteration(vec![(make_chunk("c1", None, ""), 0.20)], 0);
        match evaluate_rag_iteration(&signals, &budget, &acc) {
            EvalAdvice::BroadenQuery { reason } => assert_eq!(reason, "low_max_score"),
            other => panic!("expected BroadenQuery, got {other:?}"),
        }
    }

    #[test]
    fn rag_budget_exhausted_with_results_synthesizes() {
        let signals = EvaluationSignals {
            recall_count: 0,
            max_score: 0.0,
            term_coverage: 0.0,
            zero_hits_per_subquery: vec![],
        };
        let mut budget = LoopBudget::rag(UserTier::Pro);
        // Pro tier RAG budget = 4; tick 4 times to exhaust.
        budget.tick();
        budget.tick();
        budget.tick();
        budget.tick();
        let mut acc = AccumulatedRagResults::new();
        acc.merge_iteration(vec![(make_chunk("c1", None, ""), 0.4)], 0);
        assert_eq!(evaluate_rag_iteration(&signals, &budget, &acc), EvalAdvice::Synthesize);
    }

    #[test]
    fn rag_budget_exhausted_no_results_degrades() {
        let signals = EvaluationSignals {
            recall_count: 0,
            max_score: 0.0,
            term_coverage: 0.0,
            zero_hits_per_subquery: vec![],
        };
        let mut budget = LoopBudget::rag(UserTier::Pro);
        // Pro tier RAG budget = 4; tick 4 times to exhaust.
        budget.tick();
        budget.tick();
        budget.tick();
        budget.tick();
        let acc = AccumulatedRagResults::new();
        match evaluate_rag_iteration(&signals, &budget, &acc) {
            EvalAdvice::Degrade { reason } => match reason {
                DegradeReason::NoResultsAfterAllFallbacks => {}
                other => panic!("expected NoResultsAfterAllFallbacks, got {other:?}"),
            },
            other => panic!("expected Degrade, got {other:?}"),
        }
    }

    // ---------------- evaluate_search_iteration ----------------

    #[test]
    fn search_strong_recall_advises_synthesize() {
        let signals = EvaluationSignals {
            recall_count: 5,
            max_score: 0.0,
            term_coverage: 0.7,
            zero_hits_per_subquery: vec![],
        };
        let budget = LoopBudget::search(UserTier::Pro);
        let results = vec![make_search_result("t", "u", "s")];
        assert_eq!(
            evaluate_search_iteration(&signals, &budget, &results),
            EvalAdvice::Synthesize
        );
    }

    #[test]
    fn search_zero_recall_escalates_vertical() {
        let signals = EvaluationSignals {
            recall_count: 0,
            max_score: 0.0,
            term_coverage: 0.0,
            zero_hits_per_subquery: vec!["q1".to_string()],
        };
        let budget = LoopBudget::search(UserTier::Pro);
        let results: Vec<SearchResult> = vec![];
        match evaluate_search_iteration(&signals, &budget, &results) {
            EvalAdvice::EscalateVertical { reason } => assert_eq!(reason, "search_zero_recall"),
            other => panic!("expected EscalateVertical, got {other:?}"),
        }
    }

    #[test]
    fn search_low_term_coverage_broadens() {
        let signals = EvaluationSignals {
            recall_count: 4,
            max_score: 0.0,
            term_coverage: 0.20,
            zero_hits_per_subquery: vec![],
        };
        let budget = LoopBudget::search(UserTier::Pro);
        let results = vec![make_search_result("t", "u", "s")];
        match evaluate_search_iteration(&signals, &budget, &results) {
            EvalAdvice::BroadenQuery { reason } => assert_eq!(reason, "low_term_coverage"),
            other => panic!("expected BroadenQuery, got {other:?}"),
        }
    }

    #[test]
    fn search_budget_exhausted_no_results_degrades() {
        let signals = EvaluationSignals {
            recall_count: 0,
            max_score: 0.0,
            term_coverage: 0.0,
            zero_hits_per_subquery: vec![],
        };
        let mut budget = LoopBudget::search(UserTier::Pro);
        // Pro tier Search budget = 3; tick 3 times to exhaust.
        budget.tick();
        budget.tick();
        budget.tick();
        let results: Vec<SearchResult> = vec![];
        match evaluate_search_iteration(&signals, &budget, &results) {
            EvalAdvice::Degrade { reason } => match reason {
                DegradeReason::NoResultsAfterAllFallbacks => {}
                other => panic!("expected NoResultsAfterAllFallbacks, got {other:?}"),
            },
            other => panic!("expected Degrade, got {other:?}"),
        }
    }

    #[test]
    fn search_budget_exhausted_with_results_synthesizes() {
        let signals = EvaluationSignals {
            recall_count: 2,
            max_score: 0.0,
            term_coverage: 0.5,
            zero_hits_per_subquery: vec![],
        };
        let mut budget = LoopBudget::search(UserTier::Pro);
        // Pro tier Search budget = 3; tick 3 times to exhaust.
        budget.tick();
        budget.tick();
        budget.tick();
        let results = vec![make_search_result("t", "u", "s")];
        assert_eq!(
            evaluate_search_iteration(&signals, &budget, &results),
            EvalAdvice::Synthesize
        );
    }

    #[test]
    fn eval_advice_is_terminal_classification() {
        assert!(EvalAdvice::Synthesize.is_terminal());
        assert!(EvalAdvice::Clarify {
            question: "q".to_string()
        }
        .is_terminal());
        assert!(EvalAdvice::Degrade {
            reason: DegradeReason::AllToolsFailed
        }
        .is_terminal());
        assert!(!EvalAdvice::Replan { reason: "x" }.is_terminal());
        assert!(!EvalAdvice::BroadenQuery { reason: "x" }.is_terminal());
    }

    // ---------------- env override ----------------

    #[test]
    fn threshold_functions_read_env_with_fallback() {
        // Uses unique env keys so parallel runs of *this test* do not conflict.
        // (Tests in this module do not otherwise touch these keys.)
        unsafe {
            std::env::set_var("TEST_RAG_MIN_MAX_SCORE", "0.99");
        }
        assert!((env_f32("TEST_RAG_MIN_MAX_SCORE", 0.30) - 0.99).abs() < 1e-6);
        unsafe {
            std::env::remove_var("TEST_RAG_MIN_MAX_SCORE");
        }

        // Fallback when env is absent.
        assert!((env_f32("TEST_RAG_MIN_MAX_SCORE_MISSING", 0.30) - 0.30).abs() < 1e-6);

        // Malformed env falls back to default.
        unsafe {
            std::env::set_var("TEST_RAG_BAD", "not_a_float");
        }
        assert!((env_f32("TEST_RAG_BAD", 0.50) - 0.50).abs() < 1e-6);
        unsafe {
            std::env::remove_var("TEST_RAG_BAD");
        }
    }
}
