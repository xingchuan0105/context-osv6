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
//!
//! # SaC composite bridge
//!
//! Python shim lists dense/lexical/grep/web/fetch/doc_*/history/user_profile.
//! `RuntimeBridge` owns retrieval; this module's [`SacHostBridge`] fills
//! web/memory ports and forwards the rest.

use std::collections::HashSet;
use std::sync::atomic::AtomicU64;
use std::sync::{Arc, Mutex};

use crate::runtime::AgentRequest;
use agent_tools::skills::builtin::calculator::CalculatorSkill;
use agent_tools::skills::builtin::user_context::UserContextSkill;
use agent_tools::skills::builtin::weather_query::WeatherQuerySkill;
use agent_tools::skills::builtin::web_fetch::WebFetchSkill;
use agent_tools::skills::memory_dispatch::{conversation_history_load, user_profile_load};
use agent_tools::skills::{ExecutionContext, SkillComponent};
use agent_tools::tool_registry::OwnedToolDeps;
use app_core::ChatPersistencePort;
use async_trait::async_trait;
use avrag_code_interpreter::{CodeInterpreter, ExecutionResult, HostBridge, InterpreterError};
use contracts::auth_runtime::AuthContext;
use contracts::sdk_primitives::{SdkCapability, ids_for};
use contracts::{ToolResult, ToolStatus};
use serde_json::{Value, json};

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

    pub fn with_rag_runtime(mut self, runtime: Option<Arc<avrag_rag_core::RagRuntime>>) -> Self {
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

    /// 沙箱是否需要 bridged 执行:有检索面,或本轮开放任意 base 原语
    /// (纯 chat 的 save/load/history/user_profile/user_context/calculator/weather_query
    /// 也经 host bridge;空 allowlist = 全开,测试专用)。
    pub fn sdk_can_bridge(&self, sdk_allowed: &HashSet<String>) -> bool {
        if self.rag_runtime.is_some() || self.search_executor.is_some() {
            return true;
        }
        sdk_allowed.is_empty()
            || ids_for(SdkCapability::BASE)
                .iter()
                .any(|id| sdk_allowed.contains(*id))
    }

    /// CodegenPort: run Python with SaC host bridge when configured.
    ///
    /// Returns execution result plus loop-local [`BridgeCallObs`] (rag-core types
    /// never leave this module). Callers must pass a non-empty `sdk_allowed` for
    /// capability gating (empty set = allow-all, tests only — see `method_allowed`).
    pub async fn execute_codegen_bridged_with_session(
        &self,
        code: &str,
        auth: &AuthContext,
        doc_scope: &[String],
        alias_counter: Arc<AtomicU64>,
        seen_chunk_aliases: Arc<std::sync::Mutex<std::collections::HashMap<String, String>>>,
        seen_chunk_bodies: Arc<std::sync::Mutex<std::collections::HashMap<String, String>>>,
        session_id: Option<uuid::Uuid>,
        session_fs: Arc<super::session_fs::SessionFs>,
        client_ip: Option<String>,
        client_local_time: Option<String>,
        client_timezone: Option<String>,
        sdk_allowed: Arc<HashSet<String>>,
    ) -> BridgedCodegenExec {
        // 纯 chat(base 原语开放)或带检索(search/rag)都走 bridged 路径;
        // 仅当 base 原语一个都不开放且无检索时,沙箱无任何可用 client.*,拒绝执行。
        if !self.sdk_can_bridge(&sdk_allowed) {
            return BridgedCodegenExec {
                exec: Err(InterpreterError::Bridge(
                    "no retrieval runtime and no base SDK primitives open for SaC codegen".into(),
                )),
                bridge_results: Vec::new(),
                bridge_calls: Vec::new(),
            };
        }

        let rag = self.rag_runtime.as_ref().map(|runtime| {
            avrag_rag_core::runtime::bridge::RuntimeBridge::new(
                Arc::clone(runtime),
                auth.clone(),
                doc_scope.to_vec(),
            )
            .with_alias_counter(Arc::clone(&alias_counter))
            .with_seen_chunk_aliases(Arc::clone(&seen_chunk_aliases))
            .with_seen_chunk_bodies(Arc::clone(&seen_chunk_bodies))
        });

        let bridge = Arc::new(SacHostBridge {
            rag,
            search: self.search_executor.clone(),
            chat_persistence: self.effective_chat_persistence(),
            auth: auth.clone(),
            session_id,
            session_fs,
            client_ip,
            client_local_time,
            client_timezone,
            sdk_allowed,
            extra_results: Mutex::new(Vec::new()),
            extra_calls: Mutex::new(Vec::new()),
        });
        let interpreter = CodeInterpreter::new();
        match interpreter
            .execute_with_bridge(code, Arc::clone(&bridge))
            .await
        {
            Ok(exec) => BridgedCodegenExec {
                bridge_results: bridge.take_all_results(),
                bridge_calls: bridge.take_all_calls(),
                exec: Ok(exec),
            },
            Err(e) => BridgedCodegenExec {
                bridge_results: bridge.take_all_results(),
                bridge_calls: bridge.take_all_calls(),
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

/// Product SaC host: retrieval via [`RuntimeBridge`] + base/web/memory ports.
struct SacHostBridge {
    rag: Option<avrag_rag_core::runtime::bridge::RuntimeBridge>,
    search: Option<Arc<dyn avrag_search::SearchProvider>>,
    chat_persistence: Option<Arc<dyn ChatPersistencePort>>,
    auth: AuthContext,
    session_id: Option<uuid::Uuid>,
    session_fs: Arc<super::session_fs::SessionFs>,
    client_ip: Option<String>,
    client_local_time: Option<String>,
    client_timezone: Option<String>,
    /// Empty set = open (tests). Product runs always pass a non-empty allowlist.
    sdk_allowed: Arc<HashSet<String>>,
    extra_results: Mutex<Vec<ToolResult>>,
    extra_calls: Mutex<Vec<BridgeCallObs>>,
}

impl SacHostBridge {
    fn take_all_results(&self) -> Vec<ToolResult> {
        let mut out = self
            .rag
            .as_ref()
            .map(|r| r.take_captured_results())
            .unwrap_or_default();
        out.extend(
            self.extra_results
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .drain(..),
        );
        out
    }

    fn take_all_calls(&self) -> Vec<BridgeCallObs> {
        let mut out = self
            .rag
            .as_ref()
            .map(|r| map_bridge_calls(r.take_captured_calls()))
            .unwrap_or_default();
        out.extend(
            self.extra_calls
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .drain(..),
        );
        out
    }

    fn record_extra(&self, method: &str, query: Option<String>, result: ToolResult) -> Value {
        let data = match result.status {
            ToolStatus::Ok => result.data.clone().unwrap_or(json!({})),
            ToolStatus::Error => {
                let message = result
                    .data
                    .as_ref()
                    .and_then(|d| d.get("error"))
                    .cloned()
                    .unwrap_or_else(|| json!("tool execution failed"));
                json!({ "error": { "code": "tool_error", "message": message } })
            }
            _ => result.data.clone().unwrap_or(json!({})),
        };
        self.extra_results
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .push(result.clone());
        self.extra_calls
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .push(BridgeCallObs {
                method: method.to_string(),
                query,
                result,
            });
        data
    }

    async fn call_web(&self, args: &Value) -> Value {
        let query = args
            .get("query")
            .and_then(|v| v.as_str())
            .unwrap_or_default();
        if query.is_empty() {
            return self.record_extra(
                "web",
                None,
                ToolResult {
                    tool: "web_search".into(),
                    version: "1.0".into(),
                    status: ToolStatus::Error,
                    data: Some(json!({ "error": "missing query" })),
                    trace: None,
                },
            );
        }
        let Some(provider) = &self.search else {
            return self.record_extra(
                "web",
                Some(query.to_string()),
                ToolResult {
                    tool: "web_search".into(),
                    version: "1.0".into(),
                    status: ToolStatus::Error,
                    data: Some(json!({ "error": "search provider not available" })),
                    trace: None,
                },
            );
        };
        let result = match provider.execute_search(query, None).await {
            Ok(response) => ToolResult {
                tool: "web_search".into(),
                version: "1.0".into(),
                status: ToolStatus::Ok,
                data: serde_json::to_value(&response).ok(),
                trace: None,
            },
            Err(e) => ToolResult {
                tool: "web_search".into(),
                version: "1.0".into(),
                status: ToolStatus::Error,
                data: Some(json!({ "error": e.to_string() })),
                trace: None,
            },
        };
        self.record_extra("web", Some(query.to_string()), result)
    }

    async fn call_fetch(&self, args: &Value) -> Value {
        let url = args.get("url").and_then(|v| v.as_str()).map(str::to_owned);
        let skill = WebFetchSkill;
        let ctx = ExecutionContext::new(self.search.as_deref().map(|p| p as _));
        let result = skill.execute(args, &ctx).await;
        self.record_extra("fetch", url, result)
    }

    async fn call_history(&self, args: &Value) -> Value {
        let Some(session_id) = self.session_id else {
            return self.record_extra(
                "history",
                None,
                ToolResult {
                    tool: "conversation_history_load".into(),
                    version: "1.0".into(),
                    status: ToolStatus::Error,
                    data: Some(json!({ "error": "session_id required for history" })),
                    trace: None,
                },
            );
        };
        let Some(persist) = &self.chat_persistence else {
            return self.record_extra(
                "history",
                None,
                ToolResult {
                    tool: "conversation_history_load".into(),
                    version: "1.0".into(),
                    status: ToolStatus::Error,
                    data: Some(json!({ "error": "chat persistence not available" })),
                    trace: None,
                },
            );
        };
        let result =
            conversation_history_load(args, &self.auth, session_id, persist.as_ref()).await;
        self.record_extra("history", None, result)
    }

    async fn call_user_profile(&self) -> Value {
        let Some(persist) = &self.chat_persistence else {
            return self.record_extra(
                "user_profile",
                None,
                ToolResult {
                    tool: "user_profile_load".into(),
                    version: "1.0".into(),
                    status: ToolStatus::Error,
                    data: Some(json!({ "error": "chat persistence not available" })),
                    trace: None,
                },
            );
        };
        let result = user_profile_load(&self.auth, persist.as_ref()).await;
        self.record_extra("user_profile", None, result)
    }

    async fn call_user_context(&self) -> Value {
        let skill = UserContextSkill;
        let ctx = ExecutionContext::new(self.search.as_deref().map(|p| p as _)).with_client_context(
            self.client_ip.clone(),
            self.client_local_time.clone(),
            self.client_timezone.clone(),
        );
        let result = skill.execute(&json!({}), &ctx).await;
        self.record_extra("user_context", None, result)
    }

    async fn call_calculator(&self, args: &Value) -> Value {
        let skill = CalculatorSkill;
        let ctx = ExecutionContext::new(self.search.as_deref().map(|p| p as _));
        let result = skill.execute(args, &ctx).await;
        self.record_extra("calculator", None, result)
    }

    async fn call_weather_query(&self, args: &Value) -> Value {
        // SDK 面签名是 city/lat/lon;host 技能面用 location("city" 或 "lat,lon")。
        let coord = |key: &str| {
            args.get(key)
                .and_then(|v| v.as_str().map(str::to_owned))
                .or_else(|| args.get(key).and_then(|v| v.as_f64()).map(|f| f.to_string()))
        };
        let location = match args.get("city").and_then(|v| v.as_str()) {
            Some(city) if !city.is_empty() => city.to_string(),
            _ => match (coord("lat"), coord("lon")) {
                (Some(lat), Some(lon)) => format!("{lat},{lon}"),
                _ => String::new(),
            },
        };
        let skill = WeatherQuerySkill;
        let ctx = ExecutionContext::new(self.search.as_deref().map(|p| p as _));
        let result = skill.execute(&json!({ "location": location }), &ctx).await;
        self.record_extra("weather_query", None, result)
    }

    fn call_save(&self, args: &Value) -> Value {
        let path = args.get("path").and_then(|v| v.as_str()).unwrap_or("");
        let data = args.get("data").cloned().unwrap_or(Value::Null);
        match self.session_fs.save(path, data) {
            Ok(()) => self.record_extra(
                "save",
                Some(path.to_string()),
                ToolResult {
                    tool: "session_fs_save".into(),
                    version: "1.0".into(),
                    status: ToolStatus::Ok,
                    data: Some(json!({ "ok": true, "path": path })),
                    trace: None,
                },
            ),
            Err(e) => self.record_extra(
                "save",
                Some(path.to_string()),
                ToolResult {
                    tool: "session_fs_save".into(),
                    version: "1.0".into(),
                    status: ToolStatus::Error,
                    data: Some(json!({ "error": e })),
                    trace: None,
                },
            ),
        }
    }

    fn call_load(&self, args: &Value) -> Value {
        let path = args.get("path").and_then(|v| v.as_str()).unwrap_or("");
        match self.session_fs.load(path) {
            Ok(data) => self.record_extra(
                "load",
                Some(path.to_string()),
                ToolResult {
                    tool: "session_fs_load".into(),
                    version: "1.0".into(),
                    status: ToolStatus::Ok,
                    data: Some(json!({ "path": path, "data": data })),
                    trace: None,
                },
            ),
            Err(e) => self.record_extra(
                "load",
                Some(path.to_string()),
                ToolResult {
                    tool: "session_fs_load".into(),
                    version: "1.0".into(),
                    status: ToolStatus::Error,
                    data: Some(json!({ "error": e })),
                    trace: None,
                },
            ),
        }
    }
}

#[async_trait]
impl HostBridge for SacHostBridge {
    async fn call(&self, method: &str, args: Value) -> Value {
        if !super::sdk_gate::method_allowed(&self.sdk_allowed, method) {
            return super::sdk_gate::capability_denied_error(method);
        }
        match method {
            "web" => self.call_web(&args).await,
            "fetch" => self.call_fetch(&args).await,
            "history" => self.call_history(&args).await,
            "user_profile" => self.call_user_profile().await,
            "user_context" => self.call_user_context().await,
            "calculator" => self.call_calculator(&args).await,
            "weather_query" => self.call_weather_query(&args).await,
            "save" => self.call_save(&args),
            "load" => self.call_load(&args),
            // Retrieval — RuntimeBridge when configured.
            _ => match &self.rag {
                Some(rag) => rag.call(method, args).await,
                None => json!({
                    "error": {
                        "code": "not_configured",
                        "message": format!(
                            "retrieval method `{method}` requires rag runtime (not configured)"
                        ),
                    }
                }),
            },
        }
    }
}
