# Context-OS Global Context & Domain Dictionary (CONTEXT.md)

This document serves as the project's source of truth for architecture, domain terminology, development status, and remaining work.

---

## 1. Domain Dictionary (Ubiquitous Language)

To maintain semantic consistency across the codebase, tests, and documentation, the following terminology is defined:

| Term | Definition | Context / Scope |
| :--- | :--- | :--- |
| **Unified Agent** | The core single-agent service architecture (V5) that replaced the legacy multi-agent graph flows. It handles stream chats, RAG execution, and web searches. | Backend (`avrag-rs/crates/app`) |
| **avrag-api** | The HTTP/REST API server providing backend functionalities to the frontend, including authentication, chat streams, and workspace uploads. | Backend (`avrag-rs/bins/api`) |
| **avrag-worker** | The background worker that claims and processes asynchronous ingestion, analytics, audit logging, and document cleanup tasks. | Backend (`avrag-rs/bins/worker`) |
| **frontend_next** | The main production frontend application built using Next.js 15+, React, TypeScript, Tailwind CSS, and `pnpm`. | Frontend (`frontend_next`) |
| **RAG Ingestion** | The multi-stage process of converting uploaded documents (PDFs, Markdown, Office Docs) into chunked, normalized representations and indexing them into Milvus/PostgreSQL. | Pipeline (`crates/ingestion` & `bins/worker`) |
| **E2E Throttling Bypass** | The mechanism where the HTTP rate-limiter is raised to 10,000 RPM when `E2E_ENABLED=true` is set, avoiding HTTP 429 errors during automated testing. | Middleware (`crates/transport-http`) |
| **Free Tier** | The free billing tier, providing base quota limits for chat, RAG, and document storage. | Billing (`avrag-rs/crates/billing`) |
| **Plus Tier** | The mid-level subscription tier (replacing the legacy Enterprise tier), offering higher usage quotas and advanced search features. | Billing (`avrag-rs/crates/billing`) |
| **Pro Tier** | The highest subscription tier, offering maximum execution limits and priority resources. | Billing (`avrag-rs/crates/billing`) |
| **Creem Provider** | B2C manual subscription billing provider via Creem checkout for global credit cards. | Billing (`avrag-rs/crates/billing`) |
| **Alipay Provider** | B2C manual subscription billing provider via Alipay precreate QR scan-code for CNY payments. | Billing (`avrag-rs/crates/billing`) |
| **Lazy Billing Downgrade** | Automatic check and state transition of expired user subscriptions to 'expired' and downgrade to the free tier upon access or API query. | Billing (`avrag-rs/crates/billing`) |
| **LoopOptimizer** | ReActLoop 的参谋模块。基于跨迭代信号（重复 chunk、budget 余量）向 LLM 上下文注入优化提示，不替代 LLM 决策权。 | Agent Loop (`avrag-rs/crates/app/src/agents/loop/optimizer.rs`) |
| **IterationProgress** | LoopOptimizer 的跨轮状态追踪器。仅记录 chunk_id 的首次出现轮次，不存储评分或原文。 | Agent Loop (`avrag-rs/crates/app/src/agents/loop/optimizer.rs`) |
| **ContextAdjustment** | LoopOptimizer 的输出：可能是重复 chunk 提示、budget 预警，或无需干预。 | Agent Loop (`avrag-rs/crates/app/src/agents/loop/optimizer.rs`) |
| **ModeSchema** | ReAct loop 执行模式的静态元数据（id、requires_internet、external_tools_used）。原名 `StrategySchema`，已删除虚假的状态转移描述。 | Capability API (`avrag-rs/crates/app/src/agents/capability/schemas.rs`) |
| **Messenger Model** | Agent 决策模式：LLM 是最终决策者，代码负责传递上下文和执行 tool。`ReActLoop` 采用此模式。 | Agent Architecture |
| **Commander Model** | Agent 决策模式：代码基于信号评估强制改变 LLM 的检索策略。`evaluator.rs` 原设计采用此模式，已废弃。 | Agent Architecture (deprecated) |
| **AgentKind** / **AgentMode** | 同义词，指代 chat/rag/search 三种 ReAct loop 执行模式。`AgentKind` 是 enum 名，`ModeConfig.id` / `ModeSchema.id` 是字符串标识。 | Agent Architecture |
| **Subagents** | 未来的 auto mode 架构方向：由 Orchestrator Agent 将任务委派给多个 Specialist Subagent，各自独立执行。与规则引擎有本质区别。 | Agent Architecture (future) |
| **ContextAssembler** | 每轮按「披露阶段 + 触发」组装 ReAct loop 的 system 上下文（orchestrator 全文 + 披露切片）。三种 mode 仅靠 `ModeConfig` 数据区分，不含 `mode.id==` 硬编码分支。 | Agent Loop (`loop/assembler.rs`) |
| **Disclosure Phase（披露阶段）** | 渐进披露的轴：`Retrieve`（检索阶段）/ `Synthesis`（合成阶段）。**取代已废弃的 `round_idx` 轴**（ReAct 轮数由 LLM 决定、不固定，按轮号编号失配）。「首个检索轮」是 assembler 内部状态，非配置键。 | Agent Loop (`loop/assembler.rs`) |
| **Disclosure Trigger（披露触发）** | 决定「何时递交 skill body」的事件：`mandatory`（首个检索轮强制项 / Synthesis 强制 answer）或 `skill_request`（LLM 主动请求能力簇）。 | Agent Loop (`loop/assembler.rs`) |
| **ClusterIndex / SkillBody** | 渐进披露的两层：ClusterIndex 仅给 LLM「有哪些能力簇」（低 token）；SkillBody 是被请求簇后注入的完整 SKILL.md 正文。 | Agent Loop (`loop/assembler.rs`) |
| **Retrieval Bridge** | 沙箱 codegen 与宿主 `RagRuntime` 之间的 fd 管道 JSON RPC（非网络）。模型写 `client.dense_search(...)` 时，宿主强制 `doc_scope` 并调用 `tools::dispatch`。见 ADR-0009。 | Code Interpreter + RAG (`code-interpreter/bridge.rs`, `rag-core/runtime/bridge.rs`) |

---

## 2. Workspace & Git Worktrees

The repository's git worktree structure has been consolidated and cleaned up:

- **Primary Workspace**: `/home/chuan/context-osv6` (on `master` branch) contains all current active developments, unified tests, and frontend wiring.
- **Obsolete/Merged Worktrees Cleaned**:
  - `worktree-agent-ae326bb2e7a264b82`: Fully merged to `master` and physically removed.
  - `worktree-p0-prompt-injection-fixes`: Outdated and superseded by V5 migration. Removed.
  - `worktree-e2e-analyzer`: Obsolete. Active logic integrated directly into the `master` workspace as uncommitted changes.
  - Temporary detached worktrees in `/tmp` have been pruned.

---

## 3. Development Stage Assessment

The project is currently in **Phase 5 (Unified Agent Integration & End-to-End Hardening)** with active **pricing-tier revamp** work on branch `feat/pricing-tiers-revamp`.

- **Backend Status**: All Rust unit tests and contract integration tests pass. The migration from legacy graph flows to the `UnifiedAgentService` is complete. Billing exposes rolling-window usage (`/billing/usage/window`) and structured quota denial reasons (`QuotaDenyReason`). The system relies on Postgres, Redis, Milvus, and MinIO.
- **Agent Architecture Cleanup (In Progress)**: Architecture review completed. Six design documents produced for post-migration cleanup: `LoopOptimizer` replaces the unused `evaluator.rs` (commander model → messenger model), `RouterPolicy` removal (telemetry-only pass-through), `ModeSchema` alignment (removing false state-machine semantics from `StrategySchema`), v5 state-machine residue cleanup (`StateRecord`, `StateTransition`, search-round counters, `rig_adapter.rs`), frontend `useChatSession` hook extraction from the 2514-line `WorkspaceChatPane` god component, and `RawWorkspace*` mapping layer removal. See `docs/agents/*.md`.
- **Frontend Status**: The production frontend (`frontend_next`) is fully updated. Settings billing tab and the dashboard header wire `UsageMeter` (5h/7d rolling windows, compact variant on dashboard) with `data-testid` hooks (`usage-meter`, `plan-display`). Dynamic routing parameters support Next.js 15's promise-based architecture. Vitest covers billing format/API/UsageMeter components.
- **E2E Test Architecture**: Playwright runs `smoke`, `journey`, `skills`, `billing`, and `visual` suites. Journey specs use isolated run contexts; `avrag-worker` is in the Playwright `webServer` lifecycle with a TCP health check on port `8081` for ingestion polling. Billing E2E asserts usage meter and plan display on `/settings?tab=billing`.

---

## 4. Remaining Gaps & Goals

### Gaps
1. **Document Ingestion Worker Latency in E2E**: Background tasks processed via `avrag-worker` are asynchronous. Ingestion specs must keep robust polling with sensible timeouts for worker status updates.
2. **Environment Configuration Safety**: Local testing relies on Milvus, MinIO, Redis, and Postgres. CI needs containerized service bindings or mocks.

### Goals
1. **Complete E2E Verification**: Run and stabilize all Playwright specs including billing (`usage-settings`, `usage-meter`) and journey suites.
2. **Pricing Revamp Rollout**: Merge tier quota changes (migration 0037), remove dead feature flags, and land structured billing UX.
3. **Clean Root Hygiene**: Keep design blueprints, checklists, and specs inside `docs/`.
4. **Agent Architecture Cleanup Implementation**: Execute the six design documents in `docs/agents/` — `LoopOptimizer`, `RouterPolicy` removal, `ModeSchema` alignment, v5 residue cleanup, frontend `useChatSession` hook, and mapping layer removal.
