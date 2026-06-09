//! Evidence Gate — online evidence quality gate for RAG / WebSearch.
//!
//! The Evidence Gate is a **pure-code** (no LLM) decision layer that
//! inspects retrieval metadata after a search/retrieval step and
//! decides whether the collected evidence is sufficient to enter
//! grounded answer generation.
//!
//! It is the online replacement for the legacy LLM-based
//! "evaluate" state, which used to read the same chunks twice
//! (once for sufficiency judgment, once for answer synthesis).
//!
//! # Outcomes
//!
//! - [`EvidenceGateOutcome::Pass`] — evidence is sufficient, proceed
//!   directly to grounded answer synthesis.
//! - [`EvidenceGateOutcome::NeedsFocus`] — recall is too broad or
//!   scores are too diffuse; the caller should run focus-mode
//!   compression before grounded answer.
//! - [`EvidenceGateOutcome::Degrade`] — evidence is insufficient;
//!   the caller should emit a `Degraded` final decision with the
//!   reason kind carried in [`DegradeKind`].
//!
//! # Layering
//!
//! `evidence_gate` lives in `avrag-rag-core` and stays free of
//! `avrag-app` types (`DegradeReason`, `FinalDecision`, etc.).
//! Callers map [`DegradeKind`] into their own strongly-typed
//! `DegradeReason` variant at the strategy layer.

use serde::{Deserialize, Serialize};

/// Lightweight degrade reason emitted by the Evidence Gate.
///
/// Stays decoupled from `avrag_app::agents::react_loop::DegradeReason`
/// so that the gate itself remains strategy-agnostic. The
/// `RagStrategy` / `SearchStrategy` call sites perform the mapping
/// to their respective strong enums.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "kind", content = "detail")]
pub enum DegradeKind {
    /// Retrieval returned zero results.
    NoResults,
    /// Context budget usage is above the configured ceiling.
    ContextBudgetTight,
    /// Top score is below the configured minimum relevance floor.
    LowRelevance,
    /// Document metadata themes do not overlap with the query themes.
    TopicMismatch,
}

impl DegradeKind {
    /// Stable identifier used in activity events and tests.
    pub fn as_str(&self) -> &'static str {
        match self {
            DegradeKind::NoResults => "no_results",
            DegradeKind::ContextBudgetTight => "context_budget_tight",
            DegradeKind::LowRelevance => "low_relevance",
            DegradeKind::TopicMismatch => "topic_mismatch",
        }
    }
}

/// Decision produced by the Evidence Gate.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "decision", content = "reason")]
pub enum EvidenceGateOutcome {
    /// Evidence is sufficient — proceed to grounded answer.
    Pass,
    /// Recall is too broad / scores too diffuse — caller should
    /// invoke focus-mode compression before grounded answer.
    NeedsFocus {
        /// Original chunk count before compression.
        chunk_count: usize,
        /// Score variance observed (higher = more diffuse).
        score_variance: f32,
    },
    /// Evidence is insufficient — caller should emit `Degraded`.
    Degrade(DegradeKind),
}

/// Input to the Evidence Gate — flat, no LLM-derived fields.
#[derive(Debug, Clone)]
pub struct EvidenceGateInput {
    /// Number of chunks returned by retrieval.
    pub chunk_count: usize,
    /// Highest score across all chunks, normalized to `[0.0, 1.0]`.
    pub top_score: f32,
    /// Variance of the score distribution. `0.0` means all chunks
    /// have the same score; higher values mean the distribution is
    /// more spread out.
    pub score_variance: f32,
    /// Ratio of context budget already used, in `[0.0, 1.0]`.
    pub context_usage_ratio: f32,
    /// Document metadata themes (e.g. `["烘焙", "面团"]`).
    pub doc_metadata_themes: Vec<String>,
    /// Query themes / keywords (e.g. `["量子物理", "量子纠缠"]`).
    pub query_themes: Vec<String>,
}

/// Configurable thresholds for the default gate implementation.
#[derive(Debug, Clone)]
pub struct EvidenceGateConfig {
    /// Minimum recall count to consider the search "non-empty".
    pub min_chunk_count: usize,
    /// Minimum top score to consider results relevant.
    pub min_top_score: f32,
    /// Chunk count above which focus-mode compression is desirable.
    pub focus_chunk_threshold: usize,
    /// Score variance above which focus-mode compression is desirable.
    pub focus_score_variance: f32,
    /// Maximum context usage ratio (above this, force degrade).
    pub max_context_usage: f32,
    /// Minimum theme overlap ratio to consider the corpus
    /// topically aligned with the query. `0.0` disables the check.
    pub min_theme_overlap: f32,
}

impl Default for EvidenceGateConfig {
    fn default() -> Self {
        Self {
            min_chunk_count: 1,
            min_top_score: 0.30,
            focus_chunk_threshold: 20,
            focus_score_variance: 0.05,
            max_context_usage: 0.80,
            min_theme_overlap: 0.0, // disabled by default
        }
    }
}

/// Strategy-agnostic evidence quality gate.
pub trait EvidenceGate: Send + Sync {
    fn check(&self, input: &EvidenceGateInput) -> EvidenceGateOutcome;
}

/// Default, threshold-based gate implementation.
pub struct DefaultEvidenceGate {
    pub config: EvidenceGateConfig,
}

impl DefaultEvidenceGate {
    pub fn new(config: EvidenceGateConfig) -> Self {
        Self { config }
    }
}

impl Default for DefaultEvidenceGate {
    fn default() -> Self {
        Self::new(EvidenceGateConfig::default())
    }
}

impl EvidenceGate for DefaultEvidenceGate {
    fn check(&self, input: &EvidenceGateInput) -> EvidenceGateOutcome {
        let c = &self.config;

        // 1. Zero recall → degrade immediately.
        if input.chunk_count < c.min_chunk_count {
            return EvidenceGateOutcome::Degrade(DegradeKind::NoResults);
        }

        // 2. Context budget tight → degrade before answer.
        if input.context_usage_ratio > c.max_context_usage {
            return EvidenceGateOutcome::Degrade(DegradeKind::ContextBudgetTight);
        }

        // 3. Top score below floor → degrade on relevance grounds.
        if input.top_score < c.min_top_score {
            return EvidenceGateOutcome::Degrade(DegradeKind::LowRelevance);
        }

        // 4. Topic mismatch (only when min_theme_overlap > 0 and themes are provided).
        if c.min_theme_overlap > 0.0
            && !input.doc_metadata_themes.is_empty()
            && !input.query_themes.is_empty()
        {
            let overlap = theme_overlap_ratio(&input.query_themes, &input.doc_metadata_themes);
            if overlap < c.min_theme_overlap {
                return EvidenceGateOutcome::Degrade(DegradeKind::TopicMismatch);
            }
        }

        // 5. Recall is broad OR scores are diffuse → focus mode.
        if input.chunk_count > c.focus_chunk_threshold
            || input.score_variance > c.focus_score_variance
        {
            return EvidenceGateOutcome::NeedsFocus {
                chunk_count: input.chunk_count,
                score_variance: input.score_variance,
            };
        }

        EvidenceGateOutcome::Pass
    }
}

/// Jaccard-like overlap on tokenized theme strings (case-insensitive,
/// whole-word match). Returns a value in `[0.0, 1.0]`.
fn theme_overlap_ratio(query: &[String], doc: &[String]) -> f32 {
    if query.is_empty() || doc.is_empty() {
        return 0.0;
    }
    let q: std::collections::HashSet<String> = query
        .iter()
        .map(|s| s.to_lowercase())
        .flat_map(|lower| {
            lower
                .split_whitespace()
                .map(str::to_owned)
                .collect::<Vec<_>>()
        })
        .collect();
    let d: std::collections::HashSet<String> = doc
        .iter()
        .map(|s| s.to_lowercase())
        .flat_map(|lower| {
            lower
                .split_whitespace()
                .map(str::to_owned)
                .collect::<Vec<_>>()
        })
        .collect();
    let intersection = q.intersection(&d).count() as f32;
    let union = q.union(&d).count() as f32;
    if union == 0.0 {
        0.0
    } else {
        intersection / union
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn input(count: usize, top: f32, var: f32, ctx: f32) -> EvidenceGateInput {
        EvidenceGateInput {
            chunk_count: count,
            top_score: top,
            score_variance: var,
            context_usage_ratio: ctx,
            doc_metadata_themes: vec![],
            query_themes: vec![],
        }
    }

    #[test]
    fn empty_recall_degrades() {
        let gate = DefaultEvidenceGate::default();
        assert_eq!(
            gate.check(&input(0, 0.0, 0.0, 0.5)),
            EvidenceGateOutcome::Degrade(DegradeKind::NoResults)
        );
    }

    #[test]
    fn context_budget_tight_degrades() {
        let gate = DefaultEvidenceGate::default();
        assert_eq!(
            gate.check(&input(5, 0.8, 0.01, 0.95)),
            EvidenceGateOutcome::Degrade(DegradeKind::ContextBudgetTight)
        );
    }

    #[test]
    fn low_top_score_degrades() {
        let gate = DefaultEvidenceGate::default();
        assert_eq!(
            gate.check(&input(5, 0.10, 0.01, 0.5)),
            EvidenceGateOutcome::Degrade(DegradeKind::LowRelevance)
        );
    }

    #[test]
    fn high_recall_triggers_focus() {
        let gate = DefaultEvidenceGate::default();
        match gate.check(&input(50, 0.8, 0.01, 0.5)) {
            EvidenceGateOutcome::NeedsFocus { chunk_count, .. } => {
                assert_eq!(chunk_count, 50);
            }
            other => panic!("expected NeedsFocus, got {:?}", other),
        }
    }

    #[test]
    fn diffuse_scores_trigger_focus() {
        let gate = DefaultEvidenceGate::default();
        assert!(matches!(
            gate.check(&input(5, 0.8, 0.5, 0.5)),
            EvidenceGateOutcome::NeedsFocus { .. }
        ));
    }

    #[test]
    fn good_recall_passes() {
        let gate = DefaultEvidenceGate::default();
        assert_eq!(
            gate.check(&input(10, 0.8, 0.02, 0.5)),
            EvidenceGateOutcome::Pass
        );
    }

    #[test]
    fn topic_mismatch_degrades_when_configured() {
        let gate = DefaultEvidenceGate::new(EvidenceGateConfig {
            min_theme_overlap: 0.10,
            ..EvidenceGateConfig::default()
        });
        let mut inp = input(10, 0.8, 0.02, 0.5);
        inp.doc_metadata_themes = vec!["sourdough".into(), "baking".into()];
        inp.query_themes = vec!["quantum".into(), "entanglement".into()];
        assert_eq!(
            gate.check(&inp),
            EvidenceGateOutcome::Degrade(DegradeKind::TopicMismatch)
        );
    }

    #[test]
    fn theme_overlap_computes_jaccard() {
        // query: {quantum, physics}, doc: {quantum, physics, relativity}
        // intersection = 2, union = 3 → jaccard = 2/3
        let r = theme_overlap_ratio(
            &["quantum physics".into()],
            &["quantum physics".into(), "relativity".into()],
        );
        assert!((r - 2.0 / 3.0).abs() < 1e-6);
    }
}
