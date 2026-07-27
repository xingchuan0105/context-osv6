//! Judge JSON output parsing (design §4.2/§4.3).
//!
//! The judge is instructed to emit a single JSON object; `parse_judge_output`
//! tolerates markdown fences via `crate::judge::extract_first_json_object` and
//! validates the v2 schema. All five dimension blocks are required. A parse
//! failure is structured (`JudgeParseError`) so the caller can map it to
//! `judge_status = error` / `LabelV2::JudgeError` instead of auto-PASSing.

use serde::{Deserialize, Serialize};

use super::judge_prompt::SCHEMA_VERSION;

/// Full v2 judge output (design §4.2 schema).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JudgeOutput {
    /// Prompt/schema version the judge echoed back. Empty = unversioned
    /// (tolerated); a set-but-different value is a parse error.
    #[serde(default)]
    pub schema_version: String,
    pub refusal: RefusalJudgment,
    pub answer_correctness: CorrectnessJudgment,
    pub faithfulness: FaithfulnessJudgment,
    pub answer_relevancy: RelevancyJudgment,
    pub context_sufficiency: SufficiencyJudgment,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RefusalJudgment {
    pub is_refusal: bool,
    pub correct_for_expectation: bool,
    pub score: f64,
    #[serde(default)]
    pub rationale: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CorrectnessJudgment {
    pub score: f64,
    pub verdict: CorrectnessVerdict,
    #[serde(default)]
    pub rationale: String,
    #[serde(default)]
    pub key_points_hit: Vec<String>,
    #[serde(default)]
    pub key_points_missed: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CorrectnessVerdict {
    Correct,
    Partial,
    Incorrect,
    NotApplicable,
    /// Verdict outside the documented set (tolerated, not an error).
    #[serde(other)]
    Unknown,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FaithfulnessJudgment {
    pub score: f64,
    pub verdict: FaithfulnessVerdict,
    #[serde(default)]
    pub unsupported_claims: Vec<String>,
    #[serde(default)]
    pub rationale: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FaithfulnessVerdict {
    Grounded,
    Mixed,
    Ungrounded,
    NotApplicable,
    /// Verdict outside the documented set (tolerated, not an error).
    #[serde(other)]
    Unknown,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RelevancyJudgment {
    pub score: f64,
    #[serde(default)]
    pub rationale: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SufficiencyJudgment {
    pub score: f64,
    pub verdict: SufficiencyVerdict,
    #[serde(default)]
    pub rationale: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SufficiencyVerdict {
    Sufficient,
    Partial,
    Insufficient,
    Unknown,
    /// Verdict outside the documented set (tolerated, not an error).
    #[serde(other)]
    Other,
}

/// Structured judge-parse failure (design §4.3: caller maps to judge_status
/// error; the query must not auto-PASS).
#[derive(Debug, thiserror::Error)]
pub enum JudgeParseError {
    #[error("judge response contained no JSON object")]
    NoJsonObject,
    #[error("judge JSON failed schema validation: {0}")]
    InvalidJson(String),
    #[error("judge JSON schema_version mismatch: expected {expected:?}, got {actual:?}")]
    VersionMismatch { expected: &'static str, actual: String },
}

/// Extract and validate the judge's JSON output.
///
/// Parses via `serde_json::Value` first: duplicate object keys (observed in
/// real Flash output, e.g. a repeated `rationale`) are last-wins there, while
/// deserializing straight into `JudgeOutput` hard-fails on them.
pub fn parse_judge_output(raw: &str) -> Result<JudgeOutput, JudgeParseError> {
    let json =
        crate::judge::extract_first_json_object(raw).ok_or(JudgeParseError::NoJsonObject)?;
    let value: serde_json::Value =
        serde_json::from_str(json).map_err(|e| JudgeParseError::InvalidJson(e.to_string()))?;
    let output: JudgeOutput =
        serde_json::from_value(value).map_err(|e| JudgeParseError::InvalidJson(e.to_string()))?;
    if !output.schema_version.is_empty() && output.schema_version != SCHEMA_VERSION {
        return Err(JudgeParseError::VersionMismatch {
            expected: SCHEMA_VERSION,
            actual: output.schema_version,
        });
    }
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;

    const FULL_SCHEMA_JSON: &str = r#"{
        "schema_version": "rag_eval_judge_v2",
        "refusal": {
            "is_refusal": false,
            "correct_for_expectation": true,
            "score": 1.0,
            "rationale": "模型正常作答，符合预期"
        },
        "answer_correctness": {
            "score": 0.9,
            "verdict": "correct",
            "rationale": "语义等价，仅空格差异",
            "key_points_hit": ["2019年建厂", "大连"],
            "key_points_missed": []
        },
        "faithfulness": {
            "score": 0.8,
            "verdict": "grounded",
            "unsupported_claims": [],
            "rationale": "所有事实均有引用支持"
        },
        "answer_relevancy": {
            "score": 1.0,
            "rationale": "直接回答了问题"
        },
        "context_sufficiency": {
            "score": 0.7,
            "verdict": "sufficient",
            "rationale": "证据充分"
        }
    }"#;

    #[test]
    fn parses_full_schema() {
        let out = parse_judge_output(FULL_SCHEMA_JSON).unwrap();
        assert_eq!(out.schema_version, SCHEMA_VERSION);
        assert!(!out.refusal.is_refusal);
        assert!(out.refusal.correct_for_expectation);
        assert_eq!(out.answer_correctness.verdict, CorrectnessVerdict::Correct);
        assert_eq!(out.answer_correctness.key_points_hit.len(), 2);
        assert_eq!(out.faithfulness.verdict, FaithfulnessVerdict::Grounded);
        assert!((out.answer_relevancy.score - 1.0).abs() < 1e-9);
        assert_eq!(out.context_sufficiency.verdict, SufficiencyVerdict::Sufficient);
    }

    #[test]
    fn parses_json_wrapped_in_markdown_fences() {
        let fenced = format!("```json\n{FULL_SCHEMA_JSON}\n```");
        let out = parse_judge_output(&fenced).unwrap();
        assert_eq!(out.answer_correctness.verdict, CorrectnessVerdict::Correct);
    }

    #[test]
    fn missing_required_block_errors() {
        // `context_sufficiency` dropped — serde must reject.
        let broken = r#"{
            "refusal": {"is_refusal": false, "correct_for_expectation": true, "score": 1.0},
            "answer_correctness": {"score": 0.9, "verdict": "correct"},
            "faithfulness": {"score": 0.8, "verdict": "grounded"},
            "answer_relevancy": {"score": 1.0}
        }"#;
        let err = parse_judge_output(broken).unwrap_err();
        assert!(
            err.to_string().contains("context_sufficiency"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn response_without_json_object_errors() {
        let err = parse_judge_output("I cannot grade this.").unwrap_err();
        assert!(matches!(err, JudgeParseError::NoJsonObject));
    }

    #[test]
    fn unknown_verdict_is_tolerated_not_an_error() {
        let raw = FULL_SCHEMA_JSON.replace("\"correct\"", "\"mostly_right\"");
        let out = parse_judge_output(&raw).unwrap();
        assert_eq!(out.answer_correctness.verdict, CorrectnessVerdict::Unknown);
    }

    #[test]
    fn missing_faithfulness_block_errors() {
        // Valid JSON, but the `faithfulness` block is dropped — the parser
        // must fail (never silent-pass), naming the missing block.
        let raw = FULL_SCHEMA_JSON.replace(
            r#""faithfulness": {
            "score": 0.8,
            "verdict": "grounded",
            "unsupported_claims": [],
            "rationale": "所有事实均有引用支持"
        },
        "#,
            "",
        );
        assert!(!raw.contains("faithfulness"), "test setup must drop the block");
        let err = parse_judge_output(&raw).unwrap_err();
        assert!(
            err.to_string().contains("faithfulness"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn mismatched_schema_version_errors() {
        let raw = FULL_SCHEMA_JSON.replace(SCHEMA_VERSION, "rag_eval_judge_v1");
        let err = parse_judge_output(&raw).unwrap_err();
        assert!(matches!(err, JudgeParseError::VersionMismatch { .. }));
    }

    #[test]
    fn duplicate_field_is_last_wins_not_an_error() {
        // Regression for a real Flash output (run v2_20260727-022503 q030):
        // the model repeated `rationale` inside one block and the strict
        // struct parse failed with "duplicate field". Value-first parsing is
        // last-wins tolerant.
        let raw = FULL_SCHEMA_JSON.replace(
            r#""rationale": "模型正常作答，符合预期""#,
            r#""rationale": "第一版理由（应被覆盖）", "rationale": "模型正常作答，符合预期""#,
        );
        assert!(raw.matches("\"rationale\"").count() > 5, "test setup must duplicate a key");
        let out = parse_judge_output(&raw).unwrap();
        assert_eq!(out.refusal.rationale, "模型正常作答，符合预期");
    }
}
