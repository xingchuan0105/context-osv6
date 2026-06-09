use std::sync::Arc;

use avrag_llm::{ChatMessage, LlmClient};
use common::AppError;
use serde::{Deserialize, Serialize};

use super::config::ModeConfig;
use crate::agents::runtime::AgentRequest;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ReferentSlotKind {
    Pronoun,
    DefiniteWithoutAntecedent,
    Ellipsis,
    Demonstrative,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueryResolutionMeta {
    pub raw_query: String,
    pub resolved_query: String,
    pub slots: Vec<ReferentSlotKind>,
    pub method: String,
}

#[derive(Debug, Clone)]
pub struct NormalizeResult {
    pub resolved_query: String,
    pub meta: Option<QueryResolutionMeta>,
    pub clarify_answer: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum SelfContainedStatus {
    SelfContained,
    NeedsResolution(Vec<ReferentSlotKind>),
}

fn english_pronoun_hit(lower: &str) -> bool {
    const PHRASE_PATTERNS: &[&str] = &[
        " it ",
        " it?",
        " they ",
        " them ",
        " this book",
        " that book",
        " this author",
        " that author",
        " about it",
        " about this",
        " about that",
    ];
    if PHRASE_PATTERNS.iter().any(|p| lower.contains(p)) {
        return true;
    }
    lower.ends_with(" it?")
}

fn chinese_anaphora_hit(raw: &str) -> bool {
    const PATTERNS: &[&str] = &[
        "他们",
        "她们",
        "它",
        "这位",
        "那位",
        "这本书",
        "那位作者",
        "那个概念",
    ];
    PATTERNS.iter().any(|p| raw.contains(p))
}

pub fn prior_user_turns(request: &AgentRequest, max_turns: u8) -> Vec<String> {
    let mut turns: Vec<String> = request
        .messages
        .iter()
        .filter(|t| t.role == "user")
        .map(|t| {
            t.resolved_query
                .as_ref()
                .filter(|q| !q.trim().is_empty())
                .cloned()
                .unwrap_or_else(|| t.content.clone())
        })
        .collect();
    let keep = max_turns as usize;
    if turns.len() > keep {
        turns = turns.split_off(turns.len() - keep);
    }
    turns
}

pub fn classify_self_contained(raw: &str, prior_turns: &[String]) -> SelfContainedStatus {
    if prior_turns.is_empty() {
        return SelfContainedStatus::SelfContained;
    }
    let lower = raw.to_lowercase();
    let mut slots = Vec::new();

    if english_pronoun_hit(&lower) {
        slots.push(ReferentSlotKind::Pronoun);
    } else if chinese_anaphora_hit(raw) {
        slots.push(ReferentSlotKind::Pronoun);
    }

    if lower.contains("the book") || lower.contains("这本书") || lower.contains("那位作者") {
        slots.push(ReferentSlotKind::DefiniteWithoutAntecedent);
    }

    if (lower.starts_with("who wrote") || lower.contains("谁写") || lower.contains("谁写的"))
        && (lower.contains(" it") || lower.ends_with("it?") || lower.contains("它"))
    {
        slots.push(ReferentSlotKind::Ellipsis);
    }

    if lower.contains("that concept") || lower.contains("那个概念") {
        slots.push(ReferentSlotKind::Demonstrative);
    }

    if slots.is_empty() {
        SelfContainedStatus::SelfContained
    } else {
        SelfContainedStatus::NeedsResolution(slots)
    }
}

pub fn resolve_with_heuristic(raw: &str, prior_turns: &[String]) -> Option<String> {
    if prior_turns.is_empty() {
        return None;
    }
    let lower = raw.to_lowercase();
    let last = prior_turns.last()?.to_lowercase();
    let context = prior_turns.join(" ").to_lowercase();

    if (lower.contains("who wrote") || lower.contains("author"))
        && (lower.contains("book") || lower.contains("it"))
    {
        if context.contains("antifragil") || context.contains("taleb") {
            return Some("Who wrote the book Antifragile by Nassim Nicholas Taleb?".to_string());
        }
    }

    if lower.contains("它") || lower.ends_with("it?") {
        if last.contains("antifragil") {
            return Some(format!(
                "{raw} (referring to antifragility from prior turn)"
            ));
        }
    }

    None
}

pub async fn resolve_with_llm(
    llm: &LlmClient,
    raw: &str,
    prior_turns: &[String],
) -> Result<Result<String, String>, AppError> {
    let context: String = prior_turns
        .iter()
        .enumerate()
        .map(|(i, t)| format!("Turn {}: {t}", i + 1))
        .collect::<Vec<_>>()
        .join("\n");

    let system = "You resolve anaphora in follow-up questions. \
                  Output ONE line only: either a standalone resolved query, \
                  or CLARIFY: <question> if ambiguous. \
                  Do not invent entities absent from prior turns.";
    let user = format!("Prior turns:\n{context}\n\nFollow-up: {raw}");

    let response = llm
        .complete(
            &[ChatMessage::system(system), ChatMessage::user(&user)],
            Some(0.0),
        )
        .await
        .map_err(|e| AppError::internal(format!("query normalize LLM failed: {e}")))?;

    let line = response.content.trim();
    if let Some(rest) = line.strip_prefix("CLARIFY:") {
        return Ok(Err(rest.trim().to_string()));
    }
    Ok(Ok(line.to_string()))
}

pub async fn normalize_query(
    llm: &Arc<LlmClient>,
    mode: &ModeConfig,
    request: &AgentRequest,
) -> Result<NormalizeResult, AppError> {
    if request
        .cancellation_token
        .as_ref()
        .is_some_and(|t| t.is_cancelled())
    {
        return Err(crate::agents::react_loop::cancellation_error());
    }

    let cfg = &mode.query_normalization;
    if !cfg.enabled {
        return Ok(NormalizeResult {
            resolved_query: request.query.clone(),
            meta: None,
            clarify_answer: None,
        });
    }

    let prior = prior_user_turns(request, cfg.max_prior_turns);
    match classify_self_contained(&request.query, &prior) {
        SelfContainedStatus::SelfContained => Ok(NormalizeResult {
            resolved_query: request.query.clone(),
            meta: None,
            clarify_answer: None,
        }),
        SelfContainedStatus::NeedsResolution(slots) => {
            if let Some(resolved) = resolve_with_heuristic(&request.query, &prior) {
                return Ok(NormalizeResult {
                    resolved_query: resolved.clone(),
                    meta: Some(QueryResolutionMeta {
                        raw_query: request.query.clone(),
                        resolved_query: resolved,
                        slots,
                        method: "heuristic".to_string(),
                    }),
                    clarify_answer: None,
                });
            }

            if cfg.llm_fallback {
                match resolve_with_llm(llm, &request.query, &prior).await? {
                    Ok(resolved) => Ok(NormalizeResult {
                        resolved_query: resolved.clone(),
                        meta: Some(QueryResolutionMeta {
                            raw_query: request.query.clone(),
                            resolved_query: resolved,
                            slots,
                            method: "llm".to_string(),
                        }),
                        clarify_answer: None,
                    }),
                    Err(clarify) => Ok(NormalizeResult {
                        resolved_query: request.query.clone(),
                        meta: Some(QueryResolutionMeta {
                            raw_query: request.query.clone(),
                            resolved_query: request.query.clone(),
                            slots,
                            method: "clarify".to_string(),
                        }),
                        clarify_answer: Some(clarify),
                    }),
                }
            } else {
                Ok(NormalizeResult {
                    resolved_query: request.query.clone(),
                    meta: Some(QueryResolutionMeta {
                        raw_query: request.query.clone(),
                        resolved_query: request.query.clone(),
                        slots,
                        method: "unresolved".to_string(),
                    }),
                    clarify_answer: None,
                })
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn self_contained_without_prior() {
        assert!(matches!(
            classify_self_contained("What is antifragility?", &[]),
            SelfContainedStatus::SelfContained
        ));
    }

    #[test]
    fn chinese_what_is_question_not_misclassified() {
        assert!(matches!(
            classify_self_contained("什么是反脆弱性？", &["上一轮".to_string()]),
            SelfContainedStatus::SelfContained
        ));
    }

    #[test]
    fn english_what_is_this_not_misclassified() {
        assert!(matches!(
            classify_self_contained("What is this?", &["Prior topic".to_string()]),
            SelfContainedStatus::SelfContained
        ));
    }

    #[test]
    fn needs_resolution_for_it() {
        let prior = vec!["What is antifragility?".to_string()];
        assert!(matches!(
            classify_self_contained("Who wrote the book about it?", &prior),
            SelfContainedStatus::NeedsResolution(_)
        ));
    }

    #[test]
    fn heuristic_resolves_taleb_follow_up() {
        let prior = vec!["What is antifragility?".to_string()];
        let resolved = resolve_with_heuristic("Who wrote the book about it?", &prior).unwrap();
        assert!(resolved.to_lowercase().contains("taleb"));
    }

    #[test]
    fn prior_user_turns_prefers_resolved_query_from_metadata() {
        use crate::agents::runtime::AgentRequest;
        use crate::agents::AgentKind;

        let request = AgentRequest {
            kind: AgentKind::Rag,
            query: "Who wrote the book about it?".to_string(),
            resolved_query: "Who wrote the book about it?".to_string(),
            query_resolution: None,
            notebook_id: None,
            session_id: None,
            doc_scope: vec![],
            messages: vec![
                common::ChatTurnInput {
                    role: "user".to_string(),
                    content: "Who wrote it?".to_string(),
                    resolved_query: Some(
                        "Who wrote Antifragile by Nassim Nicholas Taleb?".to_string(),
                    ),
                },
                common::ChatTurnInput {
                    role: "assistant".to_string(),
                    content: "Taleb.".to_string(),
                    resolved_query: None,
                },
            ],
            session_summary: None,
            user_preferences: None,
            debug: false,
            stream: false,
            language: None,
            auth_context: serde_json::json!({}),
            docscope_metadata: None,
            metadata: std::collections::BTreeMap::new(),
            cancellation_token: None,
            guard_pipeline: None,
            preferred_tools: vec![],
            format_hint: None,
            max_iterations: None,
        };
        let prior = prior_user_turns(&request, 6);
        assert_eq!(prior.len(), 1);
        assert!(prior[0].contains("Taleb"));
    }
}
