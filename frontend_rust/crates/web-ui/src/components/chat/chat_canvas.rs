use crate::reducer::{ChatTurnState, TurnStatus, reduce_chat_event};
use crate::session::{ConversationManager, MessageRole};
use contracts::chat::{ChatEvent, ChatRequest};
use web_sdk::Cancellation;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StreamScope {
    turn_generation: u64,
    conversation_epoch: u64,
}

pub struct PreparedUserTurn {
    pub request: ChatRequest,
    pub stream_scope: StreamScope,
    pub cancellation: Cancellation,
}

pub struct ChatCanvasModel {
    manager: ConversationManager,
    live_turn: ChatTurnState,
    cancellation: Option<Cancellation>,
    next_turn_generation: u64,
    active_stream_scope: Option<StreamScope>,
}

impl ChatCanvasModel {
    pub fn new() -> Self {
        Self {
            manager: ConversationManager::new(),
            live_turn: ChatTurnState::default(),
            cancellation: None,
            next_turn_generation: 0,
            active_stream_scope: None,
        }
    }

    pub fn manager(&self) -> &ConversationManager {
        &self.manager
    }

    pub fn live_turn(&self) -> &ChatTurnState {
        &self.live_turn
    }

    pub fn is_streaming(&self) -> bool {
        matches!(self.live_turn.status, TurnStatus::Streaming)
    }

    /// 用户发起提问
    pub fn prepare_user_turn(&mut self, query: &str) -> PreparedUserTurn {
        self.invalidate_active_stream();
        self.manager.append_user_message(query);
        self.live_turn = ChatTurnState {
            request_id: None, // 等待服务端 Start 事件确立权威 request_id
            session_id: self.manager.active.session_id.clone(),
            status: TurnStatus::Streaming,
            ..Default::default()
        };
        self.next_turn_generation += 1;
        let stream_scope = StreamScope {
            turn_generation: self.next_turn_generation,
            conversation_epoch: self.manager.conversation_epoch,
        };
        let cancellation = Cancellation::new();
        self.cancellation = Some(cancellation.clone());
        self.active_stream_scope = Some(stream_scope);

        PreparedUserTurn {
            request: ChatRequest {
                query: query.to_string(),
                workspace_id: self.manager.active.workspace_id.clone(),
                session_id: self.manager.active.session_id.clone(),
                agent_type: "chat".to_string(),
                capabilities: None,
                client_context: None,
                client_ip: None,
                source_type: None,
                source_token: None,
                doc_scope: vec![],
                messages: vec![],
                stream: true,
                debug: false,
                language: None,
                format_hint: None,
                turnstile_token: None,
            },
            stream_scope,
            cancellation,
        }
    }

    /// 接收流式事件并更新
    pub fn on_event(&mut self, stream_scope: StreamScope, event: ChatEvent) -> bool {
        // 本地流身份必须先于服务端 request_id 校验。旧流的迟到 Start 也不能
        // 抢先绑定到新 turn，更不能先污染共享 live_turn 再补做会话检查。
        if self.active_stream_scope != Some(stream_scope)
            || stream_scope.conversation_epoch != self.manager.conversation_epoch
        {
            return false;
        }

        let prev_status = self.live_turn.status.clone();
        if !reduce_chat_event(&mut self.live_turn, event) {
            return false;
        }

        if matches!(
            self.live_turn.status,
            TurnStatus::Streaming | TurnStatus::Done
        ) {
            if let Some(sid) = &self.live_turn.session_id {
                if self.manager.active.session_id.as_deref() != Some(sid) {
                    self.manager.active.session_id = Some(sid.clone());
                }
            }
        }

        if prev_status != TurnStatus::Done && self.live_turn.status == TurnStatus::Done {
            let answer = self.live_turn.answer_text.clone();
            let answer_blocks = self.live_turn.answer_blocks.clone();
            let reasoning = if self.live_turn.reasoning_summary.is_empty() {
                None
            } else {
                Some(self.live_turn.reasoning_summary.as_str())
            };
            let citations = self.live_turn.citations.clone();
            self.manager
                .append_assistant_message(&answer, answer_blocks, reasoning, citations);
        }

        if self.live_turn.status.is_terminal() {
            self.cancellation = None;
            self.active_stream_scope = None;
        }

        true
    }

    /// 用户主动点击停止
    pub fn cancel(&mut self) -> bool {
        if !matches!(self.live_turn.status, TurnStatus::Streaming) {
            return false;
        }

        if let Some(cancel) = self.cancellation.take() {
            cancel.cancel();
        }
        self.active_stream_scope = None;
        self.live_turn.status = TurnStatus::Cancelled;
        true
    }

    pub fn new_personal_chat(&mut self, model_role: Option<&str>) {
        self.invalidate_active_stream();
        self.manager.new_personal_chat(model_role);
    }

    pub fn switch_to_personal_session(&mut self, session_id: &str) {
        self.invalidate_active_stream();
        self.manager.switch_to_personal_session(session_id);
    }

    pub fn switch_to_workspace(&mut self, workspace_id: &str, session_id: Option<&str>) {
        self.invalidate_active_stream();
        self.manager.switch_to_workspace(workspace_id, session_id);
    }

    /// 重试上一轮提问
    pub fn retry_last(&mut self) -> Option<PreparedUserTurn> {
        let last_user_query = self
            .manager
            .active
            .messages
            .iter()
            .rfind(|m| m.role == MessageRole::User)
            .map(|m| m.content.clone())?;

        // 移除末尾残留回复
        while let Some(last) = self.manager.active.messages.last() {
            if last.role == MessageRole::Assistant {
                self.manager.active.messages.pop();
            } else {
                break;
            }
        }
        // 弹出上一条 User 提问，避免 prepare_user_turn 重复追加
        if let Some(last) = self.manager.active.messages.last() {
            if last.role == MessageRole::User && last.content == last_user_query {
                self.manager.active.messages.pop();
            }
        }

        Some(self.prepare_user_turn(&last_user_query))
    }

    fn invalidate_active_stream(&mut self) {
        if let Some(cancel) = self.cancellation.take() {
            cancel.cancel();
        }
        self.active_stream_scope = None;
        self.live_turn = ChatTurnState::default();
    }
}

impl Default for ChatCanvasModel {
    fn default() -> Self {
        Self::new()
    }
}
