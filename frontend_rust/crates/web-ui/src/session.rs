use contracts::{
    chat::{AnswerBlock, ChatMessage},
    workspaces::{ChatSession, ConversationScopeKind},
};

#[derive(Debug, Clone, PartialEq)]
pub struct ConversationMessage {
    pub id: String,
    pub session_id: Option<String>,
    pub message_id: Option<i64>,
    pub role: MessageRole,
    pub content: String,
    pub answer_blocks: Vec<AnswerBlock>,
    pub reasoning: Option<String>,
    pub citations: Vec<serde_json::Value>,
    pub created_at: String,
}

impl ConversationMessage {
    pub fn from_wire(message: &ChatMessage) -> Option<Self> {
        let role = match message.role.as_str() {
            "user" => MessageRole::User,
            "assistant" => MessageRole::Assistant,
            _ => return None,
        };
        Some(Self {
            id: message.id.to_string(),
            session_id: Some(message.session_id.clone()),
            message_id: Some(message.id),
            role,
            content: message.content.clone(),
            answer_blocks: message.answer_blocks.clone(),
            reasoning: None,
            citations: message
                .citations
                .iter()
                .filter_map(|citation| serde_json::to_value(citation).ok())
                .collect(),
            created_at: message.created_at.clone(),
        })
    }
}

pub fn messages_from_wire(messages: &[ChatMessage]) -> Vec<ConversationMessage> {
    messages
        .iter()
        .filter_map(ConversationMessage::from_wire)
        .collect()
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MessageRole {
    User,
    Assistant,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ActiveConversation {
    pub session_id: Option<String>,
    pub workspace_id: Option<String>,
    pub model_role: String,
    pub scope_kind: ConversationScopeKind,
    pub messages: Vec<ConversationMessage>,
}

impl Default for ActiveConversation {
    fn default() -> Self {
        Self {
            session_id: None,
            workspace_id: None,
            model_role: "quick_chat".to_string(),
            scope_kind: ConversationScopeKind::Personal,
            messages: Vec::new(),
        }
    }
}

pub struct ConversationManager {
    pub active: ActiveConversation,
    pub session_list: Vec<ChatSession>,
    /// 会话身份纪元计数器（切换会话时递增，防止跨会话串流）
    pub conversation_epoch: u64,
}

impl ConversationManager {
    pub fn new() -> Self {
        Self {
            active: ActiveConversation::default(),
            session_list: Vec::new(),
            conversation_epoch: 0,
        }
    }

    /// 新建个人对话（Canonical /chat：workspace_id 为空，不暗建工作区）
    pub fn new_personal_chat(&mut self, model_role: Option<&str>) {
        self.conversation_epoch += 1;
        self.active = ActiveConversation {
            session_id: None,
            workspace_id: None,
            model_role: model_role.unwrap_or("quick_chat").to_string(),
            scope_kind: ConversationScopeKind::Personal,
            messages: Vec::new(),
        };
    }

    /// 恢复已有个人对话（Canonical /chat/:sessionId，尚无列表元数据时）
    pub fn switch_to_personal_session(&mut self, session_id: &str) {
        self.conversation_epoch += 1;
        self.active = ActiveConversation {
            session_id: Some(session_id.to_string()),
            workspace_id: None,
            model_role: self.active.model_role.clone(),
            scope_kind: ConversationScopeKind::Personal,
            messages: Vec::new(),
        };
    }

    /// 恢复已有会话，保留 wire 上的 workspace / scope / model_role。
    pub fn switch_to_session(&mut self, session: &ChatSession) {
        self.conversation_epoch += 1;
        self.active = ActiveConversation {
            session_id: Some(session.id.clone()),
            workspace_id: session.workspace_id.clone(),
            model_role: session.model_role.clone(),
            scope_kind: session.scope_kind,
            messages: Vec::new(),
        };
    }

    /// 切换到工作区上下文（工作区默认为 agent，递增 epoch）
    pub fn switch_to_workspace(&mut self, workspace_id: &str, session_id: Option<&str>) {
        self.conversation_epoch += 1;
        self.active = ActiveConversation {
            session_id: session_id.map(|s| s.to_string()),
            workspace_id: Some(workspace_id.to_string()),
            model_role: "agent".to_string(),
            scope_kind: ConversationScopeKind::Workspace,
            messages: Vec::new(),
        };
    }

    /// 记录用户提问
    pub fn append_user_message(&mut self, text: &str) {
        let sid = self.active.session_id.clone();
        self.active.messages.push(ConversationMessage {
            id: format!("msg-user-{}", self.active.messages.len() + 1),
            session_id: sid,
            message_id: None,
            role: MessageRole::User,
            content: text.to_string(),
            answer_blocks: Vec::new(),
            reasoning: None,
            citations: Vec::new(),
            created_at: "now".to_string(),
        });
    }

    /// 记录助手终答
    pub fn append_assistant_message(
        &mut self,
        text: &str,
        answer_blocks: Vec<AnswerBlock>,
        reasoning: Option<&str>,
        citations: Vec<serde_json::Value>,
    ) {
        let sid = self.active.session_id.clone();
        self.active.messages.push(ConversationMessage {
            id: format!("msg-assistant-{}", self.active.messages.len() + 1),
            session_id: sid,
            message_id: None,
            role: MessageRole::Assistant,
            content: text.to_string(),
            answer_blocks,
            reasoning: reasoning.map(|s| s.to_string()),
            citations,
            created_at: "now".to_string(),
        });
    }
}

impl Default for ConversationManager {
    fn default() -> Self {
        Self::new()
    }
}
