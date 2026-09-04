use contracts::chat::{AnswerBlock, ChatEvent, ChatResponse};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TurnStatus {
    Idle,
    Streaming,
    Done,
    Error { code: String, message: String },
    Cancelled,
}

impl TurnStatus {
    pub fn is_terminal(&self) -> bool {
        matches!(
            self,
            TurnStatus::Done | TurnStatus::Error { .. } | TurnStatus::Cancelled
        )
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ActivityEntry {
    pub phase: String,
    pub title: String,
    pub detail: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct ChatTurnState {
    /// 本轮绑定的服务端权威 request_id（由 Start 事件确立）
    pub request_id: Option<String>,
    pub session_id: Option<String>,
    pub message_id: Option<i64>,
    pub agent_type: Option<String>,
    pub status: TurnStatus,
    /// 用户主气泡正文（严格只包含模型输出）
    pub answer_text: String,
    /// 服务端权威终态中的结构化答案块
    pub answer_blocks: Vec<AnswerBlock>,
    /// 思考/推理摘要（独立展示）
    pub reasoning_summary: String,
    /// 引用条目列表
    pub citations: Vec<serde_json::Value>,
    /// 进度活动列表（不污染主气泡）
    pub activities: Vec<ActivityEntry>,
    /// 诊断/跟踪阶段（不污染主气泡）
    pub trace_stages: Vec<String>,
    /// 服务端最终 Done payload 权威收束事实
    pub done_payload: Option<serde_json::Value>,
}

impl Default for TurnStatus {
    fn default() -> Self {
        Self::Idle
    }
}

/// 提取任何 ChatEvent 所属的 request_id
pub fn event_request_id(event: &ChatEvent) -> &str {
    match event {
        ChatEvent::Start { request_id, .. } => request_id,
        ChatEvent::OperationGuide { request_id, .. } => request_id,
        ChatEvent::Activity { request_id, .. } => request_id,
        ChatEvent::AnswerStart { request_id, .. } => request_id,
        ChatEvent::Trace { request_id, .. } => request_id,
        ChatEvent::Token { request_id, .. } => request_id,
        ChatEvent::ReasoningSummaryDelta { request_id, .. } => request_id,
        ChatEvent::Citations { request_id, .. } => request_id,
        ChatEvent::Done { request_id, .. } => request_id,
        ChatEvent::Error { request_id, .. } => request_id,
    }
}

pub fn reduce_chat_event(state: &mut ChatTurnState, event: ChatEvent) -> bool {
    // 1. 终态严格守卫：一旦进入 Done / Error / Cancelled，拒绝一切后续事件继续修改
    if state.status.is_terminal() {
        return false;
    }

    let incoming_req_id = event_request_id(&event);

    // 2. Request Correlation 关联守卫：
    // 如果当前轮次已通过 Start 事件绑定了 request_id，严格拒绝任何非当前 request_id 的迟到事件
    if let Some(active_req_id) = &state.request_id {
        if incoming_req_id != active_req_id {
            return false;
        }
    } else {
        // 正常流由 Start 建立 request_id；后端也允许预检失败时只发送一条 Error。
        // 跨流隔离由宿主的本地 StreamScope 在进入 reducer 前完成。
        if !matches!(event, ChatEvent::Start { .. } | ChatEvent::Error { .. }) {
            return false;
        }
    }

    match event {
        ChatEvent::Start {
            request_id,
            session_id,
        } => {
            state.request_id = Some(request_id);
            state.session_id = Some(session_id);
            state.status = TurnStatus::Streaming;
        }
        ChatEvent::OperationGuide { .. } => {
            // 操作指南为内部指引，不修改用户可见状态
        }
        ChatEvent::Activity {
            phase,
            title,
            detail,
            ..
        } => {
            state.activities.push(ActivityEntry {
                phase,
                title,
                detail,
            });
        }
        ChatEvent::AnswerStart {
            session_id,
            message_id,
            agent_type,
            ..
        } => {
            state.session_id = Some(session_id);
            state.message_id = Some(message_id);
            state.agent_type = Some(agent_type);
        }
        ChatEvent::Trace { stage, .. } => {
            state.trace_stages.push(stage);
        }
        ChatEvent::Token { content, .. } => {
            state.answer_text.push_str(&content);
        }
        ChatEvent::ReasoningSummaryDelta { content, .. } => {
            state.reasoning_summary.push_str(&content);
        }
        ChatEvent::Citations { citations, .. } => {
            state.citations.extend(citations);
        }
        ChatEvent::Done {
            session_id,
            message_id,
            payload,
            ..
        } => {
            state.session_id = Some(session_id);
            state.message_id = Some(message_id);

            state.done_payload = Some(payload.clone());
            match serde_json::from_value::<ChatResponse>(payload) {
                Ok(response) => {
                    state.answer_text = authoritative_answer_text(&response);
                    state.answer_blocks = response.answer_blocks;
                    state.citations = merge_done_citations(
                        response
                            .citations
                            .into_iter()
                            .map(|citation| {
                                serde_json::to_value(citation)
                                    .expect("Citation must serialize to JSON")
                            })
                            .collect(),
                        &state.citations,
                    );
                    state.status = TurnStatus::Done;
                }
                Err(error) => {
                    state.status = TurnStatus::Error {
                        code: "invalid_done_payload".to_string(),
                        message: error.to_string(),
                    };
                }
            }
        }
        ChatEvent::Error {
            request_id,
            code,
            message,
        } => {
            if state.request_id.is_none() {
                state.request_id = Some(request_id);
            }
            state.status = TurnStatus::Error { code, message };
        }
    }

    true
}

fn authoritative_answer_text(response: &ChatResponse) -> String {
    if !response.answer.trim().is_empty() {
        return response.answer.clone();
    }

    response
        .answer_blocks
        .iter()
        .filter_map(|block| match block {
            AnswerBlock::Text { text, .. } => Some(text.as_str()),
            AnswerBlock::Image { .. } => None,
        })
        .collect()
}

fn merge_done_citations(
    incoming: Vec<serde_json::Value>,
    current: &[serde_json::Value],
) -> Vec<serde_json::Value> {
    if incoming.is_empty() {
        return current.to_vec();
    }

    incoming
        .into_iter()
        .map(|mut citation| {
            let Some(key) = citation_key(&citation) else {
                return citation;
            };
            let Some(prior) = current
                .iter()
                .find(|candidate| citation_key(candidate).as_deref() == Some(key.as_str()))
            else {
                return citation;
            };

            preserve_nonempty_field(&mut citation, prior, "content");
            preserve_nonempty_field(&mut citation, prior, "preview");
            citation
        })
        .collect()
}

fn citation_key(citation: &serde_json::Value) -> Option<String> {
    citation
        .get("chunk_id")
        .and_then(serde_json::Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(|value| format!("chunk:{value}"))
        .or_else(|| {
            citation
                .get("citation_id")
                .and_then(serde_json::Value::as_i64)
                .map(|value| format!("id:{value}"))
        })
}

fn preserve_nonempty_field(
    citation: &mut serde_json::Value,
    prior: &serde_json::Value,
    field: &str,
) {
    let incoming_has_text = citation
        .get(field)
        .and_then(serde_json::Value::as_str)
        .is_some_and(|value| !value.trim().is_empty());
    if incoming_has_text {
        return;
    }

    let Some(prior_text) = prior
        .get(field)
        .and_then(serde_json::Value::as_str)
        .filter(|value| !value.trim().is_empty())
    else {
        return;
    };
    if let Some(object) = citation.as_object_mut() {
        object.insert(
            field.to_string(),
            serde_json::Value::String(prior_text.to_string()),
        );
    }
}
