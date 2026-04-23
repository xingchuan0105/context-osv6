use common::{AnswerContextChunk, ChatRequest, ExecutePlanResponse};

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

#[derive(Debug, Default, Clone, Copy)]
pub struct MainAgent;

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
                if request.doc_scope.is_empty() && is_ambiguous_rag_query(&request.query) {
                    MainAgentDecision::Clarify {
                        message: "请指出具体文档，或把问题说得更具体一些，再让我执行知识库检索。"
                            .to_string(),
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
}

fn is_ambiguous_rag_query(query: &str) -> bool {
    let trimmed = query.trim();
    if trimmed.is_empty() {
        return true;
    }

    let normalized = trimmed.to_ascii_lowercase();
    matches!(
        normalized.as_str(),
        "this"
            | "that"
            | "it"
            | "these"
            | "those"
            | "this doc"
            | "that doc"
            | "the doc"
    ) || matches!(
        trimmed,
        "这个" | "这份" | "这个文档" | "那份" | "那个" | "上一个" | "上一份" | "刚才那个"
    )
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
}
