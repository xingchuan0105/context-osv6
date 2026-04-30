use crate::agents::events::{AgentEvent, AgentEventSink};
use crate::agents::runtime::{Agent, AgentRequest, AgentRunResult, AgentRunUsage};
use avrag_llm::{ChatMessage as LlmChatMessage, LlmClient, LlmUsage};
use common::{AppError, ChatRequest};
use std::sync::Arc;

struct SynthesizedSearchAnswer {
    answer: String,
    usage: Option<LlmUsage>,
}

#[async_trait::async_trait]
trait SearchAnswerSynthesizer: Send + Sync {
    async fn synthesize(
        &self,
        messages: &[LlmChatMessage],
        temperature: Option<f32>,
    ) -> anyhow::Result<SynthesizedSearchAnswer>;

    async fn synthesize_stream(
        &self,
        messages: &[LlmChatMessage],
        temperature: Option<f32>,
        on_delta: &mut (dyn FnMut(String) + Send),
    ) -> anyhow::Result<SynthesizedSearchAnswer>;
}

struct LlmSearchAnswerSynthesizer {
    llm: LlmClient,
}

#[async_trait::async_trait]
impl SearchAnswerSynthesizer for LlmSearchAnswerSynthesizer {
    async fn synthesize(
        &self,
        messages: &[LlmChatMessage],
        temperature: Option<f32>,
    ) -> anyhow::Result<SynthesizedSearchAnswer> {
        let response = self.llm.complete(messages, temperature).await?;
        Ok(SynthesizedSearchAnswer {
            answer: response.content,
            usage: Some(response.usage),
        })
    }

    async fn synthesize_stream(
        &self,
        messages: &[LlmChatMessage],
        temperature: Option<f32>,
        on_delta: &mut (dyn FnMut(String) + Send),
    ) -> anyhow::Result<SynthesizedSearchAnswer> {
        let response = self
            .llm
            .complete_stream(messages, temperature, |delta| on_delta(delta.to_string()))
            .await?;
        Ok(SynthesizedSearchAnswer {
            answer: response.content,
            usage: Some(response.usage),
        })
    }
}

/// WebSearchAgent handles external web search queries.
///
/// It wraps `SearchExecutor` and maps search stream updates into agent events.
pub struct WebSearchAgent {
    executor: Option<Arc<avrag_search::SearchExecutor>>,
    answer_synthesizer: Option<Arc<dyn SearchAnswerSynthesizer>>,
    temperature: Option<f32>,
}

impl WebSearchAgent {
    pub fn new(executor: Option<Arc<avrag_search::SearchExecutor>>) -> Self {
        Self {
            executor,
            answer_synthesizer: None,
            temperature: None,
        }
    }

    pub fn with_answer_synthesizer(
        mut self,
        answer_llm: Option<LlmClient>,
        temperature: Option<f32>,
    ) -> Self {
        self.answer_synthesizer = answer_llm.map(|llm| {
            Arc::new(LlmSearchAnswerSynthesizer { llm }) as Arc<dyn SearchAnswerSynthesizer>
        });
        self.temperature = temperature;
        self
    }
}

#[async_trait::async_trait]
impl Agent for WebSearchAgent {
    async fn run(
        &self,
        request: AgentRequest,
        sink: &dyn AgentEventSink,
    ) -> Result<AgentRunResult, AppError> {
        let Some(ref executor) = self.executor else {
            sink.emit(AgentEvent::Error {
                code: "search_unavailable".to_string(),
                message: "Search executor is not configured".to_string(),
            })
            .await;
            return Err(AppError::internal("Search executor is not configured"));
        };

        let chat_req = ChatRequest {
            query: request.query.clone(),
            notebook_id: request.notebook_id,
            session_id: request.session_id,
            agent_type: "search".to_string(),
            source_type: None,
            source_token: None,
            doc_scope: request.doc_scope,
            messages: vec![],
            stream: request.stream,
        };

        let mut answer = String::new();
        let (update_tx, mut update_rx) =
            tokio::sync::mpsc::unbounded_channel::<avrag_search::SearchStreamUpdate>();
        let search_stream = executor.execute_stream(&chat_req, move |update| {
            let _ = update_tx.send(update);
        });
        tokio::pin!(search_stream);

        let search_response = loop {
            tokio::select! {
                update = update_rx.recv() => {
                    if let Some(update) = update {
                        emit_search_update(update, sink, &mut answer).await;
                    }
                }
                result = &mut search_stream => {
                    break result.map_err(|e| AppError::internal(format!("Search execution failed: {}", e)))?;
                }
            }
        };

        while let Ok(update) = update_rx.try_recv() {
            emit_search_update(update, sink, &mut answer).await;
        }

        let mut degrade_trace = Vec::new();
        let mut answer_synthesis_mode = "provider_stream";
        let (answer, synthesis_usage) = if search_response.query_type == "brave_llm_context" {
            match synthesize_brave_answer(
                self.answer_synthesizer.as_deref(),
                self.temperature,
                &request.query,
                &search_response,
                request.stream,
                sink,
            )
            .await
            {
                Ok((answer, usage)) => {
                    answer_synthesis_mode = if request.stream {
                        "llm_stream"
                    } else {
                        "llm_complete"
                    };
                    (answer, usage)
                }
                Err(error) => {
                    answer_synthesis_mode = "evidence_fallback";
                    degrade_trace.push(common::DegradeTraceItem {
                        stage: "search.synthesize_answer".to_string(),
                        reason: error.to_string(),
                        impact:
                            "Returning Brave LLM Context evidence without final answer synthesis"
                                .to_string(),
                    });
                    if request.stream && answer.is_empty() {
                        sink.emit(AgentEvent::MessageDelta {
                            text: search_response.synthesized_answer.clone(),
                        })
                        .await;
                    }
                    (search_response.synthesized_answer.clone(), None)
                }
            }
        } else if answer.is_empty() {
            (
                search_response.synthesized_answer.clone(),
                search_response.llm_usage.clone(),
            )
        } else {
            (answer, search_response.llm_usage.clone())
        };

        let citations: Vec<common::Citation> = search_response
            .results
            .iter()
            .enumerate()
            .map(|(index, result)| common::Citation {
                citation_id: result.citation_index.unwrap_or(index + 1) as i64,
                doc_id: result.url.clone(),
                chunk_id: Some(format!(
                    "search:{}",
                    result.citation_index.unwrap_or(index + 1)
                )),
                page: None,
                doc_name: result.title.clone(),
                preview: Some(result.snippet.clone()),
                content: None,
                score: 1.0,
                layer: Some("search".to_string()),
                chunk_type: None,
                asset_id: None,
                caption: None,
                image_url: None,
                parser_backend: None,
                source_locator: Some(serde_json::json!({
                    "url": result.url.clone(),
                    "citation_index": result.citation_index.unwrap_or(index + 1),
                })),
                parse_run_id: None,
            })
            .collect();

        if !citations.is_empty() {
            sink.emit(AgentEvent::Citations {
                citations: citations.clone(),
            })
            .await;
        }

        let usage = synthesis_usage
            .as_ref()
            .or(search_response.llm_usage.as_ref());
        if let Some(u) = usage {
            sink.emit(AgentEvent::Usage {
                provider: u.provider.clone(),
                model: u.model.clone(),
                prompt_tokens: u.prompt_tokens as u64,
                completion_tokens: u.completion_tokens as u64,
                total_tokens: u.total_tokens as u64,
                request_count: 1,
                metadata: Default::default(),
            })
            .await;
        }

        let sources = search_response
            .results
            .iter()
            .map(|result| common::SourceRef {
                id: result.url.clone(),
                title: result.title.clone(),
                snippet: Some(result.snippet.clone()),
                doc_id: None,
                page: None,
            })
            .collect();

        let run_usage = usage.map(|u| AgentRunUsage {
            provider: u.provider.clone(),
            model: u.model.clone(),
            prompt_tokens: u.prompt_tokens as u64,
            completion_tokens: u.completion_tokens as u64,
            total_tokens: u.total_tokens as u64,
            request_count: 1,
        });
        let query_type = search_response.query_type.clone();
        let is_brave_llm_context = query_type == "brave_llm_context";
        let debug_payload = serde_json::json!({
            "query_type": query_type,
            "sub_queries": search_response.sub_queries.clone(),
            "result_count": search_response.results.len(),
            "evidence_fetch_mode": if is_brave_llm_context {
                "brave_llm_context_buffered"
            } else {
                "provider_stream"
            },
            "answer_synthesis_mode": answer_synthesis_mode,
        });
        emit_search_debug_trace_if_requested(request.debug, sink, debug_payload.clone()).await;

        sink.emit(AgentEvent::Done {
            final_message: Some(answer.clone()),
            usage: usage.map(|u| crate::agents::events::AgentUsage {
                provider: u.provider.clone(),
                model: u.model.clone(),
                prompt_tokens: u.prompt_tokens as u64,
                completion_tokens: u.completion_tokens as u64,
                total_tokens: u.total_tokens as u64,
            }),
        })
        .await;

        Ok(AgentRunResult {
            answer,
            citations,
            sources,
            degrade_trace,
            usage: run_usage,
            debug_payload: Some(debug_payload),
            ..Default::default()
        })
    }
}

async fn emit_search_debug_trace_if_requested(
    request_debug: bool,
    sink: &dyn AgentEventSink,
    payload: serde_json::Value,
) {
    if !request_debug {
        return;
    }
    sink.emit(AgentEvent::DebugTrace {
        kind: "search.execution".to_string(),
        payload,
    })
    .await;
}

async fn emit_search_update(
    update: avrag_search::SearchStreamUpdate,
    sink: &dyn AgentEventSink,
    answer: &mut String,
) {
    match update {
        avrag_search::SearchStreamUpdate::Searching { queries } => {
            let detail = if queries.is_empty() {
                None
            } else {
                Some(format!("Queries: {}", queries.join(" · ")))
            };
            sink.emit(AgentEvent::Activity {
                stage: "searching".to_string(),
                message: detail.unwrap_or_else(|| "Searching".to_string()),
            })
            .await;
        }
        avrag_search::SearchStreamUpdate::SourcesCollected { results } => {
            sink.emit(AgentEvent::Activity {
                stage: "reading_sources".to_string(),
                message: format!("Collected {} sources", results.len()),
            })
            .await;
        }
        avrag_search::SearchStreamUpdate::TextDelta { delta } => {
            answer.push_str(&delta);
            sink.emit(AgentEvent::MessageDelta { text: delta }).await;
        }
    }
}

async fn synthesize_brave_answer(
    synthesizer: Option<&dyn SearchAnswerSynthesizer>,
    temperature: Option<f32>,
    query: &str,
    search_response: &avrag_search::SearchResponse,
    stream: bool,
    sink: &dyn AgentEventSink,
) -> anyhow::Result<(String, Option<avrag_llm::LlmUsage>)> {
    let Some(synthesizer) = synthesizer else {
        anyhow::bail!("search answer synthesizer is not configured");
    };
    if search_response.results.is_empty() {
        anyhow::bail!("Brave LLM Context returned no sources");
    }

    let messages = build_search_answer_messages(query, &search_response.results);
    if stream {
        let (delta_tx, mut delta_rx) = tokio::sync::mpsc::unbounded_channel::<String>();
        let mut on_delta = move |delta: String| {
            let _ = delta_tx.send(delta);
        };
        let answer_stream = synthesizer.synthesize_stream(&messages, temperature, &mut on_delta);
        tokio::pin!(answer_stream);

        let response = loop {
            tokio::select! {
                delta = delta_rx.recv() => {
                    if let Some(delta) = delta {
                        sink.emit(AgentEvent::MessageDelta { text: delta }).await;
                    }
                }
                result = &mut answer_stream => {
                    break result?;
                }
            }
        };
        while let Ok(delta) = delta_rx.try_recv() {
            sink.emit(AgentEvent::MessageDelta { text: delta }).await;
        }
        Ok((response.answer, response.usage))
    } else {
        let response = synthesizer.synthesize(&messages, temperature).await?;
        Ok((response.answer, response.usage))
    }
}

fn build_search_answer_messages(
    query: &str,
    results: &[avrag_search::SearchResult],
) -> Vec<LlmChatMessage> {
    let evidence = results
        .iter()
        .enumerate()
        .map(|(index, result)| {
            format!(
                "[[{}]] title: {}\nurl: {}\nsnippet:\n{}",
                result.citation_index.unwrap_or(index + 1),
                result.title,
                result.url,
                result.snippet
            )
        })
        .collect::<Vec<_>>()
        .join("\n\n");

    vec![
        LlmChatMessage::system(
            "Answer the user's web-search question using only the provided Brave LLM Context evidence. Cite sources with [[n]] markers that match the evidence ids. If the evidence is insufficient, say so plainly.",
        ),
        LlmChatMessage::user(format!(
            "Question:\n{}\n\nBrave LLM Context evidence:\n{}",
            query.trim(),
            evidence
        )),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agents::events::CollectingSink;

    struct FakeSearchAnswerSynthesizer;

    #[async_trait::async_trait]
    impl SearchAnswerSynthesizer for FakeSearchAnswerSynthesizer {
        async fn synthesize(
            &self,
            _messages: &[LlmChatMessage],
            _temperature: Option<f32>,
        ) -> anyhow::Result<SynthesizedSearchAnswer> {
            Ok(SynthesizedSearchAnswer {
                answer: "non-stream synthesized [[1]]".to_string(),
                usage: Some(avrag_llm::LlmUsage {
                    prompt_tokens: 10,
                    completion_tokens: 4,
                    total_tokens: 14,
                    provider: "fake".to_string(),
                    model: "fake-search-llm".to_string(),
                }),
            })
        }

        async fn synthesize_stream(
            &self,
            _messages: &[LlmChatMessage],
            _temperature: Option<f32>,
            on_delta: &mut (dyn FnMut(String) + Send),
        ) -> anyhow::Result<SynthesizedSearchAnswer> {
            on_delta("stream ".to_string());
            on_delta("synthesized [[1]]".to_string());
            Ok(SynthesizedSearchAnswer {
                answer: "stream synthesized [[1]]".to_string(),
                usage: Some(avrag_llm::LlmUsage {
                    prompt_tokens: 10,
                    completion_tokens: 5,
                    total_tokens: 15,
                    provider: "fake".to_string(),
                    model: "fake-search-llm".to_string(),
                }),
            })
        }
    }

    #[tokio::test]
    async fn test_web_search_agent_without_executor_returns_error() {
        let agent = WebSearchAgent::new(None);
        let sink = CollectingSink::new();
        let req = AgentRequest {
            kind: crate::agents::AgentKind::Search,
            query: "hello".to_string(),
            notebook_id: None,
            session_id: None,
            doc_scope: vec![],
            messages: vec![],
            session_summary: None,
            user_preferences: None,
            working_memory: None,
            debug: false,
            stream: false,
            auth_context: serde_json::json!({}),
            metadata: Default::default(),
        };
        let result = agent.run(req, &sink).await;
        assert!(result.is_err());
        let events = sink.events();
        assert!(events.iter().any(|e| matches!(e, AgentEvent::Error { .. })));
    }

    #[tokio::test]
    async fn search_stream_updates_are_emitted_to_sink() {
        let sink = CollectingSink::new();
        let mut answer = String::new();

        emit_search_update(
            avrag_search::SearchStreamUpdate::Searching {
                queries: vec!["atlas".to_string()],
            },
            &sink,
            &mut answer,
        )
        .await;
        emit_search_update(
            avrag_search::SearchStreamUpdate::SourcesCollected {
                results: vec![avrag_search::SearchResult {
                    title: "Atlas".to_string(),
                    url: "https://example.com".to_string(),
                    snippet: "snippet".to_string(),
                    citation_index: Some(1),
                }],
            },
            &sink,
            &mut answer,
        )
        .await;
        emit_search_update(
            avrag_search::SearchStreamUpdate::TextDelta {
                delta: "answer".to_string(),
            },
            &sink,
            &mut answer,
        )
        .await;

        let events = sink.events();
        assert_eq!(answer, "answer");
        assert!(matches!(events[0], AgentEvent::Activity { .. }));
        assert!(matches!(events[1], AgentEvent::Activity { .. }));
        assert!(matches!(events[2], AgentEvent::MessageDelta { .. }));
    }

    #[tokio::test]
    async fn brave_answer_synthesis_streams_fake_llm_deltas_in_order() {
        let sink = CollectingSink::new();
        let search_response = avrag_search::SearchResponse {
            query_type: "brave_llm_context".to_string(),
            sub_queries: vec!["atlas rollback".to_string()],
            results: vec![avrag_search::SearchResult {
                title: "Atlas Checklist".to_string(),
                url: "https://example.com/atlas".to_string(),
                snippet: "Atlas uses the rollback checklist.".to_string(),
                citation_index: Some(1),
            }],
            synthesized_answer: "evidence fallback".to_string(),
            llm_usage: None,
        };
        let fake = FakeSearchAnswerSynthesizer;

        let (answer, usage) = synthesize_brave_answer(
            Some(&fake as &dyn SearchAnswerSynthesizer),
            Some(0.2),
            "How does Atlas handle rollback?",
            &search_response,
            true,
            &sink,
        )
        .await
        .unwrap();

        assert_eq!(answer, "stream synthesized [[1]]");
        assert_eq!(usage.as_ref().map(|usage| usage.total_tokens), Some(15));
        let deltas = sink
            .events()
            .into_iter()
            .filter_map(|event| match event {
                AgentEvent::MessageDelta { text } => Some(text),
                _ => None,
            })
            .collect::<Vec<_>>();
        assert_eq!(deltas, vec!["stream ", "synthesized [[1]]"]);
    }

    async fn emit_debug_trace_if_requested(request_debug: bool) {
        let sink = CollectingSink::new();
        emit_search_debug_trace_if_requested(
            request_debug,
            &sink,
            serde_json::json!({"internal": true}),
        )
        .await;
        let debug_events = sink
            .events()
            .into_iter()
            .filter(|event| matches!(event, AgentEvent::DebugTrace { .. }))
            .count();
        assert_eq!(debug_events, usize::from(request_debug));
    }

    #[tokio::test]
    async fn search_debug_trace_requires_debug_flag() {
        emit_debug_trace_if_requested(false).await;
    }

    #[tokio::test]
    async fn search_debug_trace_is_emitted_when_debug_flag_is_set() {
        emit_debug_trace_if_requested(true).await;
    }

    #[test]
    fn search_answer_prompt_contains_evidence_and_citation_contract() {
        let messages = build_search_answer_messages(
            "How does Atlas handle rollback?",
            &[avrag_search::SearchResult {
                title: "Atlas Checklist".to_string(),
                url: "https://example.com/atlas".to_string(),
                snippet: "Atlas uses the rollback checklist.".to_string(),
                citation_index: Some(1),
            }],
        );

        assert!(messages[0].content.contains("Cite sources with [[n]]"));
        assert!(
            messages[1]
                .content
                .contains("How does Atlas handle rollback?")
        );
        assert!(messages[1].content.contains("[[1]] title: Atlas Checklist"));
        assert!(messages[1].content.contains("https://example.com/atlas"));
    }
}
