//! Explicit mock RAG state for Product E2E — replaces scattered process-level OnceLock cells.

use std::sync::{Mutex, OnceLock};

#[derive(Debug, Default, Clone)]
pub struct MockRagState {
    pub codegen_chunk_id: Option<String>,
    pub codegen_chunk_ids: Vec<String>,
    pub codegen_doc_id: Option<String>,
    pub codegen_query: Option<String>,
    pub skip_codegen: bool,
    pub multiround_profile: bool,
    pub skill_request_memory: bool,
    pub emit_memory_tool: Option<String>,
}

fn mock_rag_state_cell() -> &'static Mutex<MockRagState> {
    static STATE: OnceLock<Mutex<MockRagState>> = OnceLock::new();
    STATE.get_or_init(|| Mutex::new(MockRagState::default()))
}

pub fn with_mock_rag_state<R>(f: impl FnOnce(&mut MockRagState) -> R) -> R {
    let mut guard = mock_rag_state_cell()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    f(&mut guard)
}

pub fn read_mock_rag_state<R>(f: impl FnOnce(&MockRagState) -> R) -> R {
    let guard = mock_rag_state_cell()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    f(&guard)
}

/// Reset per-test mock RAG state (call from TestContext setup).
pub fn reset_mock_rag_state() {
    with_mock_rag_state(|state| *state = MockRagState::default());
}

pub fn set_mock_rag_codegen_chunk_id(id: impl Into<String>) {
    let id = id.into();
    with_mock_rag_state(|state| {
        state.codegen_chunk_id = Some(id.clone());
        state.codegen_chunk_ids = vec![id];
    });
}

pub fn set_mock_rag_codegen_chunk_ids(ids: Vec<String>) {
    with_mock_rag_state(|state| {
        if let Some(first) = ids.first() {
            state.codegen_chunk_id = Some(first.clone());
        }
        state.codegen_chunk_ids = ids;
    });
}

pub fn set_mock_rag_skip_codegen(skip: bool) {
    with_mock_rag_state(|state| state.skip_codegen = skip);
}

pub fn set_mock_rag_codegen_query(query: impl Into<String>) {
    with_mock_rag_state(|state| state.codegen_query = Some(query.into()));
}

pub fn set_mock_rag_codegen_doc_id(id: impl Into<String>) {
    with_mock_rag_state(|state| state.codegen_doc_id = Some(id.into()));
}

pub fn set_mock_rag_multiround_profile(enabled: bool) {
    with_mock_rag_state(|state| state.multiround_profile = enabled);
}

pub fn set_mock_rag_skill_request_memory(enabled: bool) {
    with_mock_rag_state(|state| state.skill_request_memory = enabled);
}

pub fn set_mock_emit_memory_tool(tool: Option<impl Into<String>>) {
    with_mock_rag_state(|state| state.emit_memory_tool = tool.map(Into::into));
}
