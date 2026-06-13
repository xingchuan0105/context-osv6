//! Built-in evaluators and scoring helpers.

use super::types::{EvalCase, EvalMetric, EvalScore};

/// Pluggable evaluator interface.
#[async_trait::async_trait]
pub trait Evaluator: Send + Sync {
    /// Evaluate a single case and return a score.
    async fn evaluate(&self, case: &EvalCase) -> Result<EvalScore, common::AppError>;
}

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

pub(crate) fn normalize(text: &str) -> String {
    text.to_lowercase()
        .replace(|c: char| c.is_ascii_punctuation(), " ")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

pub(crate) fn tokenize(text: &str) -> Vec<String> {
    normalize(text)
        .split_whitespace()
        .map(|s| s.to_string())
        .collect()
}

pub(crate) fn compute_f1(pred: &[String], reference: &[String]) -> f64 {
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
