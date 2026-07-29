//! Runtime dependency bag for [`super::ReActLoop`] (Wave B1 + B1 follow-up).
//!
//! # Type ownership (allowlist for `avrag_rag_core` / `avrag_search`)
//!
//! Concrete runtime types and bridge construction live **only in this module**
//! inside `react_loop/`. Other loop files should consume [`BridgeCallObs`] and
//! [`LoopRuntimeDeps`] methods — not name `RuntimeBridge` / `RagRuntime` directly.
//!
//! Grep gate (production paths under `react_loop/`, excluding this file and
//! builder signatures on `ReActLoop` if any): no `avrag_rag_core::` /
//! `avrag_search::` outside `deps.rs`.

use std::sync::atomic::AtomicU64;
use std::sync::{Arc, Mutex};

use agent_tools::tool_registry::OwnedToolDeps;
use app_core::ChatPersistencePort;
use avrag_code_interpreter::{CodeInterpreter, ExecutionResult, InterpreterError};
use contracts::ToolResult;
use contracts::auth_runtime::AuthContext;
use crate::runtime::AgentRequest;

/// Loop-local view of one sandbox `client.*` call (no rag-core types).
#[derive(Debug, Clone)]
pub struct BridgeCallObs {
    pub method: String,
    pub query: Option<String>,
    pub result: ToolResult,
}

/// Outcome of one codegen block executed with optional retrieval bridge.
pub struct BridgedCodegenExec {
    pub exec: Result<ExecutionResult, InterpreterError>,
    pub bridge_results: Vec<ToolResult>,
    pub bridge_calls: Vec<BridgeCallObs>,
}

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

    pub fn has_rag_runtime(&self) -> bool {
        self.rag_runtime.is_some()
    }

    pub fn has_search_executor(&self) -> bool {
        self.search_executor.is_some()
    }

    /// CodegenPort: run Python with RagRuntime bridge when configured.
    ///
    /// Returns execution result plus loop-local [`BridgeCallObs`] (rag-core types
    /// never leave this module).
    pub async fn execute_codegen_bridged(
        &self,
        code: &str,
        auth: &AuthContext,
        doc_scope: &[String],
        alias_counter: Arc<AtomicU64>,
    ) -> BridgedCodegenExec {
        let Some(runtime) = &self.rag_runtime else {
            return BridgedCodegenExec {
                exec: Err(InterpreterError::Bridge(
                    "rag runtime not configured for bridged codegen".into(),
                )),
                bridge_results: Vec::new(),
                bridge_calls: Vec::new(),
            };
        };

        let bridge = Arc::new(
            avrag_rag_core::runtime::bridge::RuntimeBridge::new(
                Arc::clone(runtime),
                auth.clone(),
                doc_scope.to_vec(),
            )
            .with_alias_counter(alias_counter),
        );
        let interpreter = CodeInterpreter::new();
        match interpreter
            .execute_with_bridge(code, Arc::clone(&bridge))
            .await
        {
            Ok(exec) => BridgedCodegenExec {
                bridge_results: bridge.take_captured_results(),
                bridge_calls: map_bridge_calls(bridge.take_captured_calls()),
                exec: Ok(exec),
            },
            Err(e) => BridgedCodegenExec {
                // Preserve any calls that succeeded before the interpreter failed.
                bridge_results: bridge.take_captured_results(),
                bridge_calls: map_bridge_calls(bridge.take_captured_calls()),
                exec: Err(e),
            },
        }
    }

    /// Auto-fallback dense/lexical/graph via RagRuntime tool dispatch.
    pub async fn dispatch_rag_fallback(
        &self,
        auth: &AuthContext,
        tool_id: &str,
        args: serde_json::Value,
    ) -> Option<ToolResult> {
        let runtime = self.rag_runtime.as_ref()?;
        let call = contracts::ToolCall {
            tool: tool_id.to_string(),
            version: "1.0".to_string(),
            args,
        };
        Some(avrag_rag_core::runtime::tools::dispatch(runtime, auth, &call).await)
    }

    /// Web search auto-fallback via SearchProvider.
    pub async fn execute_search_fallback(
        &self,
        query: &str,
        vertical: Option<&str>,
    ) -> Option<Result<avrag_search::SearchResponse, anyhow::Error>> {
        let executor = self.search_executor.as_ref()?;
        Some(executor.execute_search(query, vertical).await)
    }
}

fn map_bridge_calls(
    calls: Vec<avrag_rag_core::runtime::bridge::CapturedBridgeCall>,
) -> Vec<BridgeCallObs> {
    calls
        .into_iter()
        .map(|c| BridgeCallObs {
            method: c.method,
            query: c.query,
            result: c.result,
        })
        .collect()
}
