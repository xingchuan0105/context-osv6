//! LLM-as-judge evaluator and response parsing.

use super::runner::Evaluator;
use super::types::{EvalCase, EvalMetric, EvalScore};

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
pub(crate) fn parse_llm_judge_output(content: &str) -> Result<(f64, Option<String>), String> {
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
pub(crate) fn extract_first_json_object(text: &str) -> Option<&str> {
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

#[cfg(test)]
mod tests {
    use super::*;

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
