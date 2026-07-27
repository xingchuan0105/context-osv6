//! RAG Eval v2 (ADR-0012) — judge-first generation-layer evaluation.
//!
//! Slices so far: module skeleton + label/score types + judge ENV client +
//! judge output parsing + judge-input artifact builder + trimmed deterministic
//! retrieval/selection math (P0); versioned judge prompt + score-driven
//! labeling + suite aggregation (P1). Later slices: runner integration (P2),
//! report/cache (P3).

pub mod aggregate;
pub mod artifact;
pub mod cache;
pub mod judge_client;
pub mod judge_parse;
pub mod judge_prompt;
pub mod report;
pub mod retrieval;

pub use aggregate::{LabelInput, SuiteSummaryV2, SubsetSummaryV2, derived_refusal_correct, label_for};
pub use artifact::{ContextSource, JudgeInput};
pub use cache::JudgeCache;
pub use judge_client::{DEFAULT_JUDGE_MODEL, JUDGE_TEMPERATURE, JudgeClient, JudgeConfig};
pub use judge_parse::{
    CorrectnessJudgment, CorrectnessVerdict, FaithfulnessJudgment, FaithfulnessVerdict,
    JudgeOutput, JudgeParseError, RelevancyJudgment, RefusalJudgment, SufficiencyJudgment,
    SufficiencyVerdict, parse_judge_output,
};
pub use judge_prompt::{SCHEMA_VERSION, SYSTEM_PROMPT, build_user_prompt};
pub use report::{render_per_query_tsv, render_summary_md};
pub use retrieval::{RetrievalScoreV2, SelectionScoreV2, score_retrieval, score_selection};

use serde::{Deserialize, Serialize};

/// One-word root-cause label for a v2-scored query (design §5).
///
/// Declaration order is the attribution priority (`InfraError` first, `Pass`
/// last). The score-threshold mapping that derives a label lands with
/// aggregation in a later slice; P0 only defines the type.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum LabelV2 {
    InfraError,
    JudgeError,
    RetrievalMiss,
    SelectionMiss,
    RefusalWrong,
    Ungrounded,
    Incorrect,
    Partial,
    Pass,
}

impl LabelV2 {
    pub fn as_str(self) -> &'static str {
        match self {
            LabelV2::InfraError => "INFRA_ERROR",
            LabelV2::JudgeError => "JUDGE_ERROR",
            LabelV2::RetrievalMiss => "RETRIEVAL_MISS",
            LabelV2::SelectionMiss => "SELECTION_MISS",
            LabelV2::RefusalWrong => "REFUSAL_WRONG",
            LabelV2::Ungrounded => "UNGROUNDED",
            LabelV2::Incorrect => "INCORRECT",
            LabelV2::Partial => "PARTIAL",
            LabelV2::Pass => "PASS",
        }
    }
}

/// Whether the judge call + parse succeeded for a query (design §4.3). An
/// `Error` here maps to `LabelV2::JudgeError` upstream; the query must not
/// auto-PASS.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum JudgeStatus {
    Ok,
    Error,
}

/// Per-query v2 result holder (design §5/§7): Layer A deterministic scores
/// plus the Layer B judge outcome and the derived label.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScoreV2 {
    pub query: String,
    pub subset: String,
    pub retrieval: RetrievalScoreV2,
    pub selection: SelectionScoreV2,
    /// Parsed judge output; `None` when the judge was not called or failed.
    pub judge: Option<JudgeOutput>,
    pub judge_status: JudgeStatus,
    pub label: LabelV2,
    /// Golden reference answer (rubric) shown in the human-readable report
    /// (design §15). `None` for infra-failed questions.
    #[serde(default)]
    pub reference_answer: Option<String>,
    /// The model answer that was judged. `None` for infra-failed questions.
    #[serde(default)]
    pub model_answer: Option<String>,
    /// Where the judge's grounding context came from. `NoContext` marks
    /// non-RAG questions whose faithfulness must not be scored or averaged.
    /// Defaults to `Cited` so pre-field artifacts still deserialize.
    #[serde(default)]
    pub context_source: ContextSource,
}

/// Judge label thresholds (design §5 initial values). Report-only until
/// calibrated (Phase 0: no hard gate); P0 does not derive labels from them.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct JudgeThresholds {
    pub tau_correctness: f64,
    pub tau_faithfulness: f64,
    pub partial_min: f64,
}

impl Default for JudgeThresholds {
    fn default() -> Self {
        Self {
            tau_correctness: 0.7,
            tau_faithfulness: 0.7,
            partial_min: 0.4,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn label_v2_serde_names_match_design() {
        let expect = [
            (LabelV2::InfraError, "INFRA_ERROR"),
            (LabelV2::JudgeError, "JUDGE_ERROR"),
            (LabelV2::RetrievalMiss, "RETRIEVAL_MISS"),
            (LabelV2::SelectionMiss, "SELECTION_MISS"),
            (LabelV2::RefusalWrong, "REFUSAL_WRONG"),
            (LabelV2::Ungrounded, "UNGROUNDED"),
            (LabelV2::Incorrect, "INCORRECT"),
            (LabelV2::Partial, "PARTIAL"),
            (LabelV2::Pass, "PASS"),
        ];
        for (label, name) in expect {
            assert_eq!(label.as_str(), name);
            // serde roundtrip keeps the same SCREAMING_SNAKE name.
            let json = serde_json::to_string(&label).unwrap();
            assert_eq!(json, format!("\"{name}\""));
            let back: LabelV2 = serde_json::from_str(&json).unwrap();
            assert_eq!(back, label);
        }
    }

    #[test]
    fn judge_thresholds_default_to_design_initial_values() {
        let t = JudgeThresholds::default();
        assert_eq!(t.tau_correctness, 0.7);
        assert_eq!(t.tau_faithfulness, 0.7);
        assert_eq!(t.partial_min, 0.4);
    }
}
