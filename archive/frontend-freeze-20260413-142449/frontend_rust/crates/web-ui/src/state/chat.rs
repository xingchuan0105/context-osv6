//! Chat state - reactive state management for chat using Leptos signals

use leptos::prelude::*;
use web_sdk::dtos::{AnswerBlock, Citation};

use crate::platform::next_client_id;

/// Chat status enum for the state machine
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChatStatus {
    Idle,
    Submitting,
    Streaming,
    Done,
    Error,
}

/// Chat message for display
#[derive(Debug, Clone)]
pub struct ChatMessage {
    pub id: String,
    pub role: ChatRole,
    pub content: String,
    pub answer_blocks: Vec<AnswerBlock>,
    pub citations: Vec<Citation>,
    pub session_id: Option<String>,
    pub server_message_id: Option<i64>,
}

/// Role in a chat conversation
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChatRole {
    User,
    Assistant,
}

impl ChatRole {
    pub fn as_str(&self) -> &'static str {
        match self {
            ChatRole::User => "user",
            ChatRole::Assistant => "assistant",
        }
    }

    pub fn from_api_role(role: &str) -> Self {
        match role {
            "assistant" => ChatRole::Assistant,
            _ => ChatRole::User,
        }
    }
}

/// Right sidebar tab types
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RightTab {
    Evidence,
    Trace,
    Session,
}

/// Chat state for managing chat UI state
#[derive(Clone)]
pub struct ChatState {
    // Core state
    pub status: ReadSignal<ChatStatus>,
    pub set_status: WriteSignal<ChatStatus>,
    pub messages: ReadSignal<Vec<ChatMessage>>,
    pub set_messages: WriteSignal<Vec<ChatMessage>>,
    pub citations: ReadSignal<Vec<Citation>>,
    pub set_citations: WriteSignal<Vec<Citation>>,
    pub current_answer: ReadSignal<String>,
    pub set_current_answer: WriteSignal<String>,
    pub error_message: ReadSignal<Option<String>>,
    pub set_error_message: WriteSignal<Option<String>>,
    pub session_id: ReadSignal<Option<String>>,
    pub set_session_id: WriteSignal<Option<String>>,
    pub trace_events: ReadSignal<Vec<String>>,
    pub set_trace_events: WriteSignal<Vec<String>>,
    pub rag_trace_json: ReadSignal<Option<String>>,
    pub set_rag_trace_json: WriteSignal<Option<String>>,
    pub planner_mode: ReadSignal<Option<String>>,
    pub set_planner_mode: WriteSignal<Option<String>>,
    pub source_count: ReadSignal<usize>,
    pub set_source_count: WriteSignal<usize>,
    pub degrade_reasons: ReadSignal<Vec<String>>,
    pub set_degrade_reasons: WriteSignal<Vec<String>>,

    // UI state
    pub active_tab: ReadSignal<RightTab>,
    pub set_active_tab: WriteSignal<RightTab>,
    pub active_citation: ReadSignal<Option<Citation>>,
    pub set_active_citation: WriteSignal<Option<Citation>>,

    // Agent mode
    pub agent_mode: ReadSignal<AgentMode>,
    pub set_agent_mode: WriteSignal<AgentMode>,
}

/// Agent mode for chat
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AgentMode {
    Rag,
    Search,
    General,
}

impl AgentMode {
    /// Returns the agent_type string used in API calls
    pub fn as_str(&self) -> &'static str {
        match self {
            AgentMode::Rag => "rag",
            AgentMode::Search => "search",
            AgentMode::General => "general",
        }
    }

    /// Returns display label
    pub fn label(&self) -> &'static str {
        match self {
            AgentMode::Rag => "RAG",
            AgentMode::Search => "Search",
            AgentMode::General => "General",
        }
    }
}

/// Provides the chat state as a Leptos context.
pub fn provide_chat_state() -> ChatState {
    let (status, set_status) = signal(ChatStatus::Idle);
    let (messages, set_messages) = signal(Vec::<ChatMessage>::new());
    let (citations, set_citations) = signal(Vec::<Citation>::new());
    let (current_answer, set_current_answer) = signal(String::new());
    let (error_message, set_error_message) = signal(None);
    let (session_id, set_session_id) = signal(None);
    let (trace_events, set_trace_events) = signal(Vec::<String>::new());
    let (rag_trace_json, set_rag_trace_json) = signal(None);
    let (planner_mode, set_planner_mode) = signal(None);
    let (source_count, set_source_count) = signal(0usize);
    let (degrade_reasons, set_degrade_reasons) = signal(Vec::<String>::new());
    let (active_tab, set_active_tab) = signal(RightTab::Evidence);
    let (active_citation, set_active_citation) = signal(None);
    let (agent_mode, set_agent_mode) = signal(AgentMode::Rag);

    let state = ChatState {
        status,
        set_status,
        messages,
        set_messages,
        citations,
        set_citations,
        current_answer,
        set_current_answer,
        error_message,
        set_error_message,
        session_id,
        set_session_id,
        trace_events,
        set_trace_events,
        rag_trace_json,
        set_rag_trace_json,
        planner_mode,
        set_planner_mode,
        source_count,
        set_source_count,
        degrade_reasons,
        set_degrade_reasons,
        active_tab,
        set_active_tab,
        active_citation,
        set_active_citation,
        agent_mode,
        set_agent_mode,
    };

    provide_context(state.clone());
    state
}

/// Retrieves the chat state from context.
/// Panics if called outside of a component that has called `provide_chat_state`.
pub fn use_chat_state() -> ChatState {
    use_context().expect("ChatState not provided - did you call provide_chat_state()?")
}

impl ChatState {
    /// Append a user message to the message list
    pub fn add_user_message(&self, content: String) {
        let msg = ChatMessage {
            id: uuid_v4(),
            role: ChatRole::User,
            content,
            answer_blocks: Vec::new(),
            citations: Vec::new(),
            session_id: self.session_id.get(),
            server_message_id: None,
        };
        self.set_messages.update(|msgs| msgs.push(msg));
    }

    /// Add or update the assistant's streaming response
    pub fn update_streaming_message(&self, content: String, citations: Vec<Citation>) {
        self.set_messages.update(|msgs| {
            // Find the last assistant message or create one
            if let Some(last) = msgs.last_mut() {
                if last.role == ChatRole::Assistant {
                    last.content = content;
                    last.answer_blocks = Vec::new();
                    last.citations = citations;
                    last.session_id = self.session_id.get();
                    return;
                }
            }
            // Create new assistant message
            msgs.push(ChatMessage {
                id: uuid_v4(),
                role: ChatRole::Assistant,
                content,
                answer_blocks: Vec::new(),
                citations,
                session_id: self.session_id.get(),
                server_message_id: None,
            });
        });
    }

    /// Finalize the streaming response
    pub fn finalize_response(
        &self,
        session_id: Option<String>,
        server_message_id: Option<i64>,
        content: Option<String>,
        answer_blocks: Option<Vec<AnswerBlock>>,
        citations: Option<Vec<Citation>>,
    ) {
        self.set_status.set(ChatStatus::Done);
        self.set_messages.update(|msgs| {
            if let Some(last) = msgs.last_mut()
                && last.role == ChatRole::Assistant
            {
                if let Some(session_id) = session_id.clone() {
                    last.session_id = Some(session_id);
                }
                last.server_message_id = server_message_id;
                if let Some(content) = content.clone() {
                    last.content = content;
                }
                if let Some(answer_blocks) = answer_blocks.clone() {
                    last.answer_blocks = answer_blocks;
                }
                if let Some(citations) = citations.clone() {
                    last.citations = citations;
                }
            }
        });
    }

    /// Reset to idle state for a new conversation
    pub fn reset(&self) {
        self.set_status.set(ChatStatus::Idle);
        self.set_messages.set(Vec::new());
        self.set_citations.set(Vec::new());
        self.set_current_answer.set(String::new());
        self.set_error_message.set(None);
        self.set_session_id.set(None);
        self.set_trace_events.set(Vec::new());
        self.set_rag_trace_json.set(None);
        self.set_planner_mode.set(None);
        self.set_source_count.set(0);
        self.set_degrade_reasons.set(Vec::new());
    }

    /// Set error state
    pub fn set_error(&self, message: String) {
        self.set_error_message.set(Some(message));
        self.set_status.set(ChatStatus::Error);
    }

    /// Start submitting a query
    pub fn start_submit(&self, query: String) {
        self.add_user_message(query);
        self.set_status.set(ChatStatus::Submitting);
        self.set_current_answer.set(String::new());
        self.set_citations.set(Vec::new());
        self.set_error_message.set(None);
        self.set_trace_events.set(Vec::new());
        self.set_rag_trace_json.set(None);
        self.set_planner_mode.set(None);
        self.set_source_count.set(0);
        self.set_degrade_reasons.set(Vec::new());
    }

    /// Append a token to the current answer
    pub fn append_token(&self, token: String) {
        self.set_current_answer.update(|a| a.push_str(&token));
        self.set_status.set(ChatStatus::Streaming);
    }

    /// Set citations from the response
    pub fn set_citations(&self, citations: Vec<Citation>) {
        self.set_citations.set(citations.clone());
        // Also update the last assistant message's citations
        self.set_messages.update(|msgs| {
            if let Some(last) = msgs.last_mut() {
                if last.role == ChatRole::Assistant {
                    last.citations = citations;
                }
            }
        });
    }

    pub fn set_session(&self, session_id: String) {
        self.set_session_id.set(Some(session_id));
    }

    pub fn push_trace(&self, entry: String) {
        self.set_trace_events.update(|events| events.push(entry));
    }

    pub fn set_planner_mode_value(&self, mode: String) {
        self.set_planner_mode.set(Some(mode.clone()));
        self.push_trace(format!("planner_complete: {mode}"));
    }

    pub fn set_rag_trace(&self, trace_json: serde_json::Value) {
        self.set_rag_trace_json.set(Some(
            serde_json::to_string_pretty(&trace_json).unwrap_or_default(),
        ));
    }

    pub fn set_source_refs(&self, count: usize) {
        self.set_source_count.set(count);
    }

    pub fn set_degrade_trace(&self, reasons: Vec<String>) {
        self.set_degrade_reasons.set(reasons);
    }

    pub fn load_session_messages(
        &self,
        session_id: String,
        messages: Vec<(i64, String, String, Vec<AnswerBlock>, Vec<Citation>)>,
    ) {
        let converted_messages = messages
            .into_iter()
            .map(
                |(server_message_id, role, content, answer_blocks, citations)| ChatMessage {
                    id: uuid_v4(),
                    role: ChatRole::from_api_role(&role),
                    content,
                    answer_blocks,
                    citations,
                    session_id: Some(session_id.clone()),
                    server_message_id: Some(server_message_id),
                },
            )
            .collect::<Vec<_>>();
        let latest_citations = converted_messages
            .iter()
            .rev()
            .find(|message| message.role == ChatRole::Assistant)
            .map(|message| message.citations.clone())
            .unwrap_or_default();

        self.set_session_id.set(Some(session_id));
        self.set_messages.set(converted_messages);
        self.set_citations.set(latest_citations);
        self.set_current_answer.set(String::new());
        self.set_error_message.set(None);
        self.set_trace_events.set(Vec::new());
        self.set_rag_trace_json.set(None);
        self.set_planner_mode.set(None);
        self.set_source_count.set(0);
        self.set_degrade_reasons.set(Vec::new());
        self.set_active_citation.set(None);
        self.set_status.set(ChatStatus::Done);
    }
}

fn uuid_v4() -> String {
    next_client_id("msg")
}
