//! Core evaluation types and metric definitions.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// A single evaluation run against a dataset or a single case.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvalRun {
    pub run_id: String,
    pub run_name: String,
    /// Mode under evaluation (e.g. "chat", "rag").
    pub strategy: String,
    /// Version of the agent mode code.
    pub strategy_version: String,
    /// Timestamp when the run started (Unix millis).
    pub started_at_ms: u64,
    /// Timestamp when the run completed (Unix millis).
    pub completed_at_ms: Option<u64>,
    /// Individual case results.
    pub cases: Vec<EvalCase>,
    /// Aggregated scores across all cases.
    pub summary: Option<EvalSummary>,
    /// What triggered this evaluation run.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub trigger: Option<EvalTrigger>,
    /// Cases that failed (score below threshold or evaluator error).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub failures: Vec<EvalFailure>,
}

/// One evaluation case (a single request/response pair).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvalCase {
    pub case_id: String,
    /// Input request.
    pub request: crate::agents::runtime::AgentRequest,
    /// Agent output.
    pub result: crate::agents::runtime::AgentRunResult,
    /// Optional ground-truth answer for comparison.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ground_truth: Option<String>,
    /// Scores produced by evaluators.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub scores: Vec<EvalScore>,
}

/// A named metric and its computed score for a case.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvalScore {
    pub metric: String,
    pub score: f64,
    /// Human-readable explanation of the score.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub explanation: Option<String>,
}

/// Summary statistics across all cases in a run.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvalSummary {
    pub total_cases: usize,
    pub passed_cases: usize,
    pub failed_cases: usize,
    /// Per-metric averages.
    pub metric_averages: BTreeMap<String, f64>,
    /// Overall score (weighted average of metric averages).
    pub overall_score: f64,
}

/// What triggered an evaluation run.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EvalTrigger {
    /// Pre-merge gate: runs on every PR before merge.
    PreMerge { dataset: String, sample_size: usize },
    /// Nightly regression suite: compares against baseline.
    NightlyRegression { dataset: String },
    /// Online sampling: evaluates a fraction of live traffic.
    OnlineSampling { rate: f64 },
    /// Red-team / adversarial probing.
    RedTeam {
        attack_vectors: Vec<crate::agents::redteam::AttackVector>,
    },
}

impl EvalTrigger {
    /// Default sample size per trigger type.
    pub fn default_sample_size(&self) -> usize {
        match self {
            EvalTrigger::PreMerge { .. } => 20,
            EvalTrigger::NightlyRegression { .. } => 100,
            EvalTrigger::OnlineSampling { .. } => 50,
            EvalTrigger::RedTeam { .. } => 30,
        }
    }

    /// Default minimum pass threshold (overall score must be >= this).
    pub fn default_pass_threshold(&self) -> f64 {
        match self {
            EvalTrigger::PreMerge { .. } => 0.75,
            EvalTrigger::NightlyRegression { .. } => 0.80,
            EvalTrigger::OnlineSampling { .. } => 0.70,
            EvalTrigger::RedTeam { .. } => 0.60,
        }
    }
}

/// Record of a failed evaluation case.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct EvalFailure {
    pub case_id: String,
    pub metric: String,
    pub score: f64,
    pub threshold: f64,
    pub reason: String,
}

// ---------------------------------------------------------------------------
// Quality / System metric definitions
// ---------------------------------------------------------------------------

/// Quality metrics: measure the correctness and usefulness of agent outputs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum QualityMetric {
    /// Fraction of cases that met all pass thresholds.
    TaskCompletionRate,
    /// Fraction of citations that are accurate (ground-truth aligned).
    CitationAccuracy,
    /// Whether the answer can be directly executed or acted upon.
    AnswerExecutability,
    /// Fraction of cases where hallucination was detected.
    HallucinationRate,
    /// Aggregated user satisfaction score (when available).
    UserSatisfaction,
}

impl QualityMetric {
    pub fn name(&self) -> &'static str {
        match self {
            QualityMetric::TaskCompletionRate => "task_completion_rate",
            QualityMetric::CitationAccuracy => "citation_accuracy",
            QualityMetric::AnswerExecutability => "answer_executability",
            QualityMetric::HallucinationRate => "hallucination_rate",
            QualityMetric::UserSatisfaction => "user_satisfaction",
        }
    }
}

/// System metrics: measure operational efficiency and reliability.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SystemMetric {
    /// Fraction of tool invocations that returned Ok.
    ToolSuccessRate,
    /// 50th percentile latency across cases (ms).
    LatencyP50,
    /// 95th percentile latency across cases (ms).
    LatencyP95,
    /// 99th percentile latency across cases (ms).
    LatencyP99,
    /// Tokens consumed per unit of useful output.
    TokenEfficiency,
    /// Estimated cost per evaluation run.
    CostPerRun,
    /// Fraction of runs that exhausted their budget.
    BudgetExhaustionRate,
    /// Fraction of cases that triggered a replan.
    ReplanRate,
    /// Fraction of failures that were successfully recovered.
    FailureRecoveryRate,
    /// Average number of tool calls per task.
    AvgToolCallsPerTask,
    /// Consistency score when replaying the same case.
    ReplayConsistency,
}

impl SystemMetric {
    pub fn name(&self) -> &'static str {
        match self {
            SystemMetric::ToolSuccessRate => "tool_success_rate",
            SystemMetric::LatencyP50 => "latency_p50",
            SystemMetric::LatencyP95 => "latency_p95",
            SystemMetric::LatencyP99 => "latency_p99",
            SystemMetric::TokenEfficiency => "token_efficiency",
            SystemMetric::CostPerRun => "cost_per_run",
            SystemMetric::BudgetExhaustionRate => "budget_exhaustion_rate",
            SystemMetric::ReplanRate => "replan_rate",
            SystemMetric::FailureRecoveryRate => "failure_recovery_rate",
            SystemMetric::AvgToolCallsPerTask => "avg_tool_calls_per_task",
            SystemMetric::ReplayConsistency => "replay_consistency",
        }
    }
}

/// A single computed metric value with an optional target.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct MetricValue {
    /// Metric name (use `QualityMetric::name()` or `SystemMetric::name()`).
    pub metric: String,
    /// Computed value (0.0–1.0 for rates, raw units for latencies / counts).
    pub value: f64,
    /// Optional target / SLO value for comparison.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub target: Option<f64>,
}

/// Built-in evaluation metrics.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EvalMetric {
    /// Exact string match against ground truth.
    ExactMatch,
    /// F1 score based on token overlap.
    F1,
    /// ROUGE-L score (longest common subsequence).
    RougeL,
    /// Semantic similarity via embeddings.
    SemanticSimilarity,
    /// LLM-as-judge: asks an LLM to score the answer.
    LlmAsJudge,
    /// Citation recall: what fraction of ground-truth claims are cited.
    CitationRecall,
    /// Citation precision: what fraction of citations support the answer.
    CitationPrecision,
    /// Hallucination detection: does the answer contain unsupported claims.
    Hallucination,
}

impl EvalMetric {
    pub fn name(&self) -> &'static str {
        match self {
            EvalMetric::ExactMatch => "exact_match",
            EvalMetric::F1 => "f1",
            EvalMetric::RougeL => "rouge_l",
            EvalMetric::SemanticSimilarity => "semantic_similarity",
            EvalMetric::LlmAsJudge => "llm_as_judge",
            EvalMetric::CitationRecall => "citation_recall",
            EvalMetric::CitationPrecision => "citation_precision",
            EvalMetric::Hallucination => "hallucination",
        }
    }
}

/// Dataset specification for a trigger-based evaluation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvalDatasetSpec {
    pub dataset_id: String,
    /// Max cases to sample (actual may be fewer if dataset is small).
    pub sample_size: usize,
    /// Optional filter expression (e.g. "mode == 'rag'").
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub filter: Option<String>,
}

/// Configuration for running an evaluation under a specific trigger.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvalTriggerConfig {
    pub trigger: EvalTrigger,
    pub dataset: EvalDatasetSpec,
    /// Overall score must be >= this to pass.
    pub pass_threshold: f64,
    /// Per-metric minimum thresholds (metric name -> min score).
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub metric_thresholds: BTreeMap<String, f64>,
}

impl EvalTriggerConfig {
    /// Build a default config for a trigger type and dataset.
    pub fn new(trigger: EvalTrigger, dataset_id: impl Into<String>) -> Self {
        let sample_size = trigger.default_sample_size();
        let pass_threshold = trigger.default_pass_threshold();
        Self {
            trigger,
            dataset: EvalDatasetSpec {
                dataset_id: dataset_id.into(),
                sample_size,
                filter: None,
            },
            pass_threshold,
            metric_thresholds: BTreeMap::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;

    #[test]
    fn eval_metric_names() {
        assert_eq!(EvalMetric::ExactMatch.name(), "exact_match");
        assert_eq!(EvalMetric::F1.name(), "f1");
        assert_eq!(EvalMetric::LlmAsJudge.name(), "llm_as_judge");
    }
    #[test]
    fn eval_trigger_default_sample_sizes() {
        assert_eq!(
            EvalTrigger::PreMerge {
                dataset: "d".to_string(),
                sample_size: 20
            }
            .default_sample_size(),
            20
        );
        assert_eq!(
            EvalTrigger::NightlyRegression {
                dataset: "d".to_string()
            }
            .default_sample_size(),
            100
        );
        assert_eq!(
            EvalTrigger::OnlineSampling { rate: 0.1 }.default_sample_size(),
            50
        );
        assert_eq!(
            EvalTrigger::RedTeam {
                attack_vectors: vec![]
            }
            .default_sample_size(),
            30
        );
    }

    #[test]
    fn eval_trigger_default_pass_thresholds() {
        assert!(
            (EvalTrigger::PreMerge {
                dataset: "d".to_string(),
                sample_size: 20
            }
            .default_pass_threshold()
                - 0.75)
                .abs()
                < 1e-6
        );
        assert!(
            (EvalTrigger::NightlyRegression {
                dataset: "d".to_string()
            }
            .default_pass_threshold()
                - 0.80)
                .abs()
                < 1e-6
        );
        assert!(
            (EvalTrigger::OnlineSampling { rate: 0.1 }.default_pass_threshold() - 0.70).abs()
                < 1e-6
        );
        assert!(
            (EvalTrigger::RedTeam {
                attack_vectors: vec![]
            }
            .default_pass_threshold()
                - 0.60)
                .abs()
                < 1e-6
        );
    }

    #[test]
    fn eval_trigger_config_builds_defaults() {
        let config = EvalTriggerConfig::new(
            EvalTrigger::PreMerge {
                dataset: "chat_smoke".to_string(),
                sample_size: 20,
            },
            "chat_smoke",
        );
        assert_eq!(config.dataset.dataset_id, "chat_smoke");
        assert_eq!(config.dataset.sample_size, 20);
        assert!((config.pass_threshold - 0.75).abs() < 1e-6);
        assert!(config.metric_thresholds.is_empty());
    }
}
