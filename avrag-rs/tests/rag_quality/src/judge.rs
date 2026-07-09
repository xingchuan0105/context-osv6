//! Faithfulness judges for RAG answer evaluation.
//!
//! Smoke tests should use `SubstringFaithfulnessJudge` (fast, deterministic,
//! no API). Regression/calibration runs can use `LlmNliJudge` for semantic
//! claim support checks, but only after calibrating against human labels.

use crate::harness_extract::CitedChunks;
use crate::metrics_v2::{FaithfulnessReport, substring_faithfulness};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FaithfulnessInput {
    pub query: String,
    pub answer: String,
    pub cited_chunks: CitedChunks,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FaithfulnessJudgment {
    pub query: String,
    pub faithfulness: f64,
    pub total_claims: usize,
    pub supported_claims: usize,
    pub unsupported_claims: Vec<String>,
    pub raw_judge_output: Option<String>,
}

impl From<FaithfulnessReport> for FaithfulnessJudgment {
    fn from(report: FaithfulnessReport) -> Self {
        Self {
            query: report.query,
            faithfulness: report.faithfulness,
            total_claims: report.total_claims,
            supported_claims: report.supported_claims,
            unsupported_claims: report.unsupported_claims,
            raw_judge_output: None,
        }
    }
}

#[async_trait]
pub trait FaithfulnessJudge: Send + Sync {
    async fn judge(&self, input: &FaithfulnessInput) -> anyhow::Result<FaithfulnessJudgment>;
}

/// Fast deterministic judge for smoke runs.
pub struct SubstringFaithfulnessJudge;

#[async_trait]
impl FaithfulnessJudge for SubstringFaithfulnessJudge {
    async fn judge(&self, input: &FaithfulnessInput) -> anyhow::Result<FaithfulnessJudgment> {
        let mut report = substring_faithfulness(&input.answer, &input.cited_chunks);
        report.query = input.query.clone();
        Ok(report.into())
    }
}

/// LLM-as-Judge semantic faithfulness checker.
///
/// The judge asks an LLM to decompose the answer into atomic claims and check
/// whether each claim is supported by the cited context. It uses temperature 0
/// and requires a JSON object response. This is intended for regression /
/// calibration runs, not fast smoke loops.
pub struct LlmNliJudge {
    llm: avrag_llm::LlmClient,
}

impl LlmNliJudge {
    pub fn new(llm: avrag_llm::LlmClient) -> Self {
        Self { llm }
    }

    /// Build from the existing agent LLM environment variables.
    ///
    /// The project rule is to reuse configured `AGENT_LLM_*` values instead of
    /// asking the user for new credentials.
    pub fn from_agent_env() -> anyhow::Result<Self> {
        let base_url = std::env::var("AGENT_LLM_BASE_URL")?;
        let api_key = std::env::var("AGENT_LLM_API_KEY")?;
        let model = std::env::var("AGENT_LLM_MODEL")?;
        Ok(Self::new(avrag_llm::LlmClient::new(
            avrag_llm::ModelProviderConfig {
                base_url,
                api_key,
                model,
                timeout_ms: 60_000,
                api_style: None,
                dimensions: None,
                enable_thinking: Some(false),
                enable_cache: Some(false),
                rpm_limit: None,
                tpm_limit: None,
            },
        )))
    }
}

#[async_trait]
impl FaithfulnessJudge for LlmNliJudge {
    async fn judge(&self, input: &FaithfulnessInput) -> anyhow::Result<FaithfulnessJudgment> {
        let context = input.cited_chunks.contents().join("\n\n---\n\n");
        let user = format!(
            "Question:\n{}\n\nAnswer:\n{}\n\nCited context:\n{}\n\n\
             Task:\n\
             1. Split the answer into atomic factual claims.\n\
             2. For each claim, decide whether the cited context supports it.\n\
             3. Return ONLY valid JSON with this shape:\n\
             {{\"claims\":[{{\"text\":\"...\",\"supported\":true,\"evidence_span\":\"...\"}}]}}\n\
             Use Chinese claim text when the answer is Chinese. If no factual claims exist, return an empty claims array.",
            input.query, input.answer, context
        );
        let messages = vec![
            avrag_llm::ChatMessage::system(
                "You are a strict RAG faithfulness judge. Respond only with valid JSON.",
            ),
            avrag_llm::ChatMessage::user(user),
        ];
        let response = self.llm.complete(&messages, Some(0.0)).await?;
        let output = parse_llm_nli_output(&response.content)?;
        Ok(judgment_from_output(
            input.query.clone(),
            output,
            Some(response.content),
        ))
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmNliClaim {
    pub text: String,
    pub supported: bool,
    #[serde(default)]
    pub evidence_span: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmNliOutput {
    #[serde(default)]
    pub claims: Vec<LlmNliClaim>,
}

pub fn parse_llm_nli_output(content: &str) -> anyhow::Result<LlmNliOutput> {
    let json = extract_first_json_object(content)
        .ok_or_else(|| anyhow::anyhow!("LLM judge response did not contain a JSON object"))?;
    serde_json::from_str(json).map_err(|e| anyhow::anyhow!("invalid LLM judge JSON: {e}"))
}

fn judgment_from_output(
    query: String,
    output: LlmNliOutput,
    raw_judge_output: Option<String>,
) -> FaithfulnessJudgment {
    let total = output.claims.len();
    let supported = output.claims.iter().filter(|c| c.supported).count();
    let unsupported_claims = output
        .claims
        .iter()
        .filter(|c| !c.supported)
        .map(|c| c.text.clone())
        .collect::<Vec<_>>();
    FaithfulnessJudgment {
        query,
        faithfulness: if total == 0 {
            1.0
        } else {
            supported as f64 / total as f64
        },
        total_claims: total,
        supported_claims: supported,
        unsupported_claims,
        raw_judge_output,
    }
}

/// Extract the first top-level JSON object from LLM output, tolerating markdown fences.
pub fn extract_first_json_object(text: &str) -> Option<&str> {
    if let Some(start) = text.find("```json") {
        let after = &text[start + 7..];
        if let Some(end) = after.find("```") {
            return Some(after[..end].trim());
        }
    }
    if let Some(start) = text.find("```") {
        let after = &text[start + 3..];
        if let Some(end) = after.find("```") {
            let candidate = after[..end].trim();
            if candidate.starts_with('{') {
                return Some(candidate);
            }
        }
    }

    let mut depth = 0usize;
    let mut start = None;
    for (idx, ch) in text.char_indices() {
        match ch {
            '{' => {
                if depth == 0 {
                    start = Some(idx);
                }
                depth += 1;
            }
            '}' => {
                if depth > 0 {
                    depth -= 1;
                    if depth == 0 {
                        if let Some(s) = start {
                            return Some(&text[s..=idx]);
                        }
                    }
                }
            }
            _ => {}
        }
    }
    None
}

/// Cohen's kappa for binary faithful/unfaithful labels.
///
/// `manual` and `predicted` are parallel boolean arrays where `true` means
/// faithful. Returns `None` for empty/mismatched input or undefined expected
/// agreement.
pub fn cohen_kappa_binary(manual: &[bool], predicted: &[bool]) -> Option<f64> {
    if manual.is_empty() || manual.len() != predicted.len() {
        return None;
    }
    let n = manual.len() as f64;
    let observed = manual.iter().zip(predicted).filter(|(a, b)| a == b).count() as f64 / n;

    let manual_true = manual.iter().filter(|v| **v).count() as f64 / n;
    let manual_false = 1.0 - manual_true;
    let pred_true = predicted.iter().filter(|v| **v).count() as f64 / n;
    let pred_false = 1.0 - pred_true;
    let expected = manual_true * pred_true + manual_false * pred_false;
    if (1.0 - expected).abs() < f64::EPSILON {
        return None;
    }
    Some((observed - expected) / (1.0 - expected))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::harness_extract::{CitedChunk, CitedChunks};

    #[test]
    fn parses_fenced_llm_nli_json() {
        let output = parse_llm_nli_output(
            r#"```json
{"claims":[{"text":"2019年建厂","supported":true,"evidence_span":"2019年于大连市投资建厂"},{"text":"638人","supported":false,"evidence_span":""}]}
```"#,
        )
        .unwrap();
        assert_eq!(output.claims.len(), 2);
        let judgment = judgment_from_output("q".to_string(), output, None);
        assert_eq!(judgment.supported_claims, 1);
        assert_eq!(judgment.unsupported_claims, vec!["638人".to_string()]);
        assert!((judgment.faithfulness - 0.5).abs() < 1e-9);
    }

    #[tokio::test]
    async fn substring_judge_is_deterministic() {
        let input = FaithfulnessInput {
            query: "q".to_string(),
            answer: "2019年建厂，638人".to_string(),
            cited_chunks: CitedChunks {
                chunks: vec![CitedChunk {
                    chunk_id: Some("c1".to_string()),
                    citation_id: 1,
                    content: "2019年于大连市投资建厂".to_string(),
                    score: 1.0,
                }],
            },
        };
        let judge = SubstringFaithfulnessJudge;
        let judgment = judge.judge(&input).await.unwrap();
        assert_eq!(judgment.query, "q");
        assert!(judgment.faithfulness < 1.0);
        assert!(judgment.unsupported_claims.contains(&"638".to_string()));
    }

    #[test]
    fn cohen_kappa_binary_scores_agreement() {
        let manual = [true, true, false, false];
        let predicted = [true, false, false, false];
        let kappa = cohen_kappa_binary(&manual, &predicted).unwrap();
        assert!(kappa > 0.0 && kappa < 1.0);
        assert_eq!(cohen_kappa_binary(&manual, &manual), Some(1.0));
    }
}
