use contracts::{
    chat::AnswerBlock,
    workspaces::{ChatSession, ConversationScopeKind},
};

#[derive(Debug, Clone, PartialEq)]
pub struct ConversationMessage {
    pub id: String,
    pub role: MessageRole,
    pub content: String,
    pub answer_blocks: Vec<AnswerBlock>,
    pub reasoning: Option<String>,
    pub citations: Vec<serde_json::Value>,
    pub created_at: String,
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

    /// 恢复已有个人对话（Canonical /chat/:sessionId）
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

    /// 切换到工作区上下文（仅作为附加上下文，保留当前 model_role 设置，递增 epoch）
    pub fn switch_to_workspace(&mut self, workspace_id: &str, session_id: Option<&str>) {
        self.conversation_epoch += 1;
        self.active = ActiveConversation {
            session_id: session_id.map(|s| s.to_string()),
            workspace_id: Some(workspace_id.to_string()),
            model_role: self.active.model_role.clone(),
            scope_kind: ConversationScopeKind::Workspace,
            messages: Vec::new(),
        };
    }

    /// 记录用户提问
    pub fn append_user_message(&mut self, text: &str) {
        self.active.messages.push(ConversationMessage {
            id: format!("msg-user-{}", self.active.messages.len() + 1),
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
        self.active.messages.push(ConversationMessage {
            id: format!("msg-assistant-{}", self.active.messages.len() + 1),
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
