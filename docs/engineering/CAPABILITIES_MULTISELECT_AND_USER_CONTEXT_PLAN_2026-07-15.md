# Capability Multiselect + Write Offline + `user_context` Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace mutually exclusive chat modes with multi-select RAG/Search capabilities, product-reject Write, compose prompts as base + manuals, and add base tool `user_context` (frontend clock + MaxMind city).

**Architecture:** Keep the single Agent lane (UnifiedAgent / ReAct / ToolCatalog). Resolve `capabilities[]` (with legacy `agent_type` map) into a `CapabilitySet`, assemble `ModeConfig` + system prompt at runtime, always include `user_context` in the base tool surface. Write is rejected at `ConversationApp` (no product write lane).

**Tech Stack:** Rust (`contracts`, `agent-loop`, `agent-tools`, `app-chat`, `app-bootstrap`, `transport-http`), Next.js frontend (`frontend_next`), MaxMind GeoLite2 via `maxminddb` crate, existing OpenWeather `weather_query`.

**Design spec:** [`docs/engineering/CAPABILITIES_MULTISELECT_AND_USER_CONTEXT_DESIGN_2026-07-15.md`](./CAPABILITIES_MULTISELECT_AND_USER_CONTEXT_DESIGN_2026-07-15.md)

**Solo verify default:** targeted `cargo test -p …` / `pnpm test`; no CI theater; no deep write-core deletion this wave.

---

## File map (create / modify)

### Contracts / wire

| Path | Role |
|------|------|
| `contracts/src/chat.rs` | Add `capabilities`, `ClientContext`; keep `agent_type` deprecated-compatible |
| `contracts/tests/chat_json.rs` | Serde fixtures for new fields |
| `frontend_next/lib/contracts/generated/*` | Regenerate TS from typeshare/export (project existing flow) |

### Capability resolve + mode assemble

| Path | Role |
|------|------|
| `avrag-rs/crates/app-chat/src/capabilities.rs` **(create)** | `CapabilitySet`, resolve from request, derive label, reject write |
| `avrag-rs/crates/app-chat/src/mode_assemble.rs` **(create)** | Merge mode YAML fragments → runtime `ModeConfig` + prompt paths |
| `avrag-rs/crates/app-chat/src/lib.rs` | `mod` + re-exports |
| `avrag-rs/crates/agent-loop/src/react_loop/policy/config/config_types.rs` | Optional multi prompt parts / new `AnswerContractKind` |
| `avrag-rs/crates/agent-loop/src/react_loop/assembler.rs` | Compose multi-part system prompt |
| `avrag-rs/crates/agent-loop/src/react_loop/answer_contract.rs` | Hybrid synthesis contract text |
| `avrag-rs/modes/chat.yaml`, `rag.yaml`, `search.yaml` | Keep as **capability fragments** (still loadable by id) |
| `avrag-rs/prompts/orchestrators/agent-base.md` **(create)** | Shared identity + task |
| `avrag-rs/prompts/orchestrators/capability-rag.md` **(create)** | RAG manual |
| `avrag-rs/prompts/orchestrators/capability-search.md` **(create)** | Search manual |
| Existing `chat-system.md` / `rag-system.md` / `search-system.md` | Source for split; leave as thin wrappers or deprecate after cutover |

### Product boundary / pipeline

| Path | Role |
|------|------|
| `avrag-rs/crates/app-bootstrap/src/product_apps/conversation.rs` | Reject write; pass resolved caps |
| `avrag-rs/crates/app-chat/src/chat/pipeline.rs` | Agent lane only for product execute (write path unreachable from product) |
| `avrag-rs/crates/app-chat/src/chat/pipeline_steps.rs` | Load assembled mode from `CapabilitySet` not bare enum only |
| `avrag-rs/crates/app-chat/src/agents/unified/*` | Use assembled `ModeConfig` |
| `avrag-rs/crates/app-chat/src/chat/service*.rs` | Persist capabilities in turn metadata; derived `agent_type` on response |
| `avrag-rs/crates/transport-http/src/handlers/chat.rs` | Inject client IP into request metadata / context for tools |

### `user_context` tool

| Path | Role |
|------|------|
| `avrag-rs/crates/agent-tools/src/geoip.rs` **(create)** | MaxMind lookup wrapper |
| `avrag-rs/crates/agent-tools/src/skills/builtin/user_context.rs` **(create)** | Skill |
| `avrag-rs/crates/agent-tools/src/skills/builtin/mod.rs` | Register skill |
| `avrag-rs/crates/agent-tools/src/skills/registry.rs` | Extend `ExecutionContext` with client clock + IP |
| `avrag-rs/crates/agent-tools/Cargo.toml` | Add `maxminddb` (+ `ipnetwork` if needed) |
| `avrag-rs/.env.example` | `GEOIP_CITY_DB_PATH` |
| Base tool pools in mode assemble | Always include `user_context` |

### Frontend

| Path | Role |
|------|------|
| `frontend_next/lib/workspace/ui-store.ts` | `capabilities: ("rag"\|"search")[]` session state; drop write from selectable modes |
| `frontend_next/components/workspace/chat-composer.tsx` | Tag toggles instead of exclusive menu |
| `frontend_next/hooks/chat-session/use-chat-stream.ts` | Send `capabilities` + `client_context` |
| `frontend_next/hooks/chat-session/helpers.ts` | Normalize capabilities for messages |
| `frontend_next/components/workspace/chat-message-list.tsx` | 0–2 capability chips |
| `frontend_next/lib/i18n/messages/workspace.ts` | Capability labels; retire write as selectable |
| `frontend_next/e2e/pom/chat-panel-page.ts` | `setCapabilities` |
| `frontend_next/tests/workspace/workspace-chat-pane.modes.test.tsx` | Update expectations |

---

## Derived response label (telemetry / old UI)

| CapabilitySet | `agent_type` on response (derived) |
|---------------|-------------------------------------|
| `{}` | `chat` |
| `{rag}` | `rag` |
| `{search}` | `search` |
| `{rag,search}` | `rag+search` |

Do **not** accept `rag+search` as a legacy **input** `agent_type` unless easy; primary input is `capabilities`.

---

### Task 1: Contract — `capabilities` + `client_context`

**Files:**
- Modify: `contracts/src/chat.rs`
- Modify: `contracts/tests/chat_json.rs`
- Test: `cargo test -p contracts --test chat_json`

- [ ] **Step 1: Write failing / extend contract types**

In `contracts/src/chat.rs`, add types and fields on `ChatRequest`:

```rust
#[typeshare]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct ClientContext {
    /// ISO-8601 local datetime with offset when possible, e.g. `2026-07-15T14:32:00+08:00`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub local_time: Option<String>,
    /// IANA timezone, e.g. `Asia/Shanghai`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub timezone: Option<String>,
}

// Inside ChatRequest, after agent_type:
    /// Product capability tags. When **present** (including empty `[]`), wins over `agent_type` for tool exposure.
    /// Allowed values: `rag`, `search`. Unknown values ignored at resolve time.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub capabilities: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub client_context: Option<ClientContext>,
```

Keep `agent_type` with existing default (`default_chat_agent`) for compatibility.

- [ ] **Step 2: Add serde tests**

In `contracts/tests/chat_json.rs`:

```rust
#[test]
fn chat_request_deserializes_capabilities_and_client_context() {
    let raw = r#"{
      "query": "hi",
      "capabilities": ["rag", "search", "rag"],
      "client_context": {
        "local_time": "2026-07-15T14:32:00+08:00",
        "timezone": "Asia/Shanghai"
      }
    }"#;
    let req: ChatRequest = serde_json::from_str(raw).unwrap();
    assert_eq!(req.capabilities.as_ref().unwrap(), &vec!["rag".into(), "search".into(), "rag".into()]);
    assert_eq!(req.client_context.as_ref().unwrap().timezone.as_deref(), Some("Asia/Shanghai"));
}

#[test]
fn chat_request_omitted_capabilities_is_none() {
    let raw = r#"{"query":"hi","agent_type":"rag"}"#;
    let req: ChatRequest = serde_json::from_str(raw).unwrap();
    assert!(req.capabilities.is_none());
    assert_eq!(req.agent_type, "rag");
}
```

- [ ] **Step 3: Run tests**

```bash
cd /home/chuan/context-osv6/contracts && cargo test --test chat_json
```

Expected: PASS (after types compile).

- [ ] **Step 4: Commit**

```bash
git add contracts/src/chat.rs contracts/tests/chat_json.rs
git commit -m "feat(contracts): add capabilities and client_context on ChatRequest"
```

---

### Task 2: `CapabilitySet` resolve + write reject unit tests

**Files:**
- Create: `avrag-rs/crates/app-chat/src/capabilities.rs`
- Modify: `avrag-rs/crates/app-chat/src/lib.rs`
- Test: unit tests in `capabilities.rs`

- [ ] **Step 1: Write failing tests first** (module with `#[cfg(test)]`)

```rust
// capabilities.rs — core API

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct CapabilitySet {
    pub rag: bool,
    pub search: bool,
}

impl CapabilitySet {
    pub fn is_pure_chat(&self) -> bool {
        !self.rag && !self.search
    }

    /// Derived wire/telemetry label.
    pub fn agent_type_label(&self) -> &'static str {
        match (self.rag, self.search) {
            (false, false) => "chat",
            (true, false) => "rag",
            (false, true) => "search",
            (true, true) => "rag+search",
        }
    }

    pub fn as_string_list(&self) -> Vec<String> {
        let mut v = Vec::new();
        if self.rag {
            v.push("rag".into());
        }
        if self.search {
            v.push("search".into());
        }
        v
    }
}

/// Error when product write is requested.
pub fn write_disabled_error() -> common::AppError {
    common::AppError::validation(
        "write_mode_disabled",
        "Writing mode is no longer available. Use chat with optional RAG/Search capabilities.",
    )
}

/// Resolve capabilities from request fields.
/// - If `capabilities` is `Some`, normalize (rag/search only, dedupe) and **ignore** agent_type for caps.
/// - If `capabilities` is `None`, map legacy agent_type.
/// - `write` / `write_refine` → Err.
pub fn resolve_capabilities(
    capabilities: Option<&[String]>,
    agent_type: &str,
) -> Result<CapabilitySet, common::AppError> {
    let at = agent_type.trim();
    if at.eq_ignore_ascii_case("write") || at.eq_ignore_ascii_case("write_refine") {
        return Err(write_disabled_error());
    }

    if let Some(list) = capabilities {
        let mut set = CapabilitySet::default();
        for raw in list {
            match raw.trim().to_ascii_lowercase().as_str() {
                "rag" => set.rag = true,
                "search" => set.search = true,
                _ => {} // ignore unknown
            }
        }
        return Ok(set);
    }

    match at.to_ascii_lowercase().as_str() {
        "rag" => Ok(CapabilitySet { rag: true, search: false }),
        "search" => Ok(CapabilitySet { rag: false, search: true }),
        "chat" | "general" | "" => Ok(CapabilitySet::default()),
        "write" | "write_refine" => Err(write_disabled_error()),
        _ => Ok(CapabilitySet::default()), // unknown legacy → pure chat
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_capabilities_is_pure_chat() {
        let s = resolve_capabilities(Some(&[]), "rag").unwrap();
        assert!(s.is_pure_chat());
        assert_eq!(s.agent_type_label(), "chat");
    }

    #[test]
    fn capabilities_win_over_agent_type() {
        let list = vec!["search".into()];
        let s = resolve_capabilities(Some(&list), "rag").unwrap();
        assert!(!s.rag && s.search);
    }

    #[test]
    fn legacy_rag_maps() {
        let s = resolve_capabilities(None, "rag").unwrap();
        assert!(s.rag && !s.search);
    }

    #[test]
    fn write_rejected() {
        let err = resolve_capabilities(None, "write").unwrap_err();
        assert!(err.to_string().contains("write") || format!("{err:?}").contains("write_mode_disabled"));
    }

    #[test]
    fn dual_label() {
        let list = vec!["rag".into(), "search".into(), "nope".into()];
        let s = resolve_capabilities(Some(&list), "chat").unwrap();
        assert_eq!(s.agent_type_label(), "rag+search");
        assert_eq!(s.as_string_list(), vec!["rag", "search"]);
    }
}
```

Wire `mod capabilities;` + `pub use capabilities::{resolve_capabilities, CapabilitySet, write_disabled_error};` in `lib.rs`.

- [ ] **Step 2: Run**

```bash
cd /home/chuan/context-osv6/avrag-rs && cargo test -p app-chat --lib capabilities::
```

Expected: PASS.

- [ ] **Step 3: Commit**

```bash
git add avrag-rs/crates/app-chat/src/capabilities.rs avrag-rs/crates/app-chat/src/lib.rs
git commit -m "feat(app-chat): resolve CapabilitySet from capabilities and legacy agent_type"
```

---

### Task 3: Product boundary — reject write in `ConversationApp`

**Files:**
- Modify: `avrag-rs/crates/app-bootstrap/src/product_apps/conversation.rs`
- Modify: `avrag-rs/crates/app-bootstrap/src/product_apps/mod.rs` (tests that expect write route)
- Related: any test that calls execute with `agent_type=write` and expects success → expect validation error

- [ ] **Step 1: Change routing**

```rust
// conversation.rs execute / execute_stream
Self::validate_user_agent_type(&req.agent_type)?;
// Reject product write (capabilities path also checked once ChatRequest is available)
if app_chat::is_write_agent_type(&req.agent_type) {
    return Err(app_chat::write_disabled_error());
}
// Optional early resolve to fail write_refine already handled
app_chat::resolve_capabilities(req.capabilities.as_deref(), &req.agent_type)?;
self.chat.execute_chat(req).await  // stream analog: execute_chat_stream only
```

Update `validate_user_agent_type` message for write_refine: no longer tell users to use `agent_type=write`; say internal-only / disabled.

- [ ] **Step 2: Fix unit tests in `product_apps/mod.rs`**

Replace “pipeline accepts write lane” expectations with:

```rust
#[tokio::test]
async fn conversation_rejects_write_agent_type() {
    // build ConversationApp with test ChatContext as existing tests do
    let err = app.execute(empty_chat_req("write")).await.unwrap_err();
    // assert validation / write_mode_disabled
}
```

Remove or invert `pipeline_defends_agent_lane_against_write` only if it still applies to lower pipeline; product entry must reject.

- [ ] **Step 3: Run**

```bash
cd /home/chuan/context-osv6/avrag-rs && cargo test -p app-bootstrap --lib product_apps
```

Expected: PASS.

- [ ] **Step 4: Commit**

```bash
git add avrag-rs/crates/app-bootstrap/src/product_apps/
git commit -m "feat(conversation): reject write mode at product boundary"
```

---

### Task 4: Prompt assets — base + two manuals

**Files:**
- Create: `avrag-rs/prompts/orchestrators/agent-base.md`
- Create: `avrag-rs/prompts/orchestrators/capability-rag.md`
- Create: `avrag-rs/prompts/orchestrators/capability-search.md`

- [ ] **Step 1: Split content from existing orchestrators**

**`agent-base.md`** (from common parts of chat/rag/search):
- Role: Context OS assistant (not mode-specific name lock-in)
- Shared rules: same language as user; no fabricated sources; ReAct lifecycle overview
- Base tools mention: may call `user_context` for local time / city-level geo; **never invent city** if tool says confidence none
- Pure-chat behavior when no capability manuals attached: conversational, no codegen, no web_search schema

**`capability-rag.md`**:
- Workspace document evidence only; `[[cite:…]]`; codegen / skill clusters from current rag-system
- Do not claim web facts without Search capability

**`capability-search.md`**:
- web_search / web_fetch; `[[n]]` citations; no codegen path
- Do not claim workspace doc facts without RAG capability

Keep YAML front-matter style consistent with existing prompts (name/version/category).

- [ ] **Step 2: Smoke load (manual or tiny test later in assemble)**

```bash
test -f avrag-rs/prompts/orchestrators/agent-base.md
test -f avrag-rs/prompts/orchestrators/capability-rag.md
test -f avrag-rs/prompts/orchestrators/capability-search.md
```

- [ ] **Step 3: Commit**

```bash
git add avrag-rs/prompts/orchestrators/agent-base.md \
  avrag-rs/prompts/orchestrators/capability-rag.md \
  avrag-rs/prompts/orchestrators/capability-search.md
git commit -m "docs(prompts): add agent-base and capability manuals for composition"
```

---

### Task 5: Mode assembly (union tools, max budget, hybrid contract)

**Files:**
- Create: `avrag-rs/crates/app-chat/src/mode_assemble.rs`
- Modify: `avrag-rs/crates/agent-loop/src/react_loop/policy/config/config_types.rs` — add `AnswerContractKind::InternalHybridAnswerV1` (serde `internal_hybrid_answer_v1`)
- Modify: `avrag-rs/crates/agent-loop/src/react_loop/answer_contract.rs` — contract prose for hybrid
- Modify: assembler to support multi prompt parts **or** mode_assemble writes a single concatenated system string into a runtime-only field

**Recommended assembly API:**

```rust
// mode_assemble.rs
use agent_loop::{load_mode_config, ModeConfig, AnswerContractKind};
use crate::capabilities::CapabilitySet;

pub struct AssembledMode {
    pub config: ModeConfig,
    /// Ordered prompt file paths relative to modes root (same as today).
    pub system_prompt_parts: Vec<String>,
}

pub fn assemble_mode(caps: CapabilitySet) -> Result<AssembledMode, common::AppError> {
    let chat = load_mode_config("chat")?; // fragment for pure-chat loop_exit defaults
    let mut parts = vec!["prompts/orchestrators/agent-base.md".into()];
    let mut tool_pool = vec!["user_context".to_string()];
    // start from chat skill_catalog / loop_exit
    let mut config = chat;
    config.id = caps.agent_type_label().into();

    if caps.rag {
        let rag = load_mode_config("rag")?;
        parts.push("prompts/orchestrators/capability-rag.md".into());
        merge_tools(&mut tool_pool, &rag.tool_pool);
        merge_skill_catalog(&mut config.skill_catalog, &rag.skill_catalog);
        config.budget.max_iterations = config.budget.max_iterations.max(rag.budget.max_iterations);
        // merge by_user_tier with max per key
        config.inject_retrieval_query = true;
        config.loop_exit.require_evidence = true;
        config.loop_exit.allow_content_early_stop = false;
        config.synthesis_output.contract = AnswerContractKind::InternalAnswerV1;
        if let Some(fb) = rag.auto_fallback { config.auto_fallback = Some(fb); }
        config.temperature = rag.temperature.or(config.temperature);
    }
    if caps.search {
        let search = load_mode_config("search")?;
        parts.push("prompts/orchestrators/capability-search.md".into());
        merge_tools(&mut tool_pool, &search.tool_pool);
        merge_skill_catalog(&mut config.skill_catalog, &search.skill_catalog);
        config.budget.max_iterations = config.budget.max_iterations.max(search.budget.max_iterations);
        config.inject_retrieval_query = true;
        config.loop_exit.require_evidence = true;
        config.loop_exit.allow_content_early_stop = false;
        if caps.rag {
            config.synthesis_output.contract = AnswerContractKind::InternalHybridAnswerV1;
        } else {
            config.synthesis_output.contract = AnswerContractKind::InternalSearchAnswerV1;
        }
        // if only search, prefer search auto_fallback
        if !caps.rag {
            config.auto_fallback = search.auto_fallback;
            config.temperature = search.temperature.or(config.temperature);
        }
    }
    if caps.is_pure_chat() {
        config.loop_exit.require_evidence = false;
        config.loop_exit.allow_content_early_stop = true;
        config.loop_exit.skip_synthesis_on_direct_answer = true;
        config.synthesis_output.contract = AnswerContractKind::ProseOnly;
        // pure chat: only user_context in tool_pool (plus memory via skill clusters)
        tool_pool = vec!["user_context".into()];
    }
    // ensure user_context always present
    if !tool_pool.iter().any(|t| t == "user_context") {
        tool_pool.insert(0, "user_context".into());
    }
    config.tool_pool = dedupe(tool_pool);
    // Point system_prompt_base at first part for back-compat; assembler uses parts list
    config.system_prompt_base = parts[0].clone();
    Ok(AssembledMode { config, system_prompt_parts: parts })
}
```

Implement `merge_tools`, `merge_skill_catalog`, `dedupe` as private helpers. Skill catalog merge = union of retrieve/synthesis/mandatory lists (dedupe preserve order).

- [ ] **Step 1: Extend `ModeConfig` OR pass parts via request metadata**

Minimal invasive option: store prompt parts on `AgentRequest.metadata` as `system_prompt_parts: [...]` and change `ContextAssembler` to:

```rust
fn load_composed_system(mode: &ModeConfig, request: &AgentRequest) -> String {
    if let Some(parts) = request.metadata.get("system_prompt_parts").and_then(|v| v.as_array()) {
        parts.iter()
            .filter_map(|p| p.as_str())
            .filter_map(|path| load_system_prompt(path).ok())
            .collect::<Vec<_>>()
            .join("\n\n---\n\n")
    } else {
        load_system_prompt(&mode.system_prompt_base).unwrap_or_default()
    }
}
```

- [ ] **Step 2: Unit tests for assemble**

```rust
#[test]
fn pure_chat_has_user_context_only_no_web() {
    let a = assemble_mode(CapabilitySet::default()).unwrap();
    assert!(a.config.tool_pool.iter().any(|t| t == "user_context"));
    assert!(!a.config.tool_pool.iter().any(|t| t == "web_search"));
    assert_eq!(a.system_prompt_parts.len(), 1);
}

#[test]
fn dual_unions_search_tools_and_hybrid_contract() {
    let a = assemble_mode(CapabilitySet { rag: true, search: true }).unwrap();
    assert!(a.config.tool_pool.iter().any(|t| t == "web_search") || a.config.tool_pool.is_empty() && /* rag uses codegen pool empty */);
    // search mode lists web_search in tool_pool — after merge must include them
    assert!(a.config.tool_pool.iter().any(|t| t == "web_search"));
    assert_eq!(a.system_prompt_parts.len(), 3);
}
```

Note: current `rag.yaml` has empty `tool_pool` (codegen path). Dual must still expose search tools + rag skill catalog.

- [ ] **Step 3: Run**

```bash
cd /home/chuan/context-osv6/avrag-rs && cargo test -p app-chat --lib mode_assemble::
cargo test -p agent-loop --lib answer_contract
```

- [ ] **Step 4: Commit**

```bash
git add avrag-rs/crates/app-chat/src/mode_assemble.rs \
  avrag-rs/crates/app-chat/src/lib.rs \
  avrag-rs/crates/agent-loop/src/react_loop/
git commit -m "feat(app-chat): assemble ModeConfig from CapabilitySet"
```

---

### Task 6: Wire resolve + assemble into agent pipeline

**Files:**
- `avrag-rs/crates/app-chat/src/chat/pipeline_steps.rs`
- `avrag-rs/crates/app-chat/src/agents/unified/mod.rs` (and wherever `load_mode_config(kind)` is called)
- `avrag-rs/crates/app-chat/src/chat/service_modes.rs` / postprocess — set response `agent_type` from `CapabilitySet::agent_type_label()`
- Persist `capabilities` into assistant/user `turn_metadata` JSON: `{ "capabilities": ["rag"] }`

- [ ] **Step 1: At start of agent execute path**

```rust
let caps = resolve_capabilities(request.capabilities.as_deref(), &request.agent_type)?;
let assembled = assemble_mode(caps)?;
// Rewrite request.agent_type to derived label for persistence/telemetry
let mut request = request;
request.agent_type = caps.agent_type_label().to_string();
// Put caps + prompt parts into agent request metadata when building AgentRequest
metadata.insert("capabilities", json!(caps.as_string_list()));
metadata.insert("system_prompt_parts", json!(assembled.system_prompt_parts));
// Use assembled.config instead of load_mode_config(agent_type)
```

Map `AgentKind` for remaining code that still needs enum:

```rust
fn agent_kind_for(caps: CapabilitySet) -> AgentKind {
    match (caps.rag, caps.search) {
        (false, false) => AgentKind::Chat,
        (true, false) => AgentKind::Rag,
        (false, true) => AgentKind::Search,
        (true, true) => AgentKind::Rag, // dual: prefer Rag kind for codegen path presence; config already hybrid
    }
}
```

Dual kind choice: prefer **Rag** so codegen retrieve remains available when rag is on; search tools still in tool_pool from assemble. Document this in code comment.

- [ ] **Step 2: Targeted tests**

```bash
cd /home/chuan/context-osv6/avrag-rs && cargo test -p app-chat --lib
cargo test -p app-bootstrap --lib
```

Fix compile breakages from ModeConfig / AgentKind assumptions.

- [ ] **Step 3: Commit**

```bash
git commit -am "feat(app-chat): wire CapabilitySet assembly into agent pipeline"
```

---

### Task 7: Transport — client IP into execution context

**Files:**
- `avrag-rs/crates/transport-http/src/handlers/chat.rs` (and stream path)
- `avrag-rs/crates/agent-tools/src/skills/registry.rs` — extend `ExecutionContext`
- `avrag-rs/crates/agent-tools/src/tool_registry.rs` — build context with new fields
- `avrag-rs/crates/app-chat` dispatch path that constructs `ExecutionContext`

- [ ] **Step 1: Extend context**

```rust
pub struct ExecutionContext<'a> {
    pub search_provider: Option<&'a dyn avrag_search::SearchProvider>,
    pub auth: Option<&'a contracts::auth_runtime::AuthContext>,
    pub session_id: Option<uuid::Uuid>,
    pub chat_persistence: Option<&'a dyn ChatPersistencePort>,
    /// Client IP for user_context geo (may be "unknown").
    pub client_ip: Option<String>,
    pub client_local_time: Option<String>,
    pub client_timezone: Option<String>,
}
```

Update `new` / `with_memory` constructors with default `None` for new fields; add builder methods or a fuller constructor used by production.

- [ ] **Step 2: In chat handler**, reuse existing `extract_client_ip`; stash on request metadata before `conversation().execute`:

```rust
// e.g. extend ChatRequest is frozen — prefer metadata side channel:
// If ChatRequest cannot hold IP, put into a request-scoped structure ChatContext already uses,
// OR add optional #[serde(skip)] only fields — better: pass via ChatContext thread/task local is wrong.
// Preferred: add to ChatRequest as non-serialized internal is impossible.
// Practical: add optional field on ChatRequest:
//   #[serde(default, skip_serializing_if = "Option::is_none")]
//   pub client_ip: Option<String>,  // set only by server, clients' value ignored/overwritten
```

**Server overwrites** any client-supplied `client_ip` with extracted IP (security).

Add to contract:

```rust
/// Set by server only; client values overwritten.
#[serde(default, skip_serializing_if = "Option::is_none")]
pub client_ip: Option<String>,
```

- [ ] **Step 3: Commit**

```bash
git commit -am "feat: plumb client_ip and client_context into tool ExecutionContext"
```

---

### Task 8: GeoIP helper + `user_context` skill

**Files:**
- Create: `avrag-rs/crates/agent-tools/src/geoip.rs`
- Create: `avrag-rs/crates/agent-tools/src/skills/builtin/user_context.rs`
- Modify: `builtin/mod.rs`, `lib.rs`, `Cargo.toml`
- Modify: `avrag-rs/.env.example`
- Ensure capability schemas / tool registry list `user_context` for resolve

- [ ] **Step 1: Add dependency**

```toml
# agent-tools/Cargo.toml
maxminddb = "0.24"
```

(Use a version that compiles on workspace MSRV; adjust if resolve fails.)

- [ ] **Step 2: `geoip.rs`**

```rust
use std::net::IpAddr;
use std::path::Path;
use std::sync::OnceLock;

pub struct GeoLookup {
    // store Reader if open succeeds
}

pub struct GeoCity {
    pub country: Option<String>,
    pub region: Option<String>,
    pub city: Option<String>,
}

pub fn lookup_city(ip: &str) -> Result<GeoCity, String> {
    // parse IpAddr; reject private/loopback → Err("private_or_local_ip")
    // path from GEOIP_CITY_DB_PATH
    // missing file → Err("geoip_db_unavailable")
    // maxmind lookup city
}
```

- [ ] **Step 3: Skill `user_context`**

```rust
pub struct UserContextSkill;

// id: "user_context"
// description: Load user local time and city-level geo from IP. Call before weather when location/date not specified.
// execute:
//   local_time/timezone from ctx
//   geo from lookup_city(ctx.client_ip)
//   return ToolResult Ok with JSON per design; confidence none on failure — never invent city
```

Register in `register_all`.

- [ ] **Step 4: Unit tests**

```rust
#[tokio::test]
async fn user_context_returns_clock_without_geo_db() {
    let mut ctx = ExecutionContext::new(None);
    ctx.client_local_time = Some("2026-07-15T14:32:00+08:00".into());
    ctx.client_timezone = Some("Asia/Shanghai".into());
    ctx.client_ip = Some("8.8.8.8".into()); // may fail geo without db
    let skill = UserContextSkill;
    let result = skill.execute(&json!({}), &ctx).await;
    assert_eq!(result.status, ToolStatus::Ok);
    let data = result.data.unwrap();
    assert_eq!(data["timezone"], "Asia/Shanghai");
    // geo.confidence is "none" or "city" depending on env — both ok if structured
    assert!(data.get("geo").is_some() || data.get("local_time").is_some());
}

#[test]
fn private_ip_no_city() {
    let err = lookup_city("127.0.0.1").unwrap_err();
    assert!(err.contains("private") || err.contains("local"));
}
```

- [ ] **Step 5: `.env.example`**

```bash
# MaxMind GeoLite2 City DB for user_context tool (optional; degrades geo to none)
# GEOIP_CITY_DB_PATH=/var/lib/GeoIP/GeoLite2-City.mmdb
```

- [ ] **Step 6: Run**

```bash
cd /home/chuan/context-osv6/avrag-rs && cargo test -p agent-tools --lib
```

- [ ] **Step 7: Commit**

```bash
git add avrag-rs/crates/agent-tools avrag-rs/.env.example
git commit -m "feat(agent-tools): add user_context skill with MaxMind geo lookup"
```

---

### Task 9: Frontend — capabilities state + request payload

**Files:**
- `frontend_next/lib/workspace/ui-store.ts`
- `frontend_next/hooks/chat-session/use-chat-stream.ts`
- `frontend_next/lib/workspace/client.ts` / generated `ChatRequest` types
- `frontend_next/hooks/chat-session/helpers.ts`

- [ ] **Step 1: Types**

```ts
export type WorkspaceCapability = "rag" | "search";

// WorkspaceUiState:
//   capabilities: WorkspaceCapability[];  // default []
//   // deprecate chatMode as source of truth; keep temporary mapping for migration if needed

export function toggleCapability(
  current: WorkspaceCapability[],
  cap: WorkspaceCapability,
): WorkspaceCapability[] {
  return current.includes(cap)
    ? current.filter((c) => c !== cap)
    : [...current, cap];
}

export function buildClientContext(): {
  local_time: string;
  timezone: string;
} {
  const timezone = Intl.DateTimeFormat().resolvedOptions().timeZone || "UTC";
  const local_time = formatOffsetIso(new Date()); // implement with toISOString or manual offset
  return { local_time, timezone };
}
```

Session-only: **do not persist capabilities in zustand persist blob** (or reset on `resetWorkspace` / new session). If workspace UI is currently persisted, store capabilities only in React session state for chat pane **or** clear capabilities key in persist partializer.

Design: session-only → prefer chat-session hook state over long-lived workspace persist.

- [ ] **Step 2: streamChat body**

```ts
await streamChat(token, {
  query: trimmedQuery,
  workspace_id: workspaceIdRef.current,
  session_id: requestSessionId,
  // derived for older servers during rollout; still send
  agent_type: deriveAgentTypeLabel(capabilitiesRef.current), // chat | rag | search | rag+search
  capabilities: capabilitiesRef.current,
  client_context: buildClientContext(),
  doc_scope: selectedSourceIdsRef.current,
  messages: [],
  stream: true,
});
```

- [ ] **Step 3: Message model**

```ts
// Chat message:
capabilities?: WorkspaceCapability[] | null;
// normalize from turn_metadata.capabilities or legacy mode
```

- [ ] **Step 4: Unit tests**

Update `workspace-chat-pane.modes.test.tsx` to assert request contains `capabilities: []` or toggled values; remove write expectations.

```bash
cd /home/chuan/context-osv6/frontend_next && pnpm exec vitest run tests/workspace/workspace-chat-pane.modes.test.tsx
```

- [ ] **Step 5: Commit**

```bash
git commit -am "feat(frontend): send capabilities and client_context on chat stream"
```

---

### Task 10: Frontend — composer tags + message chips; remove write UI

**Files:**
- `frontend_next/components/workspace/chat-composer.tsx` + CSS module
- `frontend_next/components/workspace/chat-message-list.tsx`
- `frontend_next/lib/i18n/messages/workspace.ts`
- `frontend_next/e2e/pom/chat-panel-page.ts`
- Tests under `frontend_next/tests/workspace/*`

- [ ] **Step 1: Replace mode menu with two toggles**

```tsx
// data-testid="workspace-chat-cap-rag"
// data-testid="workspace-chat-cap-search"
// aria-pressed={capabilities.includes("rag")}
// onClick → onCapabilitiesChange(toggleCapability(...))
// Remove write from CHAT_MODE_ORDER / menu
```

Props:

```ts
capabilities: WorkspaceCapability[];
onCapabilitiesChange: (next: WorkspaceCapability[]) => void;
```

- [ ] **Step 2: Message chips**

Render up to two chips from `message.capabilities`; if empty, no chip or subtle “chat” only if product wants — design allows either; **prefer no chip when empty**.

- [ ] **Step 3: POM**

```ts
async setCapabilities(caps: Array<"rag" | "search">) {
  // ensure only desired pressed
}
async switchToWebSearchMode() {
  await this.setCapabilities(["search"]);
}
```

- [ ] **Step 4: Run frontend tests**

```bash
cd /home/chuan/context-osv6/frontend_next && pnpm exec vitest run tests/workspace/
```

- [ ] **Step 5: Commit**

```bash
git commit -am "feat(frontend): capability tag toggles and message chips; remove write mode UI"
```

---

### Task 11: Capability registry / schemas / guides cleanup

**Files:**
- `avrag-rs/crates/agent-tools/src/capability/schemas.rs` — remove or stop advertising write mode schema as product; ensure chat base includes `user_context` external tool if listed
- `avrag-rs/crates/app-chat/src/external_agent_guide.rs` — guides for rag/search only; dual optional
- MCP catalog `agent_type` docs: note capabilities
- Fix product e2e that **must** still compile: `write_real.rs` either `#[ignore]` with note write offline or expect 4xx

- [ ] **Step 1: Align schemas**

```rust
// chat_mode_schema tool_pool or external_tools include user_context if that schema is used for disclosure
```

- [ ] **Step 2: write e2e**

```rust
// write_real.rs — at top of test:
// assert error write_mode_disabled OR #[ignore = "write product offline 2026-07-15"]
```

Prefer explicit assert reject if test harness is cheap; else ignore with reason.

- [ ] **Step 3: Run broader L1 subset**

```bash
cd /home/chuan/context-osv6/avrag-rs && cargo test -p agent-tools --lib && cargo test -p app-chat --lib && cargo test -p app-bootstrap --lib
```

- [ ] **Step 4: Commit**

```bash
git commit -am "chore: align capability schemas and park write e2e after product offline"
```

---

### Task 12: Docs + graphify + final verify

**Files:**
- Short note in `avrag-rs/prompts/README.md` if it lists orchestrators
- Design doc status already Approved; plan this file
- `graphify update .` after structural Rust/TS symbol changes

- [ ] **Step 1: Final targeted verify**

```bash
cd /home/chuan/context-osv6/avrag-rs && cargo test -p contracts --manifest-path ../contracts/Cargo.toml 2>/dev/null || \
  (cd /home/chuan/context-osv6/contracts && cargo test --test chat_json)
cd /home/chuan/context-osv6/avrag-rs && cargo test -p agent-tools --lib
cd /home/chuan/context-osv6/avrag-rs && cargo test -p app-chat --lib
cd /home/chuan/context-osv6/avrag-rs && cargo test -p app-bootstrap --lib
cd /home/chuan/context-osv6/frontend_next && pnpm exec vitest run tests/workspace/workspace-chat-pane.modes.test.tsx
```

- [ ] **Step 2: graphify**

```bash
cd /home/chuan/context-osv6 && graphify update .
```

- [ ] **Step 3: Commit docs if any**

```bash
git add docs/engineering/ avrag-rs/prompts/README.md 2>/dev/null
git commit -m "docs: note capability multiselect cutover and user_context ops (GeoIP path)"
```

---

## Spec coverage checklist

| Spec requirement | Task(s) |
|------------------|---------|
| Multi-select RAG + Search only | 2, 5, 9, 10 |
| Empty = pure chat, no RAG/web tools | 5, 6 |
| `capabilities[]` API | 1, 6, 9 |
| Legacy `agent_type` map | 2, 6 |
| Write product reject | 3, 11 |
| Prompt base + manuals | 4, 5, 6 |
| Dual union + max budget + hybrid contract | 5 |
| Session-only capability memory | 9 |
| Message 0–2 chips + store capabilities | 6, 9, 10 |
| `user_context` base tool | 5, 8 |
| Frontend clock | 9, 7 |
| MaxMind city one-shot | 8 |
| No deep write-core delete | 3 (boundary only) |

## Placeholder / consistency self-review

- Types: `CapabilitySet`, `ClientContext`, `user_context` tool id, error code `write_mode_disabled`, dual label `rag+search` — used consistently.
- No TBD steps remaining for product intent; GeoLite2 packaging is env-path only (ops).
- Dual `AgentKind` mapping to `Rag` is explicit; hybrid behavior lives in assembled `ModeConfig`.

---

## Execution handoff

Plan saved to:

`docs/engineering/CAPABILITIES_MULTISELECT_AND_USER_CONTEXT_PLAN_2026-07-15.md`

**Two execution options:**

1. **Subagent-Driven (recommended)** — fresh subagent per task, review between tasks  
2. **Inline Execution** — execute tasks in this session with checkpoints  

Which approach?
