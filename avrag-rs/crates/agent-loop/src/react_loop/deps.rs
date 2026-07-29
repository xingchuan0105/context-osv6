//! Runtime dependency bag for [`super::ReActLoop`] (Wave B1).
//!
//! # Type ownership
//!
//! Concrete `avrag_rag_core::RagRuntime` / `avrag_search::SearchProvider` handles
//! live **here** for the public builder surface and tool/codegen wiring.
//! Call sites should use [`LoopRuntimeDeps`] accessors rather than growing new
//! fields on `ReActLoop`.
//!
//! # Remaining coupling
//!
//! Codegen still constructs `RuntimeBridge` / `CapturedBridgeCall` in
//! `iteration_codegen` (rag-core types). Full port erasure is a follow-up
//! (`CodegenPort`); see plan Wave B1 acceptance note.

use std::sync::{Arc, Mutex};

use agent_tools::tool_registry::OwnedToolDeps;
use app_core::ChatPersistencePort;
use avrag_code_interpreter::CodeInterpreter;
use crate::runtime::AgentRequest;

/// Handles required to execute tools / codegen / auto-fallback inside the loop.
#[derive(Clone, Default)]
pub struct LoopRuntimeDeps {
    pub rag_runtime: Option<Arc<avrag_rag_core::RagRuntime>>,
    pub search_executor: Option<Arc<dyn avrag_search::SearchProvider>>,
    pub chat_persistence: Option<Arc<dyn ChatPersistencePort>>,
    pub code_interpreter: Arc<Mutex<Option<CodeInterpreter>>>,
}

impl LoopRuntimeDeps {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_rag_runtime(
        mut self,
        runtime: Option<Arc<avrag_rag_core::RagRuntime>>,
    ) -> Self {
        self.rag_runtime = runtime;
        self
    }

    pub fn with_search_executor(
        mut self,
        executor: Option<Arc<dyn avrag_search::SearchProvider>>,
    ) -> Self {
        self.search_executor = executor;
        self
    }

    pub fn with_chat_persistence(
        mut self,
        chat_persistence: Option<Arc<dyn ChatPersistencePort>>,
    ) -> Self {
        self.chat_persistence = chat_persistence;
        self
    }

    /// Prefer explicit inject, else fall back to rag-runtime embedded port.
    pub fn effective_chat_persistence(&self) -> Option<Arc<dyn ChatPersistencePort>> {
        self.chat_persistence.clone().or_else(|| {
            self.rag_runtime
                .as_ref()
                .and_then(|runtime| runtime.chat_persistence())
        })
    }

    /// Build Arc-backed deps for [`agent_tools::dispatch_tool`].
    pub fn owned_tool_deps(&self, request: &AgentRequest) -> OwnedToolDeps {
        let meta_str = |key: &str| {
            request
                .metadata
                .get(key)
                .and_then(|v| v.as_str())
                .map(str::to_string)
        };
        OwnedToolDeps {
            search_executor: self.search_executor.clone(),
            rag_runtime: self.rag_runtime.clone(),
            chat_persistence: self.effective_chat_persistence(),
            client_ip: meta_str("client_ip"),
            client_local_time: meta_str("client_local_time"),
            client_timezone: meta_str("client_timezone"),
        }
    }
}
