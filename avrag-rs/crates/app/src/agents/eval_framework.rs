//! Eval Framework — pluggable evaluation for agent outputs.
//!
//! Supports ground-truth comparison and LLM-as-judge scoring.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

// ---------------------------------------------------------------------------
// Core types
// ---------------------------------------------------------------------------

/// A single evaluation run against a dataset or a single case.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvalRun {
    pub run_id: String,
    pub run_name: String,
    /// Strategy under evaluation (e.g. "ChatStrategy", "RagStrategy").
    pub strategy: String,
    /// Version of the strategy code.
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

// ---------------------------------------------------------------------------
// Eval comparison — baseline vs candidate / golden set
// ---------------------------------------------------------------------------

/// Detected regression: a case or metric that degraded between baseline and candidate.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EvalRegression {
    /// What kind of regression was detected.
    pub kind: RegressionKind,
    /// Case ID if this is a per-case regression; None for aggregate metric regressions.
    pub case_id: Option<String>,
    /// Metric name that regressed.
    pub metric: String,
    /// Baseline score.
    pub baseline: f64,
    /// Candidate score.
    pub candidate: f64,
    /// Negative delta (candidate - baseline).
    pub delta: f64,
}

/// Classification of regression kinds.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RegressionKind {
    /// A single case score dropped below the threshold.
    CaseScoreDrop,
    /// A metric average dropped below the threshold.
    MetricAverageDrop,
    /// The overall score dropped below the threshold.
    OverallScoreDrop,
}

/// Per-case score change for a specific metric.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CaseScoreChange {
    pub case_id: String,
    pub metric: String,
    pub baseline_score: f64,
    pub candidate_score: f64,
    pub delta: f64,
}

/// Comparison result between a baseline EvalRun and a candidate EvalRun.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EvalComparison {
    pub baseline_run_id: String,
    pub candidate_run_id: String,
    /// Per-case, per-metric changes.
    pub per_case_changes: Vec<CaseScoreChange>,
    /// Per-metric average deltas.
    pub metric_deltas: BTreeMap<String, f64>,
    /// Overall score delta (candidate - baseline).
    pub overall_delta: f64,
    /// Detected regressions (empty if candidate improved or stayed flat).
    pub regressions: Vec<EvalRegression>,
}

/// Compare a candidate EvalRun against a baseline (previous version or golden set).
///
/// `regression_threshold`: a drop larger than this (in absolute terms) is flagged
/// as a regression. Default: 0.05 (5 percentage points).
pub fn compare_eval_runs(
    baseline: &EvalRun,
    candidate: &EvalRun,
    regression_threshold: f64,
) -> EvalComparison {
    let mut per_case_changes = Vec::new();
    let mut regressions = Vec::new();

    // Build baseline case lookup by case_id.
    let baseline_cases: std::collections::HashMap<_, _> = baseline
        .cases
        .iter()
        .map(|c| (c.case_id.clone(), c))
        .collect();

    for cand_case in &candidate.cases {
        let base_case = match baseline_cases.get(&cand_case.case_id) {
            Some(b) => b,
            None => continue, // new case in candidate — skip for comparison
        };

        // Compare scores by metric name.
        let cand_scores: std::collections::HashMap<_, _> = cand_case
            .scores
            .iter()
            .map(|s| (s.metric.clone(), s.score))
            .collect();
        let base_scores: std::collections::HashMap<_, _> = base_case
            .scores
            .iter()
            .map(|s| (s.metric.clone(), s.score))
            .collect();

        for (metric, cand_score) in &cand_scores {
            let base_score = base_scores.get(metric).copied().unwrap_or(0.0);
            let delta = cand_score - base_score;
            per_case_changes.push(CaseScoreChange {
                case_id: cand_case.case_id.clone(),
                metric: metric.clone(),
                baseline_score: base_score,
                candidate_score: *cand_score,
                delta,
            });

            if delta < -regression_threshold {
                regressions.push(EvalRegression {
                    kind: RegressionKind::CaseScoreDrop,
                    case_id: Some(cand_case.case_id.clone()),
                    metric: metric.clone(),
                    baseline: base_score,
                    candidate: *cand_score,
                    delta,
                });
            }
        }
    }

    // Metric average deltas.
    let base_averages = baseline
        .summary
        .as_ref()
        .map(|s| s.metric_averages.clone())
        .unwrap_or_default();
    let cand_averages = candidate
        .summary
        .as_ref()
        .map(|s| s.metric_averages.clone())
        .unwrap_or_default();

    let mut metric_deltas: BTreeMap<String, f64> = BTreeMap::new();
    let all_metrics: std::collections::HashSet<_> = base_averages
        .keys()
        .chain(cand_averages.keys())
        .cloned()
        .collect();

    for metric in all_metrics {
        let base = base_averages.get(&metric).copied().unwrap_or(0.0);
        let cand = cand_averages.get(&metric).copied().unwrap_or(0.0);
        let delta = cand - base;
        metric_deltas.insert(metric.clone(), delta);

        if delta < -regression_threshold {
            regressions.push(EvalRegression {
                kind: RegressionKind::MetricAverageDrop,
                case_id: None,
                metric,
                baseline: base,
                candidate: cand,
                delta,
            });
        }
    }

    // Overall delta.
    let base_overall = baseline
        .summary
        .as_ref()
        .map(|s| s.overall_score)
        .unwrap_or(0.0);
    let cand_overall = candidate
        .summary
        .as_ref()
        .map(|s| s.overall_score)
        .unwrap_or(0.0);
    let overall_delta = cand_overall - base_overall;

    if overall_delta < -regression_threshold {
        regressions.push(EvalRegression {
            kind: RegressionKind::OverallScoreDrop,
            case_id: None,
            metric: "overall".to_string(),
            baseline: base_overall,
            candidate: cand_overall,
            delta: overall_delta,
        });
    }

    EvalComparison {
        baseline_run_id: baseline.run_id.clone(),
        candidate_run_id: candidate.run_id.clone(),
        per_case_changes,
        metric_deltas,
        overall_delta,
        regressions,
    }
}

// ---------------------------------------------------------------------------
// Metric definitions
// ---------------------------------------------------------------------------

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

/// Result of a trigger-based evaluation, including metrics and optional baseline comparison.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvalResult {
    /// What triggered this evaluation run.
    pub trigger: EvalTrigger,
    /// Fraction of cases that passed all thresholds.
    pub pass_rate: f64,
    /// Average latency per case in milliseconds.
    pub avg_latency_ms: u64,
    /// Average token consumption per case.
    pub avg_tokens: u64,
    /// Cases that failed (score below threshold or evaluator error).
    pub failures: Vec<EvalFailure>,
    /// Comparison against a baseline run, if one was provided.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub comparison: Option<EvalComparison>,
    /// Quality metrics computed from the evaluation run.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub quality_metrics: Vec<MetricValue>,
    /// System metrics computed from the evaluation run.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub system_metrics: Vec<MetricValue>,
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

// ---------------------------------------------------------------------------
// Evaluator trait
// ---------------------------------------------------------------------------

/// Pluggable evaluator interface.
#[async_trait::async_trait]
pub trait Evaluator: Send + Sync {
    /// Evaluate a single case and return a score.
    async fn evaluate(&self, case: &EvalCase) -> Result<EvalScore, common::AppError>;
}

// ---------------------------------------------------------------------------
// Built-in evaluators
// ---------------------------------------------------------------------------

/// Exact-match evaluator (case-insensitive, after normalisation).
pub struct ExactMatchEvaluator;

#[async_trait::async_trait]
impl Evaluator for ExactMatchEvaluator {
    async fn evaluate(&self, case: &EvalCase) -> Result<EvalScore, common::AppError> {
        let ground = case.ground_truth.as_ref().ok_or_else(|| {
            common::AppError::validation("missing_ground_truth", "ExactMatch requires ground_truth")
        })?;
        let prediction = normalize(&case.result.answer);
        let reference = normalize(ground);
        let score = if prediction == reference { 1.0 } else { 0.0 };
        Ok(EvalScore {
            metric: EvalMetric::ExactMatch.name().to_string(),
            score,
            explanation: Some(format!(
                "prediction_len={}, reference_len={}, match={}",
                prediction.len(),
                reference.len(),
                score == 1.0
            )),
        })
    }
}

/// F1 score based on whitespace token overlap.
pub struct F1Evaluator;

#[async_trait::async_trait]
impl Evaluator for F1Evaluator {
    async fn evaluate(&self, case: &EvalCase) -> Result<EvalScore, common::AppError> {
        let ground = case.ground_truth.as_ref().ok_or_else(|| {
            common::AppError::validation("missing_ground_truth", "F1 requires ground_truth")
        })?;
        let pred_tokens = tokenize(&case.result.answer);
        let ref_tokens = tokenize(ground);
        let score = compute_f1(&pred_tokens, &ref_tokens);
        Ok(EvalScore {
            metric: EvalMetric::F1.name().to_string(),
            score,
            explanation: Some(format!(
                "pred_tokens={}, ref_tokens={}, f1={:.2}",
                pred_tokens.len(),
                ref_tokens.len(),
                score
            )),
        })
    }
}

/// LLM-as-judge evaluator.
///
/// Prompts an LLM to score the answer on a 1–5 scale with reasoning.
pub struct LlmAsJudgeEvaluator {
    llm_client: avrag_llm::LlmClient,
    criteria: String,
}

impl LlmAsJudgeEvaluator {
    pub fn new(llm_client: avrag_llm::LlmClient, criteria: impl Into<String>) -> Self {
        Self {
            llm_client,
            criteria: criteria.into(),
        }
    }
}

#[async_trait::async_trait]
impl Evaluator for LlmAsJudgeEvaluator {
    async fn evaluate(&self, case: &EvalCase) -> Result<EvalScore, common::AppError> {
        let user_prompt = format!(
            "You are an expert evaluator. Evaluate the following answer based on this criterion:\n{}\n\n\
             Question: {}\nAnswer: {}\n\n\
             Provide a score between 0.0 and 1.0 and a brief explanation. \
             Respond in JSON format: {{\"score\": float, \"explanation\": string}}",
            self.criteria, case.request.query, case.result.answer
        );

        let messages = vec![
            avrag_llm::ChatMessage::system(
                "You are an objective evaluator. Respond only with valid JSON.",
            ),
            avrag_llm::ChatMessage::user(user_prompt),
        ];

        let response = self
            .llm_client
            .complete(&messages, None)
            .await
            .map_err(|e| common::AppError::internal(format!("LLM-as-judge failed: {e}")))?;

        // Parse JSON from LLM response.
        let (score, explanation) = match parse_llm_judge_output(&response.content) {
            Ok((s, e)) => (s, e),
            Err(parse_err) => {
                tracing::warn!(
                    content = %response.content,
                    error = %parse_err,
                    "LLM-as-judge returned unparsable output; falling back to score 0.0"
                );
                (0.0, Some(format!("Parse error: {parse_err}")))
            }
        };

        Ok(EvalScore {
            metric: EvalMetric::LlmAsJudge.name().to_string(),
            score,
            explanation,
        })
    }
}

/// Parse the JSON output from an LLM-as-judge call.
///
/// Expected format: `{"score": float, "explanation": string}`
/// The LLM may wrap the JSON in markdown fences or include extra text;
/// this function extracts the first JSON object it finds.
fn parse_llm_judge_output(content: &str) -> Result<(f64, Option<String>), String> {
    // Try to find a JSON object in the content.
    let json_str = extract_first_json_object(content)
        .ok_or_else(|| "No JSON object found in response".to_string())?;

    let value: serde_json::Value =
        serde_json::from_str(json_str).map_err(|e| format!("Invalid JSON: {e}"))?;

    let score = value
        .get("score")
        .and_then(|v| v.as_f64())
        .ok_or_else(|| "Missing or non-numeric 'score' field".to_string())?;

    let explanation = value
        .get("explanation")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());

    Ok((score.clamp(0.0, 1.0), explanation))
}

/// Extract the first JSON object `{...}` from a string, tolerating
/// markdown fences and surrounding text.
fn extract_first_json_object(text: &str) -> Option<&str> {
    // First try to find JSON inside markdown code fences.
    if let Some(start) = text.find("```json") {
        let after_fence = &text[start + 7..];
        if let Some(end) = after_fence.find("```") {
            return Some(after_fence[..end].trim());
        }
    }
    if let Some(start) = text.find("```") {
        let after_fence = &text[start + 3..];
        if let Some(end) = after_fence.find("```") {
            let candidate = after_fence[..end].trim();
            if candidate.starts_with('{') {
                return Some(candidate);
            }
        }
    }

    // Fall back to first `{...}` pair at the top level.
    let mut depth = 0;
    let mut start = None;
    for (i, ch) in text.char_indices() {
        match ch {
            '{' => {
                if depth == 0 {
                    start = Some(i);
                }
                depth += 1;
            }
            '}' => {
                if depth > 0 {
                    depth -= 1;
                    if depth == 0
                        && let Some(s) = start
                    {
                        return Some(&text[s..=i]);
                    }
                }
            }
            _ => {}
        }
    }
    None
}

// ---------------------------------------------------------------------------
// Runner
// ---------------------------------------------------------------------------

/// Run a suite of evaluators over a set of cases and produce an EvalRun.
pub async fn run_evaluation(
    run_name: impl Into<String>,
    strategy: impl Into<String>,
    strategy_version: impl Into<String>,
    cases: Vec<EvalCase>,
    evaluators: Vec<Box<dyn Evaluator>>,
) -> Result<EvalRun, common::AppError> {
    let started_at_ms = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64;

    let mut evaluated_cases = Vec::with_capacity(cases.len());
    let mut metric_sums: BTreeMap<String, (f64, usize)> = BTreeMap::new();
    let mut passed = 0usize;
    let mut failed = 0usize;

    for mut case in cases {
        for evaluator in &evaluators {
            match evaluator.evaluate(&case).await {
                Ok(score) => {
                    if score.score >= 0.7 {
                        passed += 1;
                    } else {
                        failed += 1;
                    }
                    let entry = metric_sums.entry(score.metric.clone()).or_insert((0.0, 0));
                    entry.0 += score.score;
                    entry.1 += 1;
                    case.scores.push(score);
                }
                Err(e) => {
                    tracing::warn!(case_id = %case.case_id, error = %e, "evaluator failed");
                    failed += 1;
                }
            }
        }
        evaluated_cases.push(case);
    }

    let metric_averages: BTreeMap<String, f64> = metric_sums
        .iter()
        .map(|(k, (sum, count))| {
            (
                k.clone(),
                if *count > 0 { sum / *count as f64 } else { 0.0 },
            )
        })
        .collect();

    let overall_score = if metric_averages.is_empty() {
        0.0
    } else {
        metric_averages.values().sum::<f64>() / metric_averages.len() as f64
    };

    let completed_at_ms = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64;
    let total_cases = evaluated_cases.len();

    Ok(EvalRun {
        run_id: format!("eval-{}", uuid::Uuid::new_v4()),
        run_name: run_name.into(),
        strategy: strategy.into(),
        strategy_version: strategy_version.into(),
        started_at_ms,
        completed_at_ms: Some(completed_at_ms),
        cases: evaluated_cases,
        summary: Some(EvalSummary {
            total_cases,
            passed_cases: passed,
            failed_cases: failed,
            metric_averages,
            overall_score,
        }),
        trigger: None,
        failures: vec![],
    })
}

/// Compute quality and system metrics from an EvalRun.
///
/// Derives what it can from the `EvalRun` / `AgentRunResult` fields;
/// metrics that require data not yet collected (e.g. cost, replay) are
/// emitted with a value of `0.0` and no target.
fn compute_metrics(run: &EvalRun) -> (Vec<MetricValue>, Vec<MetricValue>) {
    let mut quality = Vec::new();
    let mut system = Vec::new();

    // ---- Quality metrics ----

    // TaskCompletionRate = pass_rate (already computed).
    let total_cases = run.cases.len();
    let unique_failed: std::collections::HashSet<_> = run
        .failures
        .iter()
        .filter(|f| f.case_id != "__overall__")
        .map(|f| f.case_id.clone())
        .collect();
    let passed = total_cases.saturating_sub(unique_failed.len());
    let task_completion_rate = if total_cases == 0 {
        0.0
    } else {
        passed as f64 / total_cases as f64
    };
    quality.push(MetricValue {
        metric: QualityMetric::TaskCompletionRate.name().to_string(),
        value: task_completion_rate,
        target: Some(0.8),
    });

    // CitationAccuracy: average of citation_precision scores when available.
    let citation_scores: Vec<f64> = run
        .cases
        .iter()
        .flat_map(|c| c.scores.iter())
        .filter(|s| s.metric == "citation_precision" || s.metric == "citation_recall")
        .map(|s| s.score)
        .collect();
    if !citation_scores.is_empty() {
        let avg = citation_scores.iter().sum::<f64>() / citation_scores.len() as f64;
        quality.push(MetricValue {
            metric: QualityMetric::CitationAccuracy.name().to_string(),
            value: avg,
            target: Some(0.75),
        });
    }

    // HallucinationRate: fraction of cases with a hallucination score below threshold.
    let hallucination_scores: Vec<f64> = run
        .cases
        .iter()
        .flat_map(|c| c.scores.iter())
        .filter(|s| s.metric == "hallucination")
        .map(|s| s.score)
        .collect();
    if !hallucination_scores.is_empty() {
        // Higher hallucination score = more hallucination detected; rate = average.
        let avg = hallucination_scores.iter().sum::<f64>() / hallucination_scores.len() as f64;
        quality.push(MetricValue {
            metric: QualityMetric::HallucinationRate.name().to_string(),
            value: avg,
            target: Some(0.1),
        });
    }

    // ---- System metrics ----

    // ToolSuccessRate: from tool_calls status across all cases.
    let total_tool_calls: usize = run.cases.iter().map(|c| c.result.tool_calls.len()).sum();
    let ok_tool_calls: usize = run
        .cases
        .iter()
        .flat_map(|c| c.result.tool_calls.iter())
        .filter(|tc| tc.status == common::ToolStatus::Ok)
        .count();
    if total_tool_calls > 0 {
        system.push(MetricValue {
            metric: SystemMetric::ToolSuccessRate.name().to_string(),
            value: ok_tool_calls as f64 / total_tool_calls as f64,
            target: Some(0.95),
        });
    }

    // Latency percentiles from total_elapsed_ms.
    let mut latencies: Vec<u64> = run
        .cases
        .iter()
        .filter_map(|c| c.result.total_elapsed_ms)
        .collect();
    if !latencies.is_empty() {
        latencies.sort_unstable();
        let p50_idx = (latencies.len() as f64 * 0.50) as usize;
        let p95_idx = ((latencies.len() as f64 * 0.95) as usize).min(latencies.len() - 1);
        let p99_idx = ((latencies.len() as f64 * 0.99) as usize).min(latencies.len() - 1);
        system.push(MetricValue {
            metric: SystemMetric::LatencyP50.name().to_string(),
            value: latencies[p50_idx] as f64,
            target: Some(2000.0),
        });
        system.push(MetricValue {
            metric: SystemMetric::LatencyP95.name().to_string(),
            value: latencies[p95_idx] as f64,
            target: Some(5000.0),
        });
        system.push(MetricValue {
            metric: SystemMetric::LatencyP99.name().to_string(),
            value: latencies[p99_idx] as f64,
            target: Some(10000.0),
        });
    }

    // TokenEfficiency: total tokens per case (lower is better, but we emit raw).
    let total_tokens: u64 = run
        .cases
        .iter()
        .filter_map(|c| c.result.usage.as_ref().map(|u| u.total_tokens))
        .sum();
    if total_cases > 0 && total_tokens > 0 {
        system.push(MetricValue {
            metric: SystemMetric::TokenEfficiency.name().to_string(),
            value: total_tokens as f64 / total_cases as f64,
            target: Some(2000.0),
        });
    }

    // BudgetExhaustionRate: fraction of cases where current >= max budget.
    let budget_exhausted: usize = run
        .cases
        .iter()
        .filter(|c| {
            c.result
                .budget_used
                .as_ref()
                .map(|b| b.current >= b.max && b.max > 0)
                .unwrap_or(false)
        })
        .count();
    if total_cases > 0 {
        system.push(MetricValue {
            metric: SystemMetric::BudgetExhaustionRate.name().to_string(),
            value: budget_exhausted as f64 / total_cases as f64,
            target: Some(0.05),
        });
    }

    // ReplanRate: fraction of cases with at least one replan decision.
    let replanned: usize = run
        .cases
        .iter()
        .filter(|c| c.result.iterations.iter().any(|it| it.decision == "replan"))
        .count();
    if total_cases > 0 {
        system.push(MetricValue {
            metric: SystemMetric::ReplanRate.name().to_string(),
            value: replanned as f64 / total_cases as f64,
            target: Some(0.2),
        });
    }

    // AvgToolCallsPerTask.
    if total_cases > 0 {
        system.push(MetricValue {
            metric: SystemMetric::AvgToolCallsPerTask.name().to_string(),
            value: total_tool_calls as f64 / total_cases as f64,
            target: Some(5.0),
        });
    }

    (quality, system)
}

/// Dataset specification for a trigger-based evaluation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvalDatasetSpec {
    pub dataset_id: String,
    /// Max cases to sample (actual may be fewer if dataset is small).
    pub sample_size: usize,
    /// Optional filter expression (e.g. "strategy == 'RagStrategy'").
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

/// Run an evaluation under a specific trigger, collecting failures and producing an EvalResult.
///
/// 1. Runs all evaluators over the cases.
/// 2. Compares scores against trigger-specific thresholds.
/// 3. Populates `EvalRun.trigger` and `EvalRun.failures`.
/// 4. If `baseline` is provided, computes an `EvalComparison` and attaches it to the result.
pub async fn run_eval_with_trigger(
    run_name: impl Into<String>,
    strategy: impl Into<String>,
    strategy_version: impl Into<String>,
    cases: Vec<EvalCase>,
    evaluators: Vec<Box<dyn Evaluator>>,
    config: EvalTriggerConfig,
    baseline: Option<&EvalRun>,
) -> Result<(EvalRun, EvalResult), common::AppError> {
    let mut run = run_evaluation(run_name, strategy, strategy_version, cases, evaluators).await?;

    run.trigger = Some(config.trigger.clone());
    run.failures.clear();

    // Collect failures: per-case per-metric under threshold.
    for case in &run.cases {
        for score in &case.scores {
            let threshold = config
                .metric_thresholds
                .get(&score.metric)
                .copied()
                .unwrap_or(config.pass_threshold);
            if score.score < threshold {
                run.failures.push(EvalFailure {
                    case_id: case.case_id.clone(),
                    metric: score.metric.clone(),
                    score: score.score,
                    threshold,
                    reason: format!(
                        "{} score {:.2} below threshold {:.2}",
                        score.metric, score.score, threshold
                    ),
                });
            }
        }
    }

    // Also flag overall score.
    if let Some(ref summary) = run.summary
        && summary.overall_score < config.pass_threshold
    {
        run.failures.push(EvalFailure {
            case_id: "__overall__".to_string(),
            metric: "overall".to_string(),
            score: summary.overall_score,
            threshold: config.pass_threshold,
            reason: format!(
                "overall score {:.2} below pass threshold {:.2}",
                summary.overall_score, config.pass_threshold
            ),
        });
    }

    // Compute pass rate.
    let total_cases = run.cases.len();
    let pass_rate = if total_cases == 0 {
        0.0
    } else {
        let _failed_count = run
            .failures
            .iter()
            .filter(|f| f.case_id != "__overall__")
            .count();
        let unique_failed_cases: std::collections::HashSet<_> = run
            .failures
            .iter()
            .filter(|f| f.case_id != "__overall__")
            .map(|f| f.case_id.clone())
            .collect();
        let passed = total_cases.saturating_sub(unique_failed_cases.len());
        passed as f64 / total_cases as f64
    };

    // Compute system metrics from case results.
    let (quality_metrics, system_metrics) = compute_metrics(&run);

    // Build EvalResult.
    let mut result = EvalResult {
        trigger: config.trigger,
        pass_rate,
        avg_latency_ms: 0, // TODO: collect per-case latency when available
        avg_tokens: 0,     // TODO: collect per-case tokens when available
        failures: run.failures.clone(),
        comparison: None,
        quality_metrics,
        system_metrics,
    };

    // If baseline provided, compute comparison.
    if let Some(base) = baseline {
        let comparison = compare_eval_runs(base, &run, 0.05);
        result.comparison = Some(comparison);
    }

    Ok((run, result))
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn normalize(text: &str) -> String {
    text.to_lowercase()
        .replace(|c: char| c.is_ascii_punctuation(), " ")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

fn tokenize(text: &str) -> Vec<String> {
    normalize(text)
        .split_whitespace()
        .map(|s| s.to_string())
        .collect()
}

fn compute_f1(pred: &[String], reference: &[String]) -> f64 {
    if pred.is_empty() && reference.is_empty() {
        return 1.0;
    }
    if pred.is_empty() || reference.is_empty() {
        return 0.0;
    }

    let pred_set: std::collections::HashSet<_> = pred.iter().collect();
    let ref_set: std::collections::HashSet<_> = reference.iter().collect();

    let overlap: std::collections::HashSet<_> = pred_set.intersection(&ref_set).collect();
    let overlap_count = overlap.len() as f64;

    let precision = overlap_count / pred_set.len() as f64;
    let recall = overlap_count / ref_set.len() as f64;

    if precision + recall == 0.0 {
        0.0
    } else {
        2.0 * (precision * recall) / (precision + recall)
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn dummy_case(answer: &str, ground: Option<&str>) -> EvalCase {
        EvalCase {
            case_id: "c1".to_string(),
            request: crate::agents::runtime::AgentRequest {
                kind: crate::agents::AgentKind::Chat,
                query: "q".to_string(),
                notebook_id: None,
                session_id: None,
                doc_scope: vec![],
                messages: vec![],
                session_summary: None,
                user_preferences: None,
                debug: false,
                stream: false,
                language: None,
                auth_context: serde_json::json!({}),
                docscope_metadata: None,
                metadata: BTreeMap::new(),
                cancellation_token: None,
                guard_pipeline: None,
                preferred_tools: vec![],
                format_hint: None,
                max_iterations: None,
            },
            result: {
                let mut r = crate::agents::runtime::AgentRunResult::default();
                r.answer = answer.to_string();
                r
            },
            ground_truth: ground.map(|s| s.to_string()),
            scores: vec![],
        }
    }

    #[tokio::test]
    async fn exact_match_perfect() {
        let case = dummy_case("hello world", Some("hello world"));
        let eval = ExactMatchEvaluator;
        let score = eval.evaluate(&case).await.unwrap();
        assert_eq!(score.score, 1.0);
    }

    #[tokio::test]
    async fn exact_match_fails_on_difference() {
        let case = dummy_case("hello world", Some("hello"));
        let eval = ExactMatchEvaluator;
        let score = eval.evaluate(&case).await.unwrap();
        assert_eq!(score.score, 0.0);
    }

    #[tokio::test]
    async fn exact_match_normalizes_case_and_punctuation() {
        let case = dummy_case("Hello, World!", Some("hello world"));
        let eval = ExactMatchEvaluator;
        let score = eval.evaluate(&case).await.unwrap();
        assert_eq!(score.score, 1.0);
    }

    #[tokio::test]
    async fn exact_match_requires_ground_truth() {
        let case = dummy_case("hello", None);
        let eval = ExactMatchEvaluator;
        let result = eval.evaluate(&case).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn f1_perfect_match() {
        let case = dummy_case("a b c", Some("a b c"));
        let eval = F1Evaluator;
        let score = eval.evaluate(&case).await.unwrap();
        assert!(score.score > 0.99);
    }

    #[tokio::test]
    async fn f1_partial_match() {
        let case = dummy_case("a b c", Some("a b d"));
        let eval = F1Evaluator;
        let score = eval.evaluate(&case).await.unwrap();
        assert!(score.score > 0.5 && score.score < 1.0);
    }

    #[tokio::test]
    async fn f1_no_overlap() {
        let case = dummy_case("x y z", Some("a b c"));
        let eval = F1Evaluator;
        let score = eval.evaluate(&case).await.unwrap();
        assert_eq!(score.score, 0.0);
    }

    #[tokio::test]
    async fn run_evaluation_computes_summary() {
        let cases = vec![
            dummy_case("hello world", Some("hello world")),
            dummy_case("foo bar", Some("foo baz")),
        ];
        let evaluators: Vec<Box<dyn Evaluator>> =
            vec![Box::new(ExactMatchEvaluator), Box::new(F1Evaluator)];
        let run = run_evaluation("test", "ChatStrategy", "v1", cases, evaluators)
            .await
            .unwrap();
        assert!(run.summary.is_some());
        let summary = run.summary.unwrap();
        assert_eq!(summary.total_cases, 2);
        assert!(summary.overall_score > 0.0);
        assert!(summary.metric_averages.contains_key("exact_match"));
        assert!(summary.metric_averages.contains_key("f1"));
    }

    #[test]
    fn eval_metric_names() {
        assert_eq!(EvalMetric::ExactMatch.name(), "exact_match");
        assert_eq!(EvalMetric::F1.name(), "f1");
        assert_eq!(EvalMetric::LlmAsJudge.name(), "llm_as_judge");
    }

    // ---------------- compare_eval_runs ----------------

    fn make_run(run_id: &str, cases: Vec<EvalCase>, overall: f64) -> EvalRun {
        let mut metric_sums: BTreeMap<String, (f64, usize)> = BTreeMap::new();
        for c in &cases {
            for s in &c.scores {
                let entry = metric_sums.entry(s.metric.clone()).or_insert((0.0, 0));
                entry.0 += s.score;
                entry.1 += 1;
            }
        }
        let metric_averages: BTreeMap<String, f64> = metric_sums
            .into_iter()
            .map(|(k, (sum, count))| (k, sum / count as f64))
            .collect();

        EvalRun {
            run_id: run_id.to_string(),
            run_name: "test".to_string(),
            strategy: "ChatStrategy".to_string(),
            strategy_version: "v1".to_string(),
            started_at_ms: 0,
            completed_at_ms: Some(0),
            cases,
            summary: Some(EvalSummary {
                total_cases: 2,
                passed_cases: 0,
                failed_cases: 0,
                metric_averages,
                overall_score: overall,
            }),
            trigger: None,
            failures: vec![],
        }
    }

    fn case_with_score(case_id: &str, metric: &str, score: f64) -> EvalCase {
        let mut case = dummy_case("answer", Some("ground"));
        case.case_id = case_id.to_string();
        case.scores = vec![EvalScore {
            metric: metric.to_string(),
            score,
            explanation: None,
        }];
        case
    }

    #[test]
    fn comparison_detects_no_regression_when_improved() {
        let baseline = make_run("base", vec![case_with_score("c1", "exact_match", 0.5)], 0.5);
        let candidate = make_run("cand", vec![case_with_score("c1", "exact_match", 0.8)], 0.8);
        let comp = compare_eval_runs(&baseline, &candidate, 0.05);
        assert!(comp.regressions.is_empty());
        assert!((comp.overall_delta - 0.3).abs() < 1e-6);
    }

    #[test]
    fn comparison_detects_case_score_regression() {
        let baseline = make_run("base", vec![case_with_score("c1", "exact_match", 0.8)], 0.8);
        let candidate = make_run("cand", vec![case_with_score("c1", "exact_match", 0.6)], 0.6);
        let comp = compare_eval_runs(&baseline, &candidate, 0.05);
        assert_eq!(comp.regressions.len(), 3); // case + metric average + overall
        assert!(
            comp.regressions
                .iter()
                .any(|r| r.kind == RegressionKind::CaseScoreDrop)
        );
    }

    #[test]
    fn comparison_detects_overall_regression() {
        let baseline = make_run("base", vec![case_with_score("c1", "f1", 0.9)], 0.9);
        let candidate = make_run("cand", vec![case_with_score("c1", "f1", 0.7)], 0.7);
        let comp = compare_eval_runs(&baseline, &candidate, 0.05);
        assert!(
            comp.regressions
                .iter()
                .any(|r| r.kind == RegressionKind::OverallScoreDrop)
        );
    }

    #[test]
    fn comparison_ignores_cases_missing_in_baseline() {
        let baseline = make_run("base", vec![case_with_score("c1", "m1", 0.5)], 0.5);
        let mut candidate_cases = vec![case_with_score("c1", "m1", 0.6)];
        candidate_cases.push(case_with_score("c2", "m1", 0.4)); // new case
        let candidate = make_run("cand", candidate_cases, 0.6);
        let comp = compare_eval_runs(&baseline, &candidate, 0.05);
        // c2 is skipped because it is not in baseline.
        assert_eq!(comp.per_case_changes.len(), 1);
    }

    // ---------------- EvalTrigger ----------------

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

    #[tokio::test]
    async fn run_eval_with_trigger_collects_failures() {
        let cases = vec![
            dummy_case("hello world", Some("hello world")), // exact_match=1.0, f1=1.0
            dummy_case("foo bar", Some("foo baz")),         // exact_match=0.0, f1<1.0
        ];
        let evaluators: Vec<Box<dyn Evaluator>> =
            vec![Box::new(ExactMatchEvaluator), Box::new(F1Evaluator)];
        let config = EvalTriggerConfig {
            trigger: EvalTrigger::PreMerge {
                dataset: "test".to_string(),
                sample_size: 2,
            },
            dataset: EvalDatasetSpec {
                dataset_id: "test".to_string(),
                sample_size: 2,
                filter: None,
            },
            pass_threshold: 0.75,
            metric_thresholds: BTreeMap::new(),
        };
        let (run, result) =
            run_eval_with_trigger("t", "ChatStrategy", "v1", cases, evaluators, config, None)
                .await
                .unwrap();
        assert_eq!(
            run.trigger,
            Some(EvalTrigger::PreMerge {
                dataset: "test".to_string(),
                sample_size: 2
            })
        );
        assert!(
            !run.failures.is_empty(),
            "expected some failures below 0.75 threshold"
        );
        assert!(run.failures.iter().any(|f| f.metric == "exact_match"));
        assert!(run.failures.iter().any(|f| f.metric == "overall"));
        assert_eq!(
            result.trigger,
            EvalTrigger::PreMerge {
                dataset: "test".to_string(),
                sample_size: 2
            }
        );
        assert!(result.pass_rate >= 0.0 && result.pass_rate <= 1.0);
    }

    #[tokio::test]
    async fn run_eval_with_trigger_uses_metric_thresholds() {
        let cases = vec![dummy_case("hello world", Some("hello world"))];
        let evaluators: Vec<Box<dyn Evaluator>> = vec![Box::new(ExactMatchEvaluator)];
        let mut metric_thresholds = BTreeMap::new();
        metric_thresholds.insert("exact_match".to_string(), 1.01); // impossible
        let config = EvalTriggerConfig {
            trigger: EvalTrigger::NightlyRegression {
                dataset: "test".to_string(),
            },
            dataset: EvalDatasetSpec {
                dataset_id: "test".to_string(),
                sample_size: 1,
                filter: None,
            },
            pass_threshold: 0.5,
            metric_thresholds,
        };
        let (run, _result) =
            run_eval_with_trigger("t", "ChatStrategy", "v1", cases, evaluators, config, None)
                .await
                .unwrap();
        assert_eq!(
            run.trigger,
            Some(EvalTrigger::NightlyRegression {
                dataset: "test".to_string()
            })
        );
        assert!(
            run.failures
                .iter()
                .any(|f| f.metric == "exact_match" && f.threshold == 1.01)
        );
    }

    #[tokio::test]
    async fn run_eval_with_trigger_no_failures_when_all_pass() {
        let cases = vec![dummy_case("hello world", Some("hello world"))];
        let evaluators: Vec<Box<dyn Evaluator>> = vec![Box::new(ExactMatchEvaluator)];
        let config = EvalTriggerConfig {
            trigger: EvalTrigger::RedTeam {
                attack_vectors: vec![],
            },
            dataset: EvalDatasetSpec {
                dataset_id: "test".to_string(),
                sample_size: 1,
                filter: None,
            },
            pass_threshold: 0.0, // everything passes
            metric_thresholds: BTreeMap::new(),
        };
        let (run, result) =
            run_eval_with_trigger("t", "ChatStrategy", "v1", cases, evaluators, config, None)
                .await
                .unwrap();
        assert!(
            run.failures.is_empty(),
            "expected no failures with threshold 0.0"
        );
        assert_eq!(result.pass_rate, 1.0);
    }

    #[tokio::test]
    async fn run_eval_with_trigger_comparison_with_baseline() {
        let cases = vec![dummy_case("hello world", Some("hello world"))];
        let evaluators: Vec<Box<dyn Evaluator>> = vec![Box::new(ExactMatchEvaluator)];
        let config = EvalTriggerConfig {
            trigger: EvalTrigger::PreMerge {
                dataset: "test".to_string(),
                sample_size: 1,
            },
            dataset: EvalDatasetSpec {
                dataset_id: "test".to_string(),
                sample_size: 1,
                filter: None,
            },
            pass_threshold: 0.0,
            metric_thresholds: BTreeMap::new(),
        };

        // Build a baseline run with lower score.
        let baseline = make_run("base", vec![case_with_score("c1", "exact_match", 0.5)], 0.5);

        let (_run, result) = run_eval_with_trigger(
            "t",
            "ChatStrategy",
            "v1",
            cases,
            evaluators,
            config,
            Some(&baseline),
        )
        .await
        .unwrap();

        assert!(
            result.comparison.is_some(),
            "expected comparison when baseline is provided"
        );
        let comp = result.comparison.unwrap();
        assert_eq!(comp.baseline_run_id, "base");
        assert!(
            comp.overall_delta > 0.0,
            "expected positive delta since candidate improved"
        );
        assert!(
            comp.regressions.is_empty(),
            "expected no regressions since candidate improved"
        );
    }

    // ---------------- parse_llm_judge_output ----------------

    #[test]
    fn parse_llm_judge_output_valid_json() {
        let text = r#"{"score": 0.85, "explanation": "Good answer"}"#;
        let (score, explanation) = parse_llm_judge_output(text).unwrap();
        assert!((score - 0.85).abs() < 1e-6);
        assert_eq!(explanation, Some("Good answer".to_string()));
    }

    #[test]
    fn parse_llm_judge_output_clamps_out_of_range() {
        let text = r#"{"score": 1.5, "explanation": "Over"}"#;
        let (score, _) = parse_llm_judge_output(text).unwrap();
        assert_eq!(score, 1.0);

        let text2 = r#"{"score": -0.3, "explanation": "Under"}"#;
        let (score2, _) = parse_llm_judge_output(text2).unwrap();
        assert_eq!(score2, 0.0);
    }

    #[test]
    fn parse_llm_judge_output_tolerates_markdown_fences() {
        let text = "Some intro text.\n```json\n{\"score\": 0.75, \"explanation\": \"ok\"}\n```";
        let (score, explanation) = parse_llm_judge_output(text).unwrap();
        assert!((score - 0.75).abs() < 1e-6);
        assert_eq!(explanation, Some("ok".to_string()));
    }

    #[test]
    fn parse_llm_judge_output_rejects_missing_score() {
        let text = r#"{"explanation": "no score"}"#;
        assert!(parse_llm_judge_output(text).is_err());
    }

    #[test]
    fn parse_llm_judge_output_rejects_invalid_json() {
        let text = "not json at all";
        assert!(parse_llm_judge_output(text).is_err());
    }

    #[test]
    fn parse_llm_judge_output_allows_no_explanation() {
        let text = r#"{"score": 0.5}"#;
        let (score, explanation) = parse_llm_judge_output(text).unwrap();
        assert!((score - 0.5).abs() < 1e-6);
        assert_eq!(explanation, None);
    }

    #[test]
    fn extract_first_json_object_finds_nested() {
        let text = "prefix {\"a\": {\"b\": 1}} suffix";
        let extracted = extract_first_json_object(text).unwrap();
        assert_eq!(extracted, r#"{"a": {"b": 1}}"#);
    }

    #[test]
    fn extract_first_json_object_returns_none_when_no_json() {
        assert!(extract_first_json_object("no braces here").is_none());
    }
}
