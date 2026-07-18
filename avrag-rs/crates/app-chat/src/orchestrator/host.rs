//! Orchestrator host: materialize → dispatch workers → chat exit (Option B).
//!
//! O1: first wave runs **all** materialized channels with [`default_brief`]
//! (§7.1 structure + §7.2 invariant by construction). Multi-hop LLM re-dispatch is O2.
//!
//! V1 (evidence store): worker tool results are normalized into a shared
//! [`EvidenceStore`] (monotonic `E{n}` ids, doc identity joined from docscope
//! metadata); the chat exit receives listings + worker digests and cites
//! `[[E:id]]`; the host rewrites markers to product citations after the run.

use agent_loop::events::{AgentEventSink, CollectingSink};
use agent_loop::runtime::{AgentRequest, AgentRunResult};
use async_trait::async_trait;
use common::AppError;

use super::chat_exit::{direct_handoff, query_for_agent, synthesize_handoff};
use super::invariant::{assert_complete, default_brief};
use super::materialize::materialize_channels;
use super::store::EvidenceStore;
use super::types::{
    Channel, ChannelNote, ChatHandoff, DispatchRecord, PackStatus, TaskBrief,
};
use super::workers::{channel_note_from_run, finalize_answer_evidence};
use crate::capabilities::CapabilitySet;

/// Abstraction so tests can mock channel + chat runs without LLM.
#[async_trait]
pub trait OrchestratorExecutor: Send + Sync {
    async fn run_channel(
        &self,
        channel: Channel,
        brief: &TaskBrief,
        base: &AgentRequest,
    ) -> Result<AgentRunResult, AppError>;

    async fn run_chat(
        &self,
        handoff: &ChatHandoff,
        base: &AgentRequest,
    ) -> Result<AgentRunResult, AppError>;
}

/// Result of an orchestrated turn.
#[derive(Debug)]
pub struct OrchestratedTurn {
    pub answer_result: AgentRunResult,
    pub store: EvidenceStore,
    pub records: Vec<DispatchRecord>,
    pub handoff: ChatHandoff,
    pub agent_type_label: String,
}

/// Feature flag: `AGENT_ORCHESTRATOR_V1=1` (or true/yes/on).
pub fn orchestrator_v1_enabled() -> bool {
    match std::env::var("AGENT_ORCHESTRATOR_V1") {
        Ok(v) => {
            let t = v.trim().to_ascii_lowercase();
            t == "1" || t == "true" || t == "yes" || t == "on"
        }
        Err(_) => false,
    }
}

/// Map a materialized channel to its localized delegate progress fact.
fn delegate_fact(channel: Channel, brief: &TaskBrief) -> agent_loop::progress::WorkFact {
    let kind = match channel {
        Channel::Rag => agent_loop::progress::ProgressKind::DelegateRag,
        Channel::Search => agent_loop::progress::ProgressKind::DelegateSearch,
    };
    agent_loop::progress::WorkFact::delegate(kind, &brief.goal)
}

struct ChannelOutcome {
    record: DispatchRecord,
    note: ChannelNote,
}

/// Run one channel dispatch: worker run → store insert → ledger entry.
async fn dispatch_channel(
    channel: Channel,
    query: &str,
    base_request: &AgentRequest,
    executor: &dyn OrchestratorExecutor,
    store: &mut EvidenceStore,
    sink: &dyn AgentEventSink,
) -> ChannelOutcome {
    let brief = default_brief(channel, query);
    agent_loop::progress::emit_work_fact(sink, delegate_fact(channel, &brief)).await;

    let dispatch_id = uuid::Uuid::new_v4().to_string();
    match executor.run_channel(channel, &brief, base_request).await {
        Ok(run) => {
            let inserted = store.insert_from_tool_results(channel, &run.tool_results);
            let status = if inserted > 0 {
                PackStatus::Ok
            } else {
                PackStatus::Empty
            };
            tracing::info!(
                channel = channel.as_str(),
                status = ?status,
                item_count = inserted,
                "orchestrator dispatch finished"
            );
            ChannelOutcome {
                record: DispatchRecord {
                    channel,
                    dispatch_id,
                    status,
                    item_count: inserted,
                    error: None,
                },
                note: ChannelNote {
                    channel,
                    status,
                    item_count: inserted,
                    note: channel_note_from_run(&run),
                    error: None,
                },
            }
        }
        Err(e) => {
            tracing::warn!(
                channel = channel.as_str(),
                error = %e,
                "orchestrator dispatch failed"
            );
            ChannelOutcome {
                record: DispatchRecord {
                    channel,
                    dispatch_id,
                    status: PackStatus::Error,
                    item_count: 0,
                    error: Some(e.to_string()),
                },
                note: ChannelNote {
                    channel,
                    status: PackStatus::Error,
                    item_count: 0,
                    note: None,
                    error: Some(e.to_string()),
                },
            }
        }
    }
}

/// Run orchestrated turn: materialize → workers → chat.
pub async fn run_orchestrated_turn(
    caps: CapabilitySet,
    base_request: &AgentRequest,
    executor: &dyn OrchestratorExecutor,
    sink: &dyn AgentEventSink,
    docscope: Option<&common::DocScopeMetadata>,
) -> Result<OrchestratedTurn, AppError> {
    let label = caps.agent_type_label().to_string();
    let channels = materialize_channels(caps);
    let query = base_request.query.clone();

    // Pure chat: no workers
    if channels.is_empty() {
        let handoff = direct_handoff(&query);
        agent_loop::progress::emit_work_fact(
            sink,
            agent_loop::progress::WorkFact::understand(&query),
        )
        .await;
        let answer_result = executor.run_chat(&handoff, base_request).await?;
        return Ok(OrchestratedTurn {
            answer_result,
            store: EvidenceStore::from_docscope(docscope),
            records: vec![],
            handoff,
            agent_type_label: label,
        });
    }

    let mut store = EvidenceStore::from_docscope(docscope);
    let mut records: Vec<DispatchRecord> = Vec::new();
    let mut channel_notes: Vec<ChannelNote> = Vec::new();

    // §7.1 first wave: every materialized channel
    for ch in &channels {
        let outcome = dispatch_channel(*ch, &query, base_request, executor, &mut store, sink).await;
        records.push(outcome.record);
        channel_notes.push(outcome.note);
    }

    // §7.2 assert. The first wave above always pushes a record per materialized
    // channel (even on worker error), so this recovery branch is unreachable
    // today — it is kept as defense for the O2 LLM-dispatch path, where the
    // orchestrator may skip a channel and the invariant must force a default run.
    if let Err(missing) = assert_complete(&channels, &records) {
        for ch in missing.channels {
            let outcome =
                dispatch_channel(ch, &query, base_request, executor, &mut store, sink).await;
            records.push(outcome.record);
            channel_notes.push(outcome.note);
        }
    }
    assert_complete(&channels, &records).map_err(|m| {
        AppError::internal(format!("orchestrator completion invariant failed: {m}"))
    })?;

    let handoff = synthesize_handoff(
        &query,
        store.source_docs().to_vec(),
        store.listings(),
        channel_notes,
        &records,
        None,
    );
    agent_loop::progress::emit_work_fact(
        sink,
        agent_loop::progress::WorkFact::compose_answer(),
    )
    .await;

    let mut answer_result = executor.run_chat(&handoff, base_request).await?;
    // Single point where E-markers become product markers + citations; dangling
    // or fabricated markers are stripped here.
    finalize_answer_evidence(&mut answer_result, &store);

    Ok(OrchestratedTurn {
        answer_result,
        store,
        records,
        handoff,
        agent_type_label: label,
    })
}

/// Production executor: runs single-channel / chat via UnifiedAgentService.
pub struct AgentServiceExecutor {
    pub agent_service: std::sync::Arc<crate::agents::service::UnifiedAgentService>,
}

#[async_trait]
impl OrchestratorExecutor for AgentServiceExecutor {
    async fn run_channel(
        &self,
        channel: Channel,
        brief: &TaskBrief,
        base: &AgentRequest,
    ) -> Result<AgentRunResult, AppError> {
        let mut req = base.clone();
        // Keep the original user query: retrieval (`inject_retrieval_query` /
        // codegen) must run on the user's words, not on the English brief
        // wrapper. The brief travels via system prompt parts instead.
        req.kind = match channel {
            Channel::Rag => crate::agents::AgentKind::Rag,
            Channel::Search => crate::agents::AgentKind::Search,
        };
        // Single-channel capability metadata so assembly path stays pure if used
        let caps = match channel {
            Channel::Rag => serde_json::json!(["rag"]),
            Channel::Search => serde_json::json!(["search"]),
        };
        req.metadata.insert("capabilities".into(), caps);
        // Prefer loading mode via kind; strip dual assembled config so worker is pure
        req.metadata.remove("assembled_mode_config");
        req.metadata.remove("system_prompt_parts");
        // Inject single-channel assembled config
        let cap_set = match channel {
            Channel::Rag => CapabilitySet {
                rag: true,
                search: false,
            },
            Channel::Search => CapabilitySet {
                rag: false,
                search: true,
            },
        };
        if let Ok(assembled) = crate::assemble_mode(cap_set) {
            let mut parts = assembled.system_prompt_parts;
            // Brief + worker-output slim: the worker's final message is an
            // internal hand-off; the chat exit writes the user answer (Option B).
            parts.push(format!(
                "## Task brief (orchestrator)\n{}\n\n\
                 Execute only this brief. Your final message is an internal hand-off, \
                 not the user-facing answer: keep it a concise evidence summary \
                 (key facts with source pointers, bullet points); another agent \
                 writes the user answer from your evidence.",
                brief.goal
            ));
            req.metadata.insert(
                "assembled_mode_config".into(),
                serde_json::to_value(&assembled.config).unwrap_or(serde_json::json!({})),
            );
            req.metadata.insert(
                "system_prompt_parts".into(),
                serde_json::to_value(&parts).unwrap_or(serde_json::json!([])),
            );
        }
        req.stream = false;
        let sink = CollectingSink::new();
        self.agent_service.run(req, &sink).await
    }

    async fn run_chat(
        &self,
        handoff: &ChatHandoff,
        base: &AgentRequest,
    ) -> Result<AgentRunResult, AppError> {
        let mut req = base.clone();
        req.query = query_for_agent(handoff);
        req.kind = crate::agents::AgentKind::Chat;
        req.metadata.insert(
            "capabilities".into(),
            serde_json::json!([]),
        );
        req.metadata.remove("assembled_mode_config");
        if let Ok(assembled) = crate::assemble_mode(CapabilitySet::default()) {
            req.metadata.insert(
                "assembled_mode_config".into(),
                serde_json::to_value(&assembled.config).unwrap_or(serde_json::json!({})),
            );
            req.metadata.insert(
                "system_prompt_parts".into(),
                serde_json::to_value(&assembled.system_prompt_parts)
                    .unwrap_or(serde_json::json!([])),
            );
        }
        // Synthesize needs evidence in the prompt (already in query); use prose chat
        req.stream = base.stream;
        let sink = CollectingSink::new();
        self.agent_service.run(req, &sink).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::orchestrator::types::PackStatus;
    use agent_loop::runtime::AgentRequest;
    use contracts::auth_runtime::{AuthContext, SubjectKind, UserId};
    use std::sync::Mutex;
    use uuid::Uuid;

    struct MockExec {
        channels_run: Mutex<Vec<Channel>>,
    }

    #[async_trait]
    impl OrchestratorExecutor for MockExec {
        async fn run_channel(
            &self,
            channel: Channel,
            _brief: &TaskBrief,
            _base: &AgentRequest,
        ) -> Result<AgentRunResult, AppError> {
            self.channels_run.lock().unwrap().push(channel);
            let mut r = AgentRunResult::default();
            if channel == Channel::Search {
                r.tool_results = vec![contracts::ToolResult {
                    tool: "web_search".into(),
                    version: "1".into(),
                    status: contracts::ToolStatus::Ok,
                    data: Some(serde_json::json!({
                        "results": [{"url": "https://a.com", "title": "A", "snippet": "best practice"}]
                    })),
                    trace: None,
                }];
            }
            Ok(r)
        }

        async fn run_chat(
            &self,
            handoff: &ChatHandoff,
            _base: &AgentRequest,
        ) -> Result<AgentRunResult, AppError> {
            let mut r = AgentRunResult::default();
            r.answer = if handoff.partial_notices.is_empty() {
                "full answer".into()
            } else {
                format!(
                    "partial: {} | {}",
                    handoff.partial_notices.join("; "),
                    "工作区未命中相关段落；以下基于网页。"
                )
            };
            Ok(r)
        }
    }

    fn base_req(q: &str) -> AgentRequest {
        AgentRequest {
            kind: crate::agents::AgentKind::Chat,
            query: q.into(),
            workspace_id: None,
            session_id: None,
            doc_scope: vec!["doc1".into()],
            messages: vec![],
            user_preferences: None,
            debug: false,
            stream: false,
            language: None,
            preferred_tools: vec![],
            format_hint: None,
            max_iterations: None,
            auth: AuthContext::new(UserId::from(Uuid::nil()), SubjectKind::User),
            docscope_metadata: None,
            metadata: Default::default(),
            cancellation_token: None,
            guard_pipeline: None,
        }
    }

    #[tokio::test]
    async fn pure_chat_skips_workers() {
        let ex = MockExec {
            channels_run: Mutex::new(vec![]),
        };
        let sink = CollectingSink::new();
        let turn = run_orchestrated_turn(CapabilitySet::default(), &base_req("hi"), &ex, &sink, None)
            .await
            .unwrap();
        assert!(ex.channels_run.lock().unwrap().is_empty());
        assert!(turn.records.is_empty());
        assert_eq!(turn.agent_type_label, "chat");
    }

    #[tokio::test]
    async fn dual_runs_both_channels() {
        let ex = MockExec {
            channels_run: Mutex::new(vec![]),
        };
        let sink = CollectingSink::new();
        let caps = CapabilitySet {
            rag: true,
            search: true,
        };
        let turn = run_orchestrated_turn(caps, &base_req("报告与最佳实践差距"), &ex, &sink, None)
            .await
            .unwrap();
        let ran = ex.channels_run.lock().unwrap().clone();
        assert!(ran.contains(&Channel::Rag));
        assert!(ran.contains(&Channel::Search));
        assert_eq!(turn.records.len(), 2);
        // rag empty → partial notice with 未命中 wording
        assert!(
            turn.handoff.partial_notices.iter().any(|n| n.contains("未命中") || n.contains("empty")),
            "notices={:?}",
            turn.handoff.partial_notices
        );
        // search returned one web entry → it is in the store + handoff listings
        assert_eq!(turn.store.count_channel(Channel::Search), 1);
        assert!(turn.handoff.listings.iter().any(|l| l.channel == Channel::Search));
        assert!(!crate::orchestrator::invariant::looks_like_user_did_not_provide_doc(
            &turn.answer_result.answer
        ));
    }

    #[tokio::test]
    async fn rag_only_empty_records_empty_status() {
        let ex = MockExec {
            channels_run: Mutex::new(vec![]),
        };
        let sink = CollectingSink::new();
        let turn = run_orchestrated_turn(
            CapabilitySet {
                rag: true,
                search: false,
            },
            &base_req("总结文档"),
            &ex,
            &sink,
            None,
        )
        .await
        .unwrap();
        assert_eq!(*ex.channels_run.lock().unwrap(), vec![Channel::Rag]);
        assert_eq!(turn.records[0].status, PackStatus::Empty);
        assert_eq!(turn.records[0].item_count, 0);
    }

    #[tokio::test]
    async fn citations_rebuilt_from_store_eids() {
        struct CiteMockExec;
        #[async_trait]
        impl OrchestratorExecutor for CiteMockExec {
            async fn run_channel(
                &self,
                channel: Channel,
                _brief: &TaskBrief,
                _base: &AgentRequest,
            ) -> Result<AgentRunResult, AppError> {
                let mut r = AgentRunResult::default();
                r.tool_results = vec![match channel {
                    Channel::Rag => contracts::ToolResult {
                        tool: "dense_retrieval".into(),
                        version: "1".into(),
                        status: contracts::ToolStatus::Ok,
                        data: Some(serde_json::json!([
                            {"chunk_id": "chunk-a", "doc_id": "doc1", "text": "doc evidence", "score": 0.9, "page": 4}
                        ])),
                        trace: None,
                    },
                    Channel::Search => contracts::ToolResult {
                        tool: "web_search".into(),
                        version: "1".into(),
                        status: contracts::ToolStatus::Ok,
                        data: Some(serde_json::json!({
                            "results": [
                                {"url": "https://a.example", "title": "A", "snippet": "web evidence"}
                            ]
                        })),
                        trace: None,
                    },
                }];
                Ok(r)
            }

            async fn run_chat(
                &self,
                _handoff: &ChatHandoff,
                _base: &AgentRequest,
            ) -> Result<AgentRunResult, AppError> {
                let mut r = AgentRunResult::default();
                r.answer = "文档证据 [[E1]]，网页佐证 [[E2]]。编造 [[E9]]。".into();
                Ok(r)
            }
        }

        let caps = CapabilitySet {
            rag: true,
            search: true,
        };
        let sink = CollectingSink::new();
        let turn = run_orchestrated_turn(caps, &base_req("对比"), &CiteMockExec, &sink, None)
            .await
            .unwrap();
        // Valid E-ids became product markers; fabricated E9 stripped.
        assert!(turn.answer_result.answer.contains("[[cite:chunk-a]]"));
        assert!(turn.answer_result.answer.contains("[[web:1]]"));
        assert!(!turn.answer_result.answer.contains("[[E"));
        assert_eq!(turn.answer_result.citations.len(), 2);
        assert!(
            turn.answer_result
                .citations
                .iter()
                .any(|c| c.chunk_id.as_deref() == Some("chunk-a") && c.page == Some(4))
        );
        assert!(
            turn.answer_result
                .citations
                .iter()
                .any(|c| c.layer.as_deref() == Some("search") && c.doc_id == "https://a.example")
        );
        assert_eq!(turn.answer_result.sources.len(), 1);
    }

    #[tokio::test]
    async fn worker_keeps_user_query_brief_goes_to_prompt_parts() {
        use agent_loop::runtime::Agent;

        struct CaptureAgent(std::sync::Arc<Mutex<Option<AgentRequest>>>);
        #[async_trait]
        impl Agent for CaptureAgent {
            async fn run(
                &self,
                request: AgentRequest,
                _sink: &dyn agent_loop::events::AgentEventSink,
            ) -> Result<AgentRunResult, AppError> {
                *self.0.lock().unwrap() = Some(request);
                Ok(AgentRunResult::default())
            }
        }

        let captured = std::sync::Arc::new(Mutex::new(None));
        let svc = std::sync::Arc::new(crate::agents::service::UnifiedAgentService::new(
            Box::new(CaptureAgent(captured.clone())),
        ));
        let exec = AgentServiceExecutor {
            agent_service: svc,
        };
        exec.run_channel(
            Channel::Rag,
            &TaskBrief::new("brief goal text"),
            &base_req("用户原始问题"),
        )
        .await
        .unwrap();

        let req = captured.lock().unwrap().clone().expect("captured request");
        // Query stays the user's words (retrieval must not see the brief wrapper).
        assert_eq!(req.query, "用户原始问题");
        let parts = req
            .metadata
            .get("system_prompt_parts")
            .and_then(|v| v.as_array())
            .cloned()
            .unwrap_or_default();
        assert!(
            parts
                .iter()
                .filter_map(|p| p.as_str())
                .any(|s| s.contains("brief goal text") && s.contains("internal hand-off")),
            "brief part missing: {parts:?}"
        );
    }
}
