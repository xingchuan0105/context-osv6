use std::collections::{BTreeSet, HashMap, HashSet};

use anyhow::Result;
use avrag_auth::AuthContext;
use avrag_llm::ChatMessage as LlmChatMessage;
use common::{
    ChatRequest, ChatResponse, Citation, DegradeTraceItem, ExecutePlanResponse, ModeDebug,
    PlannerOutput, RagModeDebug, RagPlan, RagTraceItem, RagTraceSummary, SourceRef,
    SummaryInjectionTrace, TraceInfo,
};
use uuid::Uuid;

use crate::context::count_tokens;
use crate::retrieval::ScoredChunk;

use super::RagRuntime;
use super::planner::rag_summary_mode;
pub(super) use super::response_utils::{
    ensure_inline_image_placeholder, extract_referenced_chunk_ids, materialize_answer_markup,
    no_chunks_response, no_valid_retrieval_results_answer,
};
use super::{FINAL_MIN_CHUNKS, FINAL_RERANK_BUDGET, TOTAL_CANDIDATE_BUDGET};

const DEFAULT_MODEL_MAX_TOKENS: usize = 8192;
const RESERVED_SYSTEM_TOKENS: usize = 768;
const RESERVED_HISTORY_TOKENS: usize = 1024;
const RESERVED_OUTPUT_TOKENS: usize = 1536;
const MIN_CONTEXT_BUDGET_TOKENS: usize = 512;

pub(super) fn answer_context_budget_tokens() -> usize {
    DEFAULT_MODEL_MAX_TOKENS
        .saturating_sub(RESERVED_SYSTEM_TOKENS + RESERVED_HISTORY_TOKENS + RESERVED_OUTPUT_TOKENS)
        .max(MIN_CONTEXT_BUDGET_TOKENS)
}

pub(super) fn synthesizer_history(
    session_context: Option<&crate::context::SessionContext>,
) -> Vec<LlmChatMessage> {
    let Some(session_context) = session_context else {
        return Vec::new();
    };

    let mut history = Vec::new();
    if let Some(summary) = session_context
        .summary
        .as_deref()
        .map(str::trim)
        .filter(|summary| !summary.is_empty())
    {
        history.push(LlmChatMessage::system(format!(
            "Conversation summary:\n{}",
            summary
        )));
    }

    history.extend(
        session_context
            .messages
            .iter()
            .rev()
            .take(8)
            .collect::<Vec<_>>()
            .into_iter()
            .rev()
            .filter_map(|message| {
                let content = message.content.trim();
                if content.is_empty() {
                    return None;
                }
                let role = match message.role.as_str() {
                    "user" => "user",
                    "assistant" => "assistant",
                    _ => return None,
                };
                Some(LlmChatMessage {
                    role: role.to_string(),
                    content: content.to_string(),
                })
            }),
    );

    history
}

fn stream_cited_chunk_ids(context_chunks: &[common::AnswerContextChunk]) -> Vec<String> {
    context_chunks
        .iter()
        .filter(|chunk| chunk.chunk_type != "summary")
        .map(|chunk| chunk.chunk_id.clone())
        .take(6)
        .collect()
}

impl RagRuntime {
    pub async fn apply_summary_policy(
        &self,
        request: &ChatRequest,
        auth: &AuthContext,
        rag_plan: &RagPlan,
        chunks: &[ScoredChunk],
    ) -> Result<Vec<(Uuid, String)>> {
        let Some(pg_repo) = self.config.pg_repo.as_ref() else {
            return Ok(Vec::new());
        };

        let summary_mode = rag_summary_mode(rag_plan);
        let unique_doc_ids = chunks
            .iter()
            .map(|chunk| chunk.doc_id)
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect::<Vec<_>>();
        let doc_scope_ids = super::planner::request_doc_ids(request);
        let notebook_id = request
            .notebook_id
            .as_deref()
            .and_then(|id| Uuid::parse_str(id).ok());

        let summary_target_ids = if summary_mode == "related" {
            unique_doc_ids
        } else if summary_mode == "all" {
            if let Some(scope_ids) = doc_scope_ids.as_deref() {
                scope_ids.to_vec()
            } else if let Some(notebook_id) = notebook_id {
                pg_repo
                    .list_documents(auth, Some(notebook_id), None)
                    .await
                    .unwrap_or_default()
                    .into_iter()
                    .filter_map(|document| Uuid::parse_str(&document.id).ok())
                    .collect::<Vec<_>>()
            } else {
                Vec::new()
            }
        } else {
            Vec::new()
        };

        if summary_target_ids.is_empty() {
            return Ok(Vec::new());
        }

        Ok(pg_repo
            .get_summary_chunks(auth, &summary_target_ids)
            .await
            .unwrap_or_default())
    }

    pub fn build_answer_context_chunks(
        &self,
        summary_chunks: &[(Uuid, String)],
        retrieval_chunks: &[ScoredChunk],
    ) -> Vec<common::AnswerContextChunk> {
        let context_budget = answer_context_budget_tokens();
        let mut context_chunks = Vec::new();
        let mut used_tokens = 0usize;

        // PRD §10.2: retrieval chunks are assembled before summary chunks.
        for chunk in retrieval_chunks {
            let tokens = count_tokens(&chunk.content);
            if used_tokens + tokens > context_budget {
                break;
            }
            context_chunks.push(common::AnswerContextChunk {
                chunk_id: chunk.chunk_id.to_string(),
                doc_id: Some(chunk.doc_id.to_string()),
                chunk_type: chunk.chunk_type.clone(),
                page: chunk.page,
                text: chunk.content.clone(),
                asset_id: chunk.asset_id.map(|asset_id| asset_id.to_string()),
                caption: chunk.caption.clone(),
                image_url: chunk.image_path.clone(),
                parser_backend: chunk.parser_backend.clone(),
                source_locator: chunk.source_locator.clone(),
            });
            used_tokens += tokens;
        }

        for (doc_id, content) in summary_chunks {
            let prefixed = format!("[Document Summary] {}", content);
            let tokens = count_tokens(&prefixed);
            if used_tokens + tokens > context_budget {
                break;
            }
            context_chunks.push(common::AnswerContextChunk {
                chunk_id: format!("summary-{}", doc_id),
                doc_id: Some(doc_id.to_string()),
                chunk_type: "summary".to_string(),
                page: None,
                text: prefixed,
                asset_id: None,
                caption: None,
                image_url: None,
                parser_backend: None,
                source_locator: None,
            });
            used_tokens += tokens;
        }

        context_chunks
    }

    /// Legacy compatibility synthesizer for old `RagPlan` + session context callers.
    ///
    /// Product chat now answers through the Main Agent with the execute-plan
    /// retrieval bundle as the only factual evidence.
    pub async fn synthesize_answer_text(
        &self,
        request: &ChatRequest,
        session_context: Option<&crate::context::SessionContext>,
        rag_plan: &RagPlan,
        item_trace: &[RagTraceItem],
        context_chunks: &[common::AnswerContextChunk],
        degrade_trace: &mut Vec<DegradeTraceItem>,
    ) -> avrag_llm::SynthesisOutput {
        let synthesizer_history = synthesizer_history(session_context);
        let synthesizer_history_ref =
            (!synthesizer_history.is_empty()).then_some(synthesizer_history.as_slice());

        if context_chunks.is_empty() {
            if let Some(synthesizer) = &self.config.answer_synthesizer {
                match synthesizer
                    .synthesize(
                        &request.query,
                        &[],
                        &Some(rag_plan.clone()),
                        item_trace,
                        synthesizer_history_ref,
                    )
                    .await
                {
                    Ok(answer) => return answer,
                    Err(error) => {
                        degrade_trace.push(DegradeTraceItem {
                            stage: "synthesizer".to_string(),
                            reason: format!("Synthesizer call failed: {}", error),
                            impact: "Returning a fallback no-results answer".to_string(),
                        });
                        return avrag_llm::SynthesisOutput {
                            answer_text: no_valid_retrieval_results_answer().to_string(),
                            answer_blocks: common::plain_text_answer_blocks(
                                no_valid_retrieval_results_answer(),
                            ),
                            cited_chunk_ids: Vec::new(),
                            llm_usage: None,
                        };
                    }
                }
            }

            degrade_trace.push(DegradeTraceItem {
                stage: "synthesizer".to_string(),
                reason: "Synthesizer not configured".to_string(),
                impact: "Returning a fallback no-results answer".to_string(),
            });
            return avrag_llm::SynthesisOutput {
                answer_text: no_valid_retrieval_results_answer().to_string(),
                answer_blocks: common::plain_text_answer_blocks(no_valid_retrieval_results_answer()),
                cited_chunk_ids: Vec::new(),
                llm_usage: None,
            };
        }

        if let Some(synthesizer) = &self.config.answer_synthesizer {
            match synthesizer
                .synthesize(
                    &request.query,
                    context_chunks,
                    &Some(rag_plan.clone()),
                    item_trace,
                    synthesizer_history_ref,
                )
                .await
            {
                Ok(answer) => answer,
                Err(error) => {
                    degrade_trace.push(DegradeTraceItem {
                        stage: "synthesizer".to_string(),
                        reason: format!("Synthesizer call failed: {}", error),
                        impact: "Returning explicit synthesis-unavailable answer".to_string(),
                    });
                    avrag_llm::SynthesisOutput {
                        answer_text:
                            "Answer generation is currently unavailable even though relevant evidence was retrieved."
                                .to_string(),
                        answer_blocks: common::plain_text_answer_blocks(
                            "Answer generation is currently unavailable even though relevant evidence was retrieved.",
                        ),
                        cited_chunk_ids: Vec::new(),
                        llm_usage: None,
                    }
                }
            }
        } else {
            degrade_trace.push(DegradeTraceItem {
                stage: "synthesizer".to_string(),
                reason: "Synthesizer not configured".to_string(),
                impact: "Returning explicit synthesis-unavailable answer".to_string(),
            });
            avrag_llm::SynthesisOutput {
                answer_text:
                    "Answer generation is currently unavailable even though relevant evidence was retrieved."
                        .to_string(),
                answer_blocks: common::plain_text_answer_blocks(
                    "Answer generation is currently unavailable even though relevant evidence was retrieved.",
                ),
                cited_chunk_ids: Vec::new(),
                llm_usage: None,
            }
        }
    }

    /// Legacy compatibility streaming synthesizer for old `RagPlan` callers.
    pub async fn synthesize_answer_text_stream(
        &self,
        request: &ChatRequest,
        session_context: Option<&crate::context::SessionContext>,
        rag_plan: &RagPlan,
        item_trace: &[RagTraceItem],
        context_chunks: &[common::AnswerContextChunk],
        degrade_trace: &mut Vec<DegradeTraceItem>,
        on_delta: impl FnMut(&str),
    ) -> avrag_llm::SynthesisOutput {
        let synthesizer_history = synthesizer_history(session_context);
        let synthesizer_history_ref =
            (!synthesizer_history.is_empty()).then_some(synthesizer_history.as_slice());

        if context_chunks.is_empty() {
            return self
                .synthesize_answer_text(
                    request,
                    session_context,
                    rag_plan,
                    item_trace,
                    context_chunks,
                    degrade_trace,
                )
                .await;
        }

        if let Some(synthesizer) = &self.config.answer_synthesizer {
            match synthesizer
                .synthesize_stream_text(
                    &request.query,
                    context_chunks,
                    &Some(rag_plan.clone()),
                    item_trace,
                    synthesizer_history_ref,
                    on_delta,
                )
                .await
            {
                Ok(answer) => avrag_llm::SynthesisOutput {
                    answer_text: answer.content,
                    answer_blocks: Vec::new(),
                    cited_chunk_ids: stream_cited_chunk_ids(context_chunks),
                    llm_usage: Some(answer.usage),
                },
                Err(error) => {
                    degrade_trace.push(DegradeTraceItem {
                        stage: "synthesizer".to_string(),
                        reason: format!("Streaming synthesizer call failed: {}", error),
                        impact: "Returning explicit synthesis-unavailable answer".to_string(),
                    });
                    avrag_llm::SynthesisOutput {
                        answer_text:
                            "Answer generation is currently unavailable even though relevant evidence was retrieved."
                                .to_string(),
                        answer_blocks: common::plain_text_answer_blocks(
                            "Answer generation is currently unavailable even though relevant evidence was retrieved.",
                        ),
                        cited_chunk_ids: Vec::new(),
                        llm_usage: None,
                    }
                }
            }
        } else {
            degrade_trace.push(DegradeTraceItem {
                stage: "synthesizer".to_string(),
                reason: "Synthesizer not configured".to_string(),
                impact: "Returning explicit synthesis-unavailable answer".to_string(),
            });
            avrag_llm::SynthesisOutput {
                answer_text:
                    "Answer generation is currently unavailable even though relevant evidence was retrieved."
                        .to_string(),
                answer_blocks: common::plain_text_answer_blocks(
                    "Answer generation is currently unavailable even though relevant evidence was retrieved.",
                ),
                cited_chunk_ids: Vec::new(),
                llm_usage: None,
            }
        }
    }

    pub async fn build_rag_chat_response(
        &self,
        request: &ChatRequest,
        resolved_session_id: Option<&str>,
        auth: &AuthContext,
        rag_plan: &RagPlan,
        chunks: &[ScoredChunk],
        item_trace: &[RagTraceItem],
        summary_count: usize,
        synthesis_output: avrag_llm::SynthesisOutput,
        degrade_trace: Vec<DegradeTraceItem>,
    ) -> Result<ChatResponse> {
        if chunks.is_empty() {
            return Ok(no_chunks_response(
                request,
                rag_plan,
                item_trace,
                degrade_trace,
                synthesis_output.answer_text,
            ));
        }

        let mut cited_chunk_ids = synthesis_output
            .cited_chunk_ids
            .iter()
            .cloned()
            .collect::<HashSet<_>>();
        cited_chunk_ids.extend(extract_referenced_chunk_ids(&synthesis_output.answer_text));
        let ordered_chunks = if cited_chunk_ids.is_empty() {
            chunks.to_vec()
        } else {
            let mut filtered = chunks
                .iter()
                .filter(|chunk| cited_chunk_ids.contains(&chunk.chunk_id.to_string()))
                .cloned()
                .collect::<Vec<_>>();
            if filtered.is_empty() {
                filtered = chunks.to_vec();
            }
            filtered
        };

        let unique_doc_ids = ordered_chunks
            .iter()
            .map(|chunk| chunk.doc_id)
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect::<Vec<_>>();
        let doc_names = if let Some(pg_repo) = self.config.pg_repo.as_ref() {
            pg_repo
                .get_document_names(auth, &unique_doc_ids)
                .await
                .unwrap_or_default()
        } else {
            HashMap::new()
        };

        let citations = ordered_chunks
            .iter()
            .enumerate()
            .map(|(index, chunk)| Citation {
                citation_id: (index + 1) as i64,
                doc_id: chunk.doc_id.to_string(),
                chunk_id: Some(chunk.chunk_id.to_string()),
                page: chunk.page.map(|page| page as usize),
                doc_name: doc_names
                    .get(&chunk.doc_id)
                    .cloned()
                    .unwrap_or_else(|| format!("Document {}", chunk.doc_id)),
                preview: Some(chunk.content.chars().take(100).collect()),
                content: Some(chunk.content.clone()),
                score: chunk.score,
                layer: Some(chunk.source.clone()),
                chunk_type: Some(chunk.chunk_type.clone()),
                asset_id: chunk.asset_id.map(|asset_id| asset_id.to_string()),
                caption: chunk.caption.clone(),
                image_url: chunk.image_path.clone(),
                parser_backend: chunk.parser_backend.clone(),
                source_locator: chunk.source_locator.clone(),
                parse_run_id: chunk
                    .parse_run_id
                    .map(|parse_run_id| parse_run_id.to_string()),
            })
            .collect::<Vec<_>>();

        let sources = ordered_chunks
            .iter()
            .map(|chunk| SourceRef {
                id: chunk.chunk_id.to_string(),
                title: format!("Chunk {}", chunk.chunk_id),
                snippet: Some(chunk.content.chars().take(200).collect()),
                doc_id: Some(chunk.doc_id.to_string()),
                page: chunk.page.map(|page| page as usize),
            })
            .collect::<Vec<_>>();

        let summary_mode = rag_summary_mode(rag_plan);
        let answer_text = if synthesis_output.answer_blocks.is_empty() {
            ensure_inline_image_placeholder(&synthesis_output.answer_text, &citations)
        } else {
            common::answer_blocks_to_markup(&synthesis_output.answer_blocks)
        };
        let answer_blocks = if synthesis_output.answer_blocks.is_empty() {
            common::answer_blocks_from_rendered_answer(&answer_text, &citations)
        } else {
            synthesis_output.answer_blocks.clone()
        };
        Ok(ChatResponse {
            answer: materialize_answer_markup(&answer_text, &citations),
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
                rag_plan: Some(rag_plan.clone()),
                search_plan: None,
                general_plan: None,
            }),
            mode_debug: Some(ModeDebug {
                rag: Some(RagModeDebug {
                    item_trace: item_trace.to_vec(),
                    retrieval_trace: RagTraceSummary {
                        item_count: item_trace.len(),
                        total_candidate_budget: TOTAL_CANDIDATE_BUDGET,
                        max_rerank_docs: FINAL_RERANK_BUDGET,
                        max_final_chunks: FINAL_MIN_CHUNKS,
                        top_k_returned: chunks.len(),
                        summary_mode: summary_mode.clone(),
                        items: item_trace.to_vec(),
                    },
                    summary_injection_trace: SummaryInjectionTrace {
                        mode: summary_mode,
                        injected_count: summary_count,
                    },
                }),
                search: None,
                general: None,
            }),
            message_id: None,
            guard_report: None,
        })
    }

    pub async fn build_rag_chat_response_from_bundle(
        &self,
        request: &ChatRequest,
        resolved_session_id: Option<&str>,
        rag_plan: &RagPlan,
        execute_response: &ExecutePlanResponse,
        synthesis_output: avrag_llm::SynthesisOutput,
        degrade_trace: Vec<DegradeTraceItem>,
    ) -> Result<ChatResponse> {
        // 使用 has_evidence() 检查所有 evidence 类型
        if !execute_response.bundle.has_evidence() {
            return Ok(no_chunks_response(
                request,
                rag_plan,
                &execute_response.backend_trace.item_trace,
                degrade_trace,
                synthesis_output.answer_text,
            ));
        }

        let mut cited_chunk_ids = synthesis_output
            .cited_chunk_ids
            .iter()
            .cloned()
            .collect::<HashSet<_>>();
        cited_chunk_ids.extend(extract_referenced_chunk_ids(&synthesis_output.answer_text));

        // 使用 citation_chunks() 获取所有可用 chunks
        let all_chunks = execute_response.bundle.citation_chunks();

        let ordered_chunks = if cited_chunk_ids.is_empty() {
            all_chunks.to_vec()
        } else {
            let mut filtered = all_chunks
                .iter()
                .filter(|chunk| cited_chunk_ids.contains(&chunk.chunk_id))
                .cloned()
                .collect::<Vec<_>>();
            if filtered.is_empty() {
                filtered = all_chunks.to_vec();
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

        let answer_text = if synthesis_output.answer_blocks.is_empty() {
            ensure_inline_image_placeholder(&synthesis_output.answer_text, &citations)
        } else {
            common::answer_blocks_to_markup(&synthesis_output.answer_blocks)
        };
        let answer_blocks = if synthesis_output.answer_blocks.is_empty() {
            common::answer_blocks_from_rendered_answer(&answer_text, &citations)
        } else {
            synthesis_output.answer_blocks.clone()
        };

        Ok(ChatResponse {
            answer: materialize_answer_markup(&answer_text, &citations),
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
                rag_plan: Some(rag_plan.clone()),
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
        })
    }
}
