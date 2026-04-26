use std::collections::{HashMap, HashSet};

use avrag_llm::{ChatMessage as LlmChatMessage, LlmClient, LlmUsage};
use common::{
    AnswerContextChunk, ChatRequest, ChatResponse, DegradeTraceItem, ExecutePlanItem,
    ExecutePlanRequest, ExecutePlanResponse, ExecutePlanSummaryMode, ModeDebug, PlannerOutput,
    RagModeDebug, SourceRef, SummaryInjectionTrace, TraceInfo,
};
use uuid::Uuid;

const RAG_EXECUTE_PLAN_VERSION: &str = "rag-execute-v1";
const NO_VALID_RETRIEVAL_RESULTS_ANSWER: &str =
    "未找到足够的证据来回答您的问题。请尝试更换关键词或上传更多相关文档。";
const ANSWER_UNAVAILABLE_WITH_EVIDENCE: &str =
    "Answer generation is currently unavailable even though relevant evidence was retrieved.";
const GENERAL_UNAVAILABLE_ANSWER: &str = "Network is unstable. Please try again later.";

const GENERAL_SYSTEM_PROMPT: &str = "You are the general assistant for Context OS. Maintain continuity across turns, use conversation memory when relevant, and answer directly without inventing facts.";

const RAG_PLAN_SYSTEM_PROMPT: &str = r#"You are the Context OS Main Agent in RAG planning mode.
Return exactly one raw JSON object.

When retrieval can proceed, return an ExecutePlanRequest:
{
  "plan_version": "rag-execute-v1",
  "doc_scope": ["document-id"],
  "items": [
    { "priority": 1.0, "query": "semantic retrieval query" }
  ],
  "summary_mode": "none" | "related" | "all"
}

When the target cannot be identified from the current task, doc_scope, document metadata, and reference context, return:
{
  "action": "clarify",
  "message": "one concise clarification question"
}

Rules:
- Do not answer the user.
- Do not include session_id, messages, history, clarify_needed, or clarify_message.
- Keep doc_scope exactly equal to the provided doc_scope.
- Use 1 to 4 retrieval items.
- Each item must contain exactly one of query or bm25_terms.
- Prefer one high-priority semantic query; add bm25_terms only for filenames, exact names, codes, or rare terms.
- Use summary_mode "related" when document summaries may help answer the question; otherwise use "none".
- Ask for clarification only when multiple plausible targets remain or a required scope/entity/time range is missing.
- Never ask for clarification only because a previous assistant message said retrieval failed.
"#;

const RAG_ANSWER_SYSTEM_PROMPT: &str = r#"You are the Context OS Main Agent in RAG answer mode.
Answer the user's question using only the retrieval bundle.
Do not mention internal planning, tool calls, or hidden reasoning.
Do not output JSON.
Do not output markdown code fences.
Do not include inline citation markers, chunk ids, or source ids.
Reply in the same language as the user's question unless the conversation strongly indicates another language.
If the evidence is partial, answer only the grounded portion and clearly note what remains uncertain.
If the evidence is insufficient, say so plainly and suggest how the user can refine the request.
"#;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModeProfile {
    General,
    Rag,
    Search,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MainAgentDecision {
    Clarify { message: String },
    ExecutePlan,
    DirectChat,
    ExternalSearch,
}

#[derive(Debug, Clone, Default)]
pub struct MainAgentReferenceContext {
    pub session_working_state: Option<String>,
    pub recent_user_turns: Vec<String>,
    pub user_preferences: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct MainAgentContext {
    pub mode: String,
    pub current_task: String,
    pub authoritative_context: String,
    pub reference_context: String,
    pub user_preference_memory: String,
    pub skill: String,
    pub output_contract: String,
}

#[derive(Debug, Clone)]
pub enum MainAgentRagPlanDecision {
    Execute(ExecutePlanRequest),
    Clarify(String),
}

#[derive(Debug, Default, Clone, Copy)]
pub struct MainAgent;

#[derive(Debug, Clone)]
pub struct MainAgentPlanResult {
    pub decision: MainAgentRagPlanDecision,
    pub llm_usage: Option<LlmUsage>,
}

#[derive(Debug, Clone)]
pub struct MainAgentAnswerResult {
    pub answer_text: String,
    pub llm_usage: Option<LlmUsage>,
}

impl MainAgent {
    pub fn profile(agent_type: &str) -> ModeProfile {
        match agent_type {
            "search" => ModeProfile::Search,
            "rag" => ModeProfile::Rag,
            _ => ModeProfile::General,
        }
    }

    pub fn decide(request: &ChatRequest) -> MainAgentDecision {
        match Self::profile(&request.agent_type) {
            ModeProfile::General => MainAgentDecision::DirectChat,
            ModeProfile::Search => MainAgentDecision::ExternalSearch,
            ModeProfile::Rag => {
                if request.doc_scope.is_empty() {
                    MainAgentDecision::Clarify {
                        message: "请先选择要检索的文档范围，再让我执行知识库检索。".to_string(),
                    }
                } else {
                    MainAgentDecision::ExecutePlan
                }
            }
        }
    }

    pub fn answer_context(response: &ExecutePlanResponse) -> Vec<AnswerContextChunk> {
        response.bundle.answer_context_chunks()
    }

    pub fn general_system_message() -> LlmChatMessage {
        LlmChatMessage::system(GENERAL_SYSTEM_PROMPT)
    }

    pub fn build_general_messages(
        current_task: &str,
        reference_context: Option<&MainAgentReferenceContext>,
    ) -> Vec<LlmChatMessage> {
        let reference = reference_context_section(reference_context);
        let preferences = reference_context
            .map(|context| {
                if context.user_preferences.is_empty() {
                    "none".to_string()
                } else {
                    context.user_preferences.join("\n")
                }
            })
            .unwrap_or_else(|| "none".to_string());
        let envelope = build_main_agent_envelope(MainAgentContext {
            mode: "general_chat".to_string(),
            current_task: current_task.trim().to_string(),
            authoritative_context: "none".to_string(),
            reference_context: reference,
            user_preference_memory: preferences,
            skill: "Answer directly while preserving conversational continuity. Do not treat reference context as factual evidence for document claims.".to_string(),
            output_contract: "Return a natural-language answer only.".to_string(),
        });

        vec![
            Self::general_system_message(),
            LlmChatMessage::user(envelope),
        ]
    }

    pub async fn answer_general(
        llm: Option<&LlmClient>,
        messages: &[LlmChatMessage],
        temperature: Option<f32>,
        degrade_trace: &mut Vec<DegradeTraceItem>,
    ) -> MainAgentAnswerResult {
        let Some(llm) = llm else {
            degrade_trace.push(DegradeTraceItem {
                stage: "main_agent.general_answer".to_string(),
                reason: "answer_llm_not_configured".to_string(),
                impact: "Returned retry hint to user".to_string(),
            });
            return MainAgentAnswerResult {
                answer_text: GENERAL_UNAVAILABLE_ANSWER.to_string(),
                llm_usage: None,
            };
        };

        match llm.complete(messages, temperature).await {
            Ok(response) => MainAgentAnswerResult {
                answer_text: response.content,
                llm_usage: Some(response.usage),
            },
            Err(error) => {
                degrade_trace.push(DegradeTraceItem {
                    stage: "main_agent.general_answer".to_string(),
                    reason: format!("llm_error: {error}"),
                    impact: "Returned retry hint to user".to_string(),
                });
                MainAgentAnswerResult {
                    answer_text: GENERAL_UNAVAILABLE_ANSWER.to_string(),
                    llm_usage: None,
                }
            }
        }
    }

    pub async fn answer_general_stream(
        llm: &LlmClient,
        messages: &[LlmChatMessage],
        temperature: Option<f32>,
        on_delta: impl FnMut(&str),
    ) -> anyhow::Result<avrag_llm::LlmResponse> {
        llm.complete_stream(messages, temperature, on_delta).await
    }

    pub async fn plan_rag(
        request: &ChatRequest,
        docscope_metadata: Option<&common::DocScopeMetadata>,
        reference_context: Option<&MainAgentReferenceContext>,
        llm: Option<&LlmClient>,
        temperature: Option<f32>,
        degrade_trace: &mut Vec<DegradeTraceItem>,
    ) -> MainAgentPlanResult {
        let fallback = || {
            if request.query.trim().is_empty() {
                return MainAgentRagPlanDecision::Clarify("请补充要检索的具体问题。".to_string());
            }
            if request.doc_scope.is_empty() {
                return MainAgentRagPlanDecision::Clarify("请先选择要检索的文档范围。".to_string());
            }
            MainAgentRagPlanDecision::Execute(fallback_execute_plan_request(
                request,
                docscope_metadata,
            ))
        };
        let Some(llm) = llm else {
            degrade_trace.push(DegradeTraceItem {
                stage: "main_agent.rag_plan".to_string(),
                reason: "answer_llm_not_configured".to_string(),
                impact: "Using deterministic single-query execute plan".to_string(),
            });
            return MainAgentPlanResult {
                decision: fallback(),
                llm_usage: None,
            };
        };

        let messages = vec![
            LlmChatMessage::system(RAG_PLAN_SYSTEM_PROMPT),
            LlmChatMessage::user(build_rag_plan_user_prompt(
                request,
                docscope_metadata,
                reference_context,
            )),
        ];

        match llm.complete(&messages, temperature.or(Some(0.1))).await {
            Ok(response) => {
                let llm_usage = Some(response.usage);
                match parse_rag_plan_decision(&response.content, request) {
                    Some(MainAgentRagPlanDecision::Execute(execute_request)) => {
                        MainAgentPlanResult {
                            decision: MainAgentRagPlanDecision::Execute(execute_request),
                            llm_usage,
                        }
                    }
                    Some(MainAgentRagPlanDecision::Clarify(message)) => MainAgentPlanResult {
                        decision: MainAgentRagPlanDecision::Clarify(message),
                        llm_usage,
                    },
                    None => {
                        degrade_trace.push(DegradeTraceItem {
                            stage: "main_agent.rag_plan".to_string(),
                            reason: "invalid_execute_plan_json".to_string(),
                            impact: "Using deterministic single-query execute plan".to_string(),
                        });
                        MainAgentPlanResult {
                            decision: fallback(),
                            llm_usage,
                        }
                    }
                }
            }
            Err(error) => {
                degrade_trace.push(DegradeTraceItem {
                    stage: "main_agent.rag_plan".to_string(),
                    reason: format!("llm_error: {error}"),
                    impact: "Using deterministic single-query execute plan".to_string(),
                });
                MainAgentPlanResult {
                    decision: fallback(),
                    llm_usage: None,
                }
            }
        }
    }

    pub async fn answer_rag(
        request: &ChatRequest,
        execute_request: &ExecutePlanRequest,
        execute_response: &ExecutePlanResponse,
        reference_context: Option<&MainAgentReferenceContext>,
        llm: Option<&LlmClient>,
        temperature: Option<f32>,
        degrade_trace: &mut Vec<DegradeTraceItem>,
    ) -> MainAgentAnswerResult {
        let answer_context = Self::answer_context(execute_response);
        if answer_context.is_empty() {
            return MainAgentAnswerResult {
                answer_text: NO_VALID_RETRIEVAL_RESULTS_ANSWER.to_string(),
                llm_usage: None,
            };
        }

        let Some(llm) = llm else {
            degrade_trace.push(DegradeTraceItem {
                stage: "main_agent.rag_answer".to_string(),
                reason: "answer_llm_not_configured".to_string(),
                impact: "Returning explicit synthesis-unavailable answer".to_string(),
            });
            return MainAgentAnswerResult {
                answer_text: ANSWER_UNAVAILABLE_WITH_EVIDENCE.to_string(),
                llm_usage: None,
            };
        };

        let messages = vec![
            LlmChatMessage::system(RAG_ANSWER_SYSTEM_PROMPT),
            LlmChatMessage::user(build_rag_answer_user_prompt(
                request,
                execute_request,
                execute_response,
                &answer_context,
                reference_context,
            )),
        ];

        match llm.complete(&messages, temperature).await {
            Ok(response) if !response.content.trim().is_empty() => MainAgentAnswerResult {
                answer_text: response.content.trim().to_string(),
                llm_usage: Some(response.usage),
            },
            Ok(response) => {
                degrade_trace.push(DegradeTraceItem {
                    stage: "main_agent.rag_answer".to_string(),
                    reason: "empty_llm_answer".to_string(),
                    impact: "Returning explicit synthesis-unavailable answer".to_string(),
                });
                MainAgentAnswerResult {
                    answer_text: ANSWER_UNAVAILABLE_WITH_EVIDENCE.to_string(),
                    llm_usage: Some(response.usage),
                }
            }
            Err(error) => {
                degrade_trace.push(DegradeTraceItem {
                    stage: "main_agent.rag_answer".to_string(),
                    reason: format!("llm_error: {error}"),
                    impact: "Returning explicit synthesis-unavailable answer".to_string(),
                });
                MainAgentAnswerResult {
                    answer_text: ANSWER_UNAVAILABLE_WITH_EVIDENCE.to_string(),
                    llm_usage: None,
                }
            }
        }
    }

    pub async fn answer_rag_stream(
        request: &ChatRequest,
        execute_request: &ExecutePlanRequest,
        execute_response: &ExecutePlanResponse,
        reference_context: Option<&MainAgentReferenceContext>,
        llm: Option<&LlmClient>,
        temperature: Option<f32>,
        degrade_trace: &mut Vec<DegradeTraceItem>,
        on_delta: impl FnMut(&str),
    ) -> MainAgentAnswerResult {
        let answer_context = Self::answer_context(execute_response);
        if answer_context.is_empty() {
            return MainAgentAnswerResult {
                answer_text: NO_VALID_RETRIEVAL_RESULTS_ANSWER.to_string(),
                llm_usage: None,
            };
        }

        let Some(llm) = llm else {
            degrade_trace.push(DegradeTraceItem {
                stage: "main_agent.rag_answer".to_string(),
                reason: "answer_llm_not_configured".to_string(),
                impact: "Returning explicit synthesis-unavailable answer".to_string(),
            });
            return MainAgentAnswerResult {
                answer_text: ANSWER_UNAVAILABLE_WITH_EVIDENCE.to_string(),
                llm_usage: None,
            };
        };

        let messages = vec![
            LlmChatMessage::system(RAG_ANSWER_SYSTEM_PROMPT),
            LlmChatMessage::user(build_rag_answer_user_prompt(
                request,
                execute_request,
                execute_response,
                &answer_context,
                reference_context,
            )),
        ];

        match llm.complete_stream(&messages, temperature, on_delta).await {
            Ok(response) if !response.content.trim().is_empty() => MainAgentAnswerResult {
                answer_text: response.content.trim().to_string(),
                llm_usage: Some(response.usage),
            },
            Ok(response) => {
                degrade_trace.push(DegradeTraceItem {
                    stage: "main_agent.rag_answer".to_string(),
                    reason: "empty_llm_answer".to_string(),
                    impact: "Returning explicit synthesis-unavailable answer".to_string(),
                });
                MainAgentAnswerResult {
                    answer_text: ANSWER_UNAVAILABLE_WITH_EVIDENCE.to_string(),
                    llm_usage: Some(response.usage),
                }
            }
            Err(error) => {
                degrade_trace.push(DegradeTraceItem {
                    stage: "main_agent.rag_answer".to_string(),
                    reason: format!("llm_error: {error}"),
                    impact: "Returning explicit synthesis-unavailable answer".to_string(),
                });
                MainAgentAnswerResult {
                    answer_text: ANSWER_UNAVAILABLE_WITH_EVIDENCE.to_string(),
                    llm_usage: None,
                }
            }
        }
    }

    pub fn build_rag_chat_response(
        request: &ChatRequest,
        resolved_session_id: Option<&str>,
        execute_request: &ExecutePlanRequest,
        execute_response: &ExecutePlanResponse,
        answer: MainAgentAnswerResult,
        degrade_trace: Vec<DegradeTraceItem>,
    ) -> ChatResponse {
        let cited_chunk_ids = extract_referenced_chunk_ids(&answer.answer_text);
        let ordered_chunks = if cited_chunk_ids.is_empty() {
            execute_response.bundle.chunks.clone()
        } else {
            let mut filtered = execute_response
                .bundle
                .chunks
                .iter()
                .filter(|chunk| cited_chunk_ids.contains(&chunk.chunk_id))
                .cloned()
                .collect::<Vec<_>>();
            if filtered.is_empty() {
                filtered = execute_response.bundle.chunks.clone();
            }
            filtered
        };

        let citation_by_chunk_id = execute_response
            .bundle
            .citations
            .iter()
            .filter_map(|citation| {
                citation
                    .chunk_id
                    .as_ref()
                    .map(|chunk_id| (chunk_id.clone(), citation.clone()))
            })
            .collect::<HashMap<_, _>>();

        let citations = ordered_chunks
            .iter()
            .enumerate()
            .filter_map(|(index, chunk)| {
                citation_by_chunk_id
                    .get(&chunk.chunk_id)
                    .cloned()
                    .map(|mut citation| {
                        citation.citation_id = (index + 1) as i64;
                        citation
                    })
            })
            .collect::<Vec<_>>();

        let sources = ordered_chunks
            .iter()
            .map(|chunk| {
                let title = citation_by_chunk_id
                    .get(&chunk.chunk_id)
                    .map(|citation| citation.doc_name.clone())
                    .unwrap_or_else(|| format!("Chunk {}", chunk.chunk_id));
                SourceRef {
                    id: chunk.chunk_id.clone(),
                    title,
                    snippet: Some(chunk.text.chars().take(200).collect()),
                    doc_id: Some(chunk.doc_id.clone()),
                    page: chunk.page.map(|page| page as usize),
                }
            })
            .collect::<Vec<_>>();

        let answer_text = ensure_inline_image_placeholder(&answer.answer_text, &citations);
        let rendered_answer = materialize_answer_markup(&answer_text, &citations);
        let answer_blocks =
            common::answer_blocks_from_rendered_answer(&rendered_answer, &citations);
        let rag_plan = execute_request.to_rag_plan_compat();

        ChatResponse {
            answer: rendered_answer,
            answer_blocks,
            session_id: resolved_session_id
                .map(str::to_string)
                .or_else(|| request.session_id.clone())
                .unwrap_or_else(|| Uuid::new_v4().to_string()),
            agent_type: request.agent_type.clone(),
            sources,
            citations,
            trace: TraceInfo {
                mode: "rag".to_string(),
            },
            degrade_trace,
            planner_output: Some(PlannerOutput {
                mode: "rag".to_string(),
                rag_plan: Some(rag_plan),
                search_plan: None,
                general_plan: None,
            }),
            mode_debug: Some(ModeDebug {
                rag: Some(RagModeDebug {
                    item_trace: execute_response.backend_trace.item_trace.clone(),
                    retrieval_trace: execute_response.backend_trace.retrieval_trace.clone(),
                    summary_injection_trace: SummaryInjectionTrace {
                        mode: execute_response
                            .backend_trace
                            .retrieval_trace
                            .summary_mode
                            .clone(),
                        injected_count: execute_response.coverage.summary_chunk_count,
                    },
                }),
                search: None,
                general: None,
            }),
            message_id: None,
            guard_report: None,
        }
    }
}

fn fallback_execute_plan_request(
    request: &ChatRequest,
    docscope_metadata: Option<&common::DocScopeMetadata>,
) -> ExecutePlanRequest {
    ExecutePlanRequest {
        plan_version: RAG_EXECUTE_PLAN_VERSION.to_string(),
        doc_scope: request.doc_scope.clone(),
        items: vec![ExecutePlanItem {
            priority: 1.0,
            query: Some(request.query.trim().to_string()),
            bm25_terms: None,
        }],
        summary_mode: if docscope_metadata.is_some_and(|metadata| !metadata.documents.is_empty()) {
            ExecutePlanSummaryMode::Related
        } else {
            ExecutePlanSummaryMode::None
        },
        budget: None,
        trace: None,
    }
}

fn build_rag_plan_user_prompt(
    request: &ChatRequest,
    docscope_metadata: Option<&common::DocScopeMetadata>,
    reference_context: Option<&MainAgentReferenceContext>,
) -> String {
    let metadata_json = docscope_metadata
        .and_then(|metadata| serde_json::to_string_pretty(metadata).ok())
        .unwrap_or_else(|| "null".to_string());
    let doc_scope_json =
        serde_json::to_string(&request.doc_scope).unwrap_or_else(|_| "[]".to_string());
    let authoritative = format!(
        "Provided doc_scope JSON:\n{}\n\nDocscope metadata JSON:\n{}",
        doc_scope_json, metadata_json
    );
    let reference = reference_context_section(reference_context);
    let preferences = reference_context
        .map(|context| {
            if context.user_preferences.is_empty() {
                "none".to_string()
            } else {
                context.user_preferences.join("\n")
            }
        })
        .unwrap_or_else(|| "none".to_string());

    build_main_agent_envelope(MainAgentContext {
        mode: "rag_plan".to_string(),
        current_task: request.query.trim().to_string(),
        authoritative_context: authoritative,
        reference_context: reference,
        user_preference_memory: preferences,
        skill: "Generate an execute-plan for the RAG API, or ask one natural-language clarification question.".to_string(),
        output_contract: "Return exactly one raw JSON object: either ExecutePlanRequest or {\"action\":\"clarify\",\"message\":\"...\"}.".to_string(),
    })
}

fn build_rag_answer_user_prompt(
    request: &ChatRequest,
    execute_request: &ExecutePlanRequest,
    execute_response: &ExecutePlanResponse,
    answer_context: &[AnswerContextChunk],
    reference_context: Option<&MainAgentReferenceContext>,
) -> String {
    let backend_trace_json = serde_json::to_string_pretty(&execute_response.backend_trace)
        .unwrap_or_else(|_| "{}".to_string());
    let coverage_json = serde_json::to_string_pretty(&execute_response.coverage)
        .unwrap_or_else(|_| "{}".to_string());
    let context_json =
        serde_json::to_string_pretty(answer_context).unwrap_or_else(|_| "[]".to_string());
    let preferences = reference_context
        .map(|context| {
            if context.user_preferences.is_empty() {
                "none".to_string()
            } else {
                context.user_preferences.join("\n")
            }
        })
        .unwrap_or_else(|| "none".to_string());

    build_main_agent_envelope(MainAgentContext {
        mode: "rag_answer".to_string(),
        current_task: request.query.trim().to_string(),
        authoritative_context: format!(
            "RAG Evidence (only factual evidence):\nRetrieval bundle answer context JSON:\n{}\n\nCoverage JSON:\n{}\n\nBackend trace JSON:\n{}",
            context_json, coverage_json, backend_trace_json
        ),
        reference_context: "none".to_string(),
        user_preference_memory: preferences,
        skill: format!(
            "Answer using only RAG Evidence for factual claims. Use preferences only for expression style. The executed doc_scope was: {}.",
            execute_request.doc_scope.join(", ")
        ),
        output_contract: "Return a natural-language answer only.".to_string(),
    })
}

fn parse_rag_plan_decision(raw: &str, request: &ChatRequest) -> Option<MainAgentRagPlanDecision> {
    let json = extract_json_object(raw).unwrap_or_else(|| raw.trim().to_string());
    if let Ok(value) = serde_json::from_str::<serde_json::Value>(&json) {
        if value
            .get("action")
            .and_then(serde_json::Value::as_str)
            .is_some_and(|action| action.eq_ignore_ascii_case("clarify"))
        {
            let message = value
                .get("message")
                .and_then(serde_json::Value::as_str)
                .map(str::trim)
                .filter(|message| !message.is_empty())?;
            return Some(MainAgentRagPlanDecision::Clarify(message.to_string()));
        }
    }

    let plan = serde_json::from_str::<ExecutePlanRequest>(&json).ok()?;
    match normalize_execute_plan_request(plan, request) {
        Some(plan) => Some(MainAgentRagPlanDecision::Execute(plan)),
        None => Some(MainAgentRagPlanDecision::Clarify(
            "请补充要检索的具体问题或目标文档范围。".to_string(),
        )),
    }
}

fn extract_json_object(raw: &str) -> Option<String> {
    let start = raw.find('{')?;
    let end = raw.rfind('}')?;
    (start <= end).then(|| raw[start..=end].to_string())
}

fn reference_context_section(reference_context: Option<&MainAgentReferenceContext>) -> String {
    let Some(context) = reference_context else {
        return "none".to_string();
    };
    let mut parts = Vec::new();
    if let Some(state) = context
        .session_working_state
        .as_deref()
        .map(str::trim)
        .filter(|state| !state.is_empty())
    {
        parts.push(format!("Session working state:\n{state}"));
    }
    if !context.recent_user_turns.is_empty() {
        parts.push(format!(
            "Recent user turns:\n{}",
            context.recent_user_turns.join("\n")
        ));
    }
    if parts.is_empty() {
        "none".to_string()
    } else {
        parts.join("\n\n")
    }
}

fn build_main_agent_envelope(context: MainAgentContext) -> String {
    format!(
        "<Mode>\n{}\n\n<Current Task>\n{}\n\n<Authoritative Context>\n{}\n\n<Reference Context>\n{}\n\n<User Preference Memory>\n{}\n\n<Skill>\n{}\n\n<Output Contract>\n{}",
        context.mode,
        context.current_task,
        context.authoritative_context,
        context.reference_context,
        context.user_preference_memory,
        context.skill,
        context.output_contract,
    )
}

fn normalize_execute_plan_request(
    mut plan: ExecutePlanRequest,
    request: &ChatRequest,
) -> Option<ExecutePlanRequest> {
    if plan.plan_version.trim().is_empty() {
        plan.plan_version = RAG_EXECUTE_PLAN_VERSION.to_string();
    }
    plan.doc_scope = request.doc_scope.clone();
    plan.trace = None;
    plan.items = plan
        .items
        .into_iter()
        .filter_map(normalize_execute_plan_item)
        .take(4)
        .collect();
    plan.validate().ok()?;
    Some(plan)
}

fn normalize_execute_plan_item(item: ExecutePlanItem) -> Option<ExecutePlanItem> {
    let query = item
        .query
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned);
    let bm25_terms = item.bm25_terms.map(|terms| {
        terms
            .into_iter()
            .map(|term| term.trim().to_string())
            .filter(|term| !term.is_empty())
            .collect::<Vec<_>>()
    });
    let has_query = query.is_some();
    let has_bm25_terms = bm25_terms.as_ref().is_some_and(|terms| !terms.is_empty());

    if has_query {
        Some(ExecutePlanItem {
            priority: item.priority.clamp(0.0, 1.0),
            query,
            bm25_terms: None,
        })
    } else if has_bm25_terms {
        Some(ExecutePlanItem {
            priority: item.priority.clamp(0.0, 1.0),
            query: None,
            bm25_terms,
        })
    } else {
        None
    }
}

fn extract_referenced_chunk_ids(answer_text: &str) -> HashSet<String> {
    let mut remaining = answer_text;
    let mut ids = HashSet::new();
    while let Some(start) = remaining.find("[[") {
        let after_start = &remaining[start + 2..];
        let Some(end) = after_start.find("]]") else {
            break;
        };
        let token = after_start[..end].trim();
        if let Some(chunk_id) = token.strip_prefix("cite:").map(str::trim) {
            if !chunk_id.is_empty() {
                ids.insert(chunk_id.to_string());
            }
        } else if let Some(chunk_id) = token.strip_prefix("image:").map(str::trim)
            && !chunk_id.is_empty()
        {
            ids.insert(chunk_id.to_string());
        }
        remaining = &after_start[end + 2..];
    }
    ids
}

fn ensure_inline_image_placeholder(answer_text: &str, citations: &[common::Citation]) -> String {
    if answer_text.contains("[[image:") {
        return answer_text.to_string();
    }

    let Some(image_citation) = citations.iter().find(|citation| {
        citation
            .image_url
            .as_ref()
            .is_some_and(|url| !url.trim().is_empty())
    }) else {
        return answer_text.to_string();
    };

    let Some(chunk_id) = image_citation.chunk_id.as_deref() else {
        return answer_text.to_string();
    };

    format!("{}\n\n[[image:{}]]", answer_text.trim_end(), chunk_id)
}

fn materialize_answer_markup(answer_text: &str, citations: &[common::Citation]) -> String {
    let citation_index_by_chunk = citations
        .iter()
        .filter_map(|citation| {
            citation
                .chunk_id
                .as_ref()
                .map(|chunk_id| (chunk_id.clone(), citation.citation_id))
        })
        .collect::<HashMap<_, _>>();
    let mut rendered = String::new();
    let mut remaining = answer_text;
    let mut replaced_any = false;

    while let Some(start) = remaining.find("[[") {
        rendered.push_str(&remaining[..start]);
        let after_start = &remaining[start + 2..];
        let Some(end) = after_start.find("]]") else {
            rendered.push_str(&remaining[start..]);
            remaining = "";
            break;
        };
        let token = after_start[..end].trim();
        if let Some(chunk_id) = token.strip_prefix("cite:").map(str::trim) {
            if let Some(citation_id) = citation_index_by_chunk.get(chunk_id) {
                rendered.push_str(&format!("[[{citation_id}]]"));
                replaced_any = true;
            }
        } else if let Some(chunk_id) = token.strip_prefix("image:").map(str::trim) {
            if let Some(citation_id) = citation_index_by_chunk.get(chunk_id) {
                rendered.push_str(&format!("[[image:{citation_id}]]"));
                replaced_any = true;
            }
        } else {
            rendered.push_str(&remaining[start..start + 2 + end + 2]);
        }
        remaining = &after_start[end + 2..];
    }
    rendered.push_str(remaining);

    if replaced_any || citations.is_empty() {
        return rendered;
    }

    let inline_refs = citations
        .iter()
        .take(2)
        .map(|citation| format!("[[{}]]", citation.citation_id))
        .collect::<Vec<_>>()
        .join(" ");
    if inline_refs.is_empty() {
        rendered
    } else {
        format!("{} {}", rendered.trim_end(), inline_refs)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn request(agent_type: &str, query: &str, doc_scope: &[&str]) -> ChatRequest {
        ChatRequest {
            query: query.to_string(),
            notebook_id: None,
            session_id: None,
            agent_type: agent_type.to_string(),
            source_type: None,
            source_token: None,
            doc_scope: doc_scope.iter().map(|value| value.to_string()).collect(),
            messages: Vec::new(),
            stream: false,
        }
    }

    fn sample_execute_response() -> ExecutePlanResponse {
        ExecutePlanResponse {
            bundle: common::RetrievalBundle {
                chunks: vec![common::RetrievedChunk {
                    chunk_id: "chunk-1".to_string(),
                    doc_id: "doc-1".to_string(),
                    chunk_type: "text".to_string(),
                    page: Some(1),
                    text: "retrieved".to_string(),
                    score: 0.9,
                    retrieval_channel: "dense".to_string(),
                    asset_id: None,
                    caption: None,
                    image_url: None,
                    parser_backend: None,
                    source_locator: None,
                }],
                citations: vec![common::Citation {
                    citation_id: 1,
                    doc_id: "doc-1".to_string(),
                    chunk_id: Some("chunk-1".to_string()),
                    page: Some(1),
                    doc_name: "Document 1".to_string(),
                    preview: Some("retrieved".to_string()),
                    content: Some("retrieved".to_string()),
                    score: 0.9,
                    layer: Some("dense".to_string()),
                    chunk_type: Some("text".to_string()),
                    asset_id: None,
                    caption: None,
                    image_url: None,
                    parser_backend: None,
                    source_locator: None,
                }],
                summary_chunks: Vec::new(),
            },
            coverage: common::Coverage {
                requested_doc_count: 1,
                matched_doc_count: 1,
                retrieved_chunk_count: 1,
                summary_chunk_count: 0,
            },
            degrade_trace: Vec::new(),
            backend_trace: common::BackendTrace {
                trace: None,
                item_trace: vec![common::RagTraceItem {
                    priority: 1.0,
                    payload_kind: "query".to_string(),
                    query: Some("test".to_string()),
                    bm25_terms: Vec::new(),
                    summary: None,
                    recall_budget: 100,
                    bm25_k: 0,
                    dense_k: 100,
                    rerank_budget: 100,
                    source_count: 1,
                    source_ids: vec!["chunk-1".to_string()],
                }],
                retrieval_trace: common::RagTraceSummary {
                    item_count: 1,
                    total_candidate_budget: 100,
                    max_rerank_docs: 100,
                    max_final_chunks: 30,
                    top_k_returned: 1,
                    summary_mode: "none".to_string(),
                    items: Vec::new(),
                },
            },
        }
    }

    #[test]
    fn mode_profiles_match_existing_frontend_values() {
        assert_eq!(MainAgent::profile("general"), ModeProfile::General);
        assert_eq!(MainAgent::profile("rag"), ModeProfile::Rag);
        assert_eq!(MainAgent::profile("search"), ModeProfile::Search);
    }

    #[test]
    fn rag_decision_can_return_clarify_for_ambiguous_query_without_docscope() {
        let decision = MainAgent::decide(&request("rag", "这个", &[]));
        assert!(matches!(decision, MainAgentDecision::Clarify { .. }));
    }

    #[test]
    fn rag_decision_requires_explicit_docscope_even_for_specific_query() {
        let decision = MainAgent::decide(&request("rag", "find rollback checklist", &[]));
        assert!(matches!(decision, MainAgentDecision::Clarify { .. }));
    }

    #[test]
    fn general_and_search_modes_route_to_expected_decisions() {
        assert_eq!(
            MainAgent::decide(&request("general", "hello", &[])),
            MainAgentDecision::DirectChat
        );
        assert_eq!(
            MainAgent::decide(&request("search", "latest rust release", &[])),
            MainAgentDecision::ExternalSearch
        );
    }

    #[test]
    fn execute_plan_bundle_consumption_preserves_retrieval_then_summary_order() {
        let response = ExecutePlanResponse {
            bundle: common::RetrievalBundle {
                chunks: vec![common::RetrievedChunk {
                    chunk_id: "chunk-1".to_string(),
                    doc_id: "doc-1".to_string(),
                    chunk_type: "text".to_string(),
                    page: Some(1),
                    text: "retrieved".to_string(),
                    score: 0.9,
                    retrieval_channel: "dense".to_string(),
                    asset_id: None,
                    caption: None,
                    image_url: None,
                    parser_backend: None,
                    source_locator: None,
                }],
                citations: Vec::new(),
                summary_chunks: vec![common::AnswerContextChunk {
                    chunk_id: "summary-doc-1".to_string(),
                    doc_id: Some("doc-1".to_string()),
                    chunk_type: "summary".to_string(),
                    page: None,
                    text: "[Document Summary] summary".to_string(),
                    asset_id: None,
                    caption: None,
                    image_url: None,
                    parser_backend: None,
                    source_locator: None,
                }],
            },
            coverage: common::Coverage {
                requested_doc_count: 1,
                matched_doc_count: 1,
                retrieved_chunk_count: 1,
                summary_chunk_count: 1,
            },
            degrade_trace: Vec::new(),
            backend_trace: common::BackendTrace {
                trace: None,
                item_trace: Vec::new(),
                retrieval_trace: common::RagTraceSummary {
                    item_count: 0,
                    total_candidate_budget: 0,
                    max_rerank_docs: 0,
                    max_final_chunks: 0,
                    top_k_returned: 1,
                    summary_mode: "related".to_string(),
                    items: Vec::new(),
                },
            },
        };

        let answer_context = MainAgent::answer_context(&response);
        assert_eq!(answer_context.len(), 2);
        assert_eq!(answer_context[0].chunk_type, "text");
        assert_eq!(answer_context[1].chunk_type, "summary");
    }

    #[tokio::test]
    async fn rag_planning_without_answer_llm_returns_valid_execute_plan_request() {
        let request = request("rag", "find rollback checklist", &["doc-1"]);
        let mut degrade_trace = Vec::new();
        let result =
            MainAgent::plan_rag(&request, None, None, None, Some(0.1), &mut degrade_trace).await;
        let MainAgentRagPlanDecision::Execute(execute_request) = result.decision else {
            panic!("expected execute decision");
        };
        let encoded = serde_json::to_value(&execute_request).unwrap();

        execute_request.validate().unwrap();
        assert_eq!(execute_request.doc_scope, vec!["doc-1".to_string()]);
        assert_eq!(execute_request.items.len(), 1);
        assert!(encoded.get("clarify_needed").is_none());
        assert!(encoded.get("session_id").is_none());
        assert!(encoded.get("messages").is_none());
        assert_eq!(degrade_trace[0].stage, "main_agent.rag_plan");
    }

    #[tokio::test]
    async fn rag_planning_without_docscope_returns_clarify() {
        let request = request("rag", "find rollback checklist", &[]);
        let mut degrade_trace = Vec::new();
        let result =
            MainAgent::plan_rag(&request, None, None, None, Some(0.1), &mut degrade_trace).await;

        assert!(matches!(
            result.decision,
            MainAgentRagPlanDecision::Clarify(_)
        ));
    }

    #[test]
    fn rag_answer_prompt_uses_envelope_and_keeps_preferences_out_of_evidence() {
        let request = request("rag", "summarize", &["doc-1"]);
        let execute_request = fallback_execute_plan_request(&request, None);
        let execute_response = sample_execute_response();
        let answer_context = MainAgent::answer_context(&execute_response);
        let reference_context = MainAgentReferenceContext {
            session_working_state: Some("topic: stale topic".to_string()),
            recent_user_turns: vec!["previous failed retrieval".to_string()],
            user_preferences: vec!["Use concise answers".to_string()],
        };

        let prompt = build_rag_answer_user_prompt(
            &request,
            &execute_request,
            &execute_response,
            &answer_context,
            Some(&reference_context),
        );

        assert!(prompt.contains("<Authoritative Context>"));
        assert!(prompt.contains("RAG Evidence (only factual evidence)"));
        assert!(prompt.contains("<User Preference Memory>"));
        assert!(prompt.contains("Use concise answers"));
        assert!(!prompt.contains("stale topic"));
        assert!(!prompt.contains("previous failed retrieval"));
    }

    #[tokio::test]
    async fn rag_answer_without_answer_llm_returns_degraded_non_empty_answer() {
        let request = request("rag", "summarize", &["doc-1"]);
        let execute_request = fallback_execute_plan_request(&request, None);
        let execute_response = sample_execute_response();
        let mut degrade_trace = Vec::new();

        let answer = MainAgent::answer_rag(
            &request,
            &execute_request,
            &execute_response,
            None,
            None,
            Some(0.2),
            &mut degrade_trace,
        )
        .await;
        let response = MainAgent::build_rag_chat_response(
            &request,
            Some("session-1"),
            &execute_request,
            &execute_response,
            answer,
            degrade_trace.clone(),
        );

        assert!(!response.answer.is_empty());
        assert_eq!(response.citations.len(), 1);
        assert!(response.planner_output.is_some());
        assert!(
            response
                .mode_debug
                .as_ref()
                .and_then(|debug| debug.rag.as_ref())
                .is_some()
        );
        assert_eq!(degrade_trace[0].stage, "main_agent.rag_answer");
    }
}
