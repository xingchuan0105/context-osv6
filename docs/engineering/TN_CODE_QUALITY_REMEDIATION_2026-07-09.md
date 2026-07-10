# Thermo-Nuclear 代码质量整改方案（2026-07-09）

| 字段 | 值 |
|------|-----|
| 状态 | **Done** — Wave 0–6 + 可选 P0–P7 收口；Capability/Skill/Tool 三层保留（ADR-0006 §5a）；见交接文档 |
| 日期 | 2026-07-09 |
| 交接 | [`TN_REMEDIATION_HANDOFF_2026-07-09.md`](./TN_REMEDIATION_HANDOFF_2026-07-09.md) |
| 范围 | `avrag-rs`、`frontend_next`、`contracts`、`desktop`、worker、scripts（**不含** `frontend_rust`） |
| 触发 | Thermo-Nuclear code quality review（全库复审，排除 Rust 前端） |
| 对齐 | ADR-0006（AgentLoop + ToolCall；域 crate 拆分）；`docs/adr/0006-execute-plan-removal-inventory.md`；`docs/HEALTH_OPTIMIZATION_HANDOFF_2026-06-11.md` |
| 原则 | **删概念优先于搬文件**；行为可观测等价；本地 trunk + 定向测试（见 `docs/engineering/SOLO_DISCIPLINE.md`） |

---

## 1. 背景与判定

### 1.1 相对 2026-06-11 已完成

| 项 | 结果 |
|----|------|
| `app` 拆为 `app-core` / `app-chat` / `app-documents` / `app-admin` / `app-billing` / `app-bootstrap` | 完成 |
| Graphflow 删除 → 线性 chat pipeline | 完成 |
| `NotebookAnalysisCollector`、WorkspaceSurface hooks | 完成 |
| `storage-pg` 与 `ingestion` 解耦（`ingestion-types`） | 完成（prod） |
| lib→components 反向依赖、部分错误路径测试 | 完成 |
| execute-plan **HTTP 产品入口**物理删除 | 完成 |
| `write-core` 抽出（Write 域第一刀） | 进行中 / 已有基础 |

### 1.2 复审结论（不批准「结构健康」）

| 症状 | 本质 |
|------|------|
| 上帝模块从 `app` 迁到 `app-chat`（~28k LOC） | **搬家，未减概念** |
| 4 套工具注册表 + tool-name match | **同职责多真相源** |
| Execute-plan 路由已删，内部 plan 栈仍在 | **双栈并行（僵尸架构）** |
| `AppState` ~125 透传方法 | **identity wrapper 层** |
| `app/ports` + 三套 `ChatService` | **幽灵六边形，仅测试使用** |
| `AgentRequest` 用 `serde_json::Value` 装 auth | **类型边界塌陷** |
| `notebook_id`/`workspace_id` 前端每 API 映射 | **契约层未收敛** |
| `check_file_size_limits.sh` 指向已删路径 | **门禁失效** |

**一句话目标**：让「工具从披露到执行」「检索如何运行」「HTTP 如何调领域」各自只剩 **一套** 心智模型。

---

## 2. 目标架构（目标态）

```text
                    ┌─────────────────────────────────────┐
                    │ transport-http (handlers / routes)  │
                    │  直接持有/注入域 Context，禁止透传墙  │
                    └───────────────┬─────────────────────┘
                                    │
          ┌─────────────────────────┼─────────────────────────┐
          ▼                         ▼                         ▼
   ChatContext              DocumentContext            Billing/Admin...
   (chat-orchestrator)      (app-documents)            (已有 crate)
          │
          ▼
   ┌──────────────────────────────────────────────────────┐
   │ agent-loop (ReActLoop + ModeConfig + policy)         │
   │   ToolRegistry::execute(tool_id, args, ctx)          │
   └───────────────────────┬──────────────────────────────┘
                           │
           ┌───────────────┼───────────────┐
           ▼               ▼               ▼
     RAG tools        Builtin tools    Write path
     (rag-core via    (web/calc/ci…)   (write-core)
      Tool trait)                      已在 pipeline 分流
```

**删除的概念（目标态不应再存在）**：

1. ~~`Capability` / `Skill` / `Tool` 产品词合并~~ — **作废**。产品定义见 ADR-0006 §5a：三层 **intentionally 区分**。只删 **第二套执行调度**（atomic_tools 平行 dispatch、loop 假分支、tools HashMap 双写），**不**合并 Capability/Skill/Tool 概念
2. `atomic_tools` 作为与 **执行 catalog** 平行的第二调度面
3. 产品路径上的 `ExecutePlanRequest` 解析与 `RagRuntime::execute_plan` 主入口
4. `AppState` 上数百个 `self.chat_ctx().foo()` 透传方法
5. 未接线的 `app/src/ports` + `app/src/services/chat` 假架构
6. 前端每个函数手写 `notebook_id ↔ workspace_id`

---

## 3. 工作波次总览

```text
Wave 0  门禁与清单止血          （0.5–1 天）  无行为风险
Wave 1  ToolRegistry 合并       （3–5 天）    核心 code judo
Wave 2  Execute-plan 内栈清零   （3–5 天）    对齐 ADR-0006
Wave 3  AppState 透传拆除       （2–3 天）    HTTP 边界变干净
Wave 4  幽灵架构删除 + 强类型   （2–3 天）    契约清晰
Wave 5  命名收敛 + 依赖倒置     （3–5 天）    跨层清理
Wave 6  app-chat 再拆 crate     （可选，波次 1–2 后）
```

**依赖**：Wave 1 不依赖 Wave 2，但 **建议 1→2**（工具面先干净，再拆检索入口）。  
Wave 3 可与 Wave 2 后期并行。Wave 6 仅在 1–2 稳定后做，避免同时搬家+改语义。

**验证默认值**（每波结束）：

```bash
# 触达 crate 的定向测试（示例，按波次替换 -p）
cargo test -p avrag-rag-core -p app-chat -p transport-http --lib
# 前端触达包
cd frontend_next && pnpm test --run <相关路径>
# 文件体积门禁
bash scripts/check_file_size_limits.sh
```

不强制每提交全量 E2E；波末或发版前再跑 smoke（Solo Discipline）。

---

## 4. Wave 0 — 门禁与清单止血

### 4.1 问题

`scripts/check_file_size_limits.sh` 仍引用已删除路径（如 `usage-limit`、`app/src/chat/graphflow.rs`），脚本无法作为真实门禁。

### 4.2 方案

重写 allowlist 为 **当前热点 + 硬阈值**：

| 档位 | 阈值 | 示例路径 |
|------|------|----------|
| Hard | 1000 | 任何 prod 源文件不得超过（测试文件可放宽到 1600 并单独列表） |
| Soft warn | 800 | `document_pipeline.rs`、`rag-core/.../execute.rs`、`embedding.rs` |
| Soft warn | 600 | 新建模块默认拆分线 |

**建议纳入硬监控的路径（初始集）**：

```text
avrag-rs/bins/worker/src/pipeline/document_pipeline.rs
avrag-rs/crates/rag-core/src/runtime/execute.rs
avrag-rs/crates/llm/src/embedding.rs
avrag-rs/crates/app-chat/src/token_budget/mod.rs
avrag-rs/crates/app-chat/src/agents/loop/answer_contract.rs
avrag-rs/crates/app-bootstrap/src/app_state/*.rs   # 透传拆除前监控总行数
frontend_next/lib/workspace/client.ts
frontend_next/components/admin/admin-i18n.ts
frontend_next/components/admin/admin-ops-surfaces.tsx
```

缺失文件 → 从列表删除，**不要** `set -e` 在 wc 不存在文件时静默跳过而不报「列表陈旧」。

### 4.3 验收

- [ ] `bash scripts/check_file_size_limits.sh` 在干净工作树退出 0 或仅报告真实超标
- [ ] CI/本地 pre-commit 可选挂接（solo 默认本地跑即可）

### 4.4 提交粒度

单 commit：`chore: revive file size gate for current hotspots`

---

## 5. Wave 1 — ToolRegistry 合并（最高杠杆 code judo）

### 5.1 现状（必须收敛的是 **执行多入口**，不是产品三层词）

产品词 **Capability / Skill / Tool** 分层见 **ADR-0006 §5a**（禁止合并为「一套概念」）。

| 组件 | 产品层 | 问题（整改前） |
|------|--------|----------------|
| `capability::CapabilityRegistry` | **Capability**（模式/披露/政策） | 曾双写 tools HashMap；应 **投影** 执行表，本身保留 |
| `progressive::PromptRegistry` | **Skill** 资产加载 | 可作 loader；不自称第二执行 framework |
| `skills::SkillRegistry` / components | Skill 实现 ↔ Tool 适配 | 执行应挂 **统一 ToolCatalog**，不是 loop 再 match |
| `unified::atomic_tools::dispatch_*` | （无独立产品词） | 与 catalog 平行的第二调度 → **删/内收** |
| `iteration_tools` match 字符串 | （无） | 假分支（search/native 同路径）→ **删** |

### 5.2 目标模型

```rust
// 示意：单一注册表（名称可定为 ToolRegistry 或 AgentToolCatalog）
pub struct ToolRegistry {
    tools: HashMap<ToolId, RegisteredTool>,
}

pub struct RegisteredTool {
    pub meta: ToolMeta,           // id, version, description, input_schema, tags
    pub disclose: DisclosePolicy, // plan / retrieve / never — 来自 ModeConfig.tool_pool
    pub exec: Arc<dyn ToolExec>,  // async execute(args, ToolContext) -> ToolResult
}

pub struct ToolContext<'a> {
    pub auth: &'a AuthContext,           // 强类型，禁止 Value
    pub session_id: Option<Uuid>,
    pub doc_scope: &'a [String],
    pub search: Option<&'a dyn SearchProvider>,
    pub rag: Option<&'a RagRuntime>,     // 或更窄的 RetrievalPort
    pub chat_persistence: Option<&'a dyn ChatPersistencePort>,
    // …
}
```

**ModeConfig** 只表达：

- `tool_pool: Vec<ToolId>`（或分 phase 的 pool）
- 迭代/退出策略  
不再持有第二份 schema 拷贝；schema 一律 `registry.meta(id)`。

### 5.3 迁移步骤（可合并为 2–3 个本地 commit）

| 步骤 | 动作 | 验证 |
|------|------|------|
| 1.1 | 新增 `ToolRegistry` + `ToolExec` trait，把 **builtin skills** 迁入为 adapter | `cargo test -p app-chat --lib skills` |
| 1.2 | `CapabilityRegistry.standard()` 改为 **从 ToolRegistry 投影** meta（过渡期双读） | capability API 契约测试 |
| 1.3 | `ReActLoop::dispatch_tool_call` **只**调 `registry.execute`；删除 search/native 重复方法 | loop 单测 + chat smoke |
| 1.4 | RAG 工具注册为 `ToolExec`（内部调现有 `dispatch_rag_tool`） | RAG tool 单测 |
| 1.5 | 删除 `atomic_tools::dispatch_*` 公共 API 或标 `pub(crate)` 并缩成 registry 内部 | 无外部引用 |
| 1.6 | `PromptRegistry` 降为 **仅 loader**（`load_skill_md`），不再自称 framework | 文档与 mod.rs 注释 |
| 1.7 | 删除 tool-name 硬编码 match（若仍有，必须是 registry 内 tag，而非 loop 层） | 全库 `dense_retrieval` match 仅 registry |

### 5.4 刻意不改（本波）

- Write 仍走 `writer::run_write_mode`（与 UnifiedAgent 分流保留，ADR 已接受）
- Mode YAML 文件格式可暂不改字段名，只改加载后绑定 registry 的方式

### 5.5 验收

- [ ] 新增一个 dummy **tool** 只需在 **执行 catalog 一处** register，即可被某 mode 的 tool_pool 披露并 `dispatch`
- [ ] `CapabilityRegistry` 仍表达 **agent 能力/模式**；不要求删除或并入 ToolCatalog
- [ ] Skill 资产加载路径清晰；**不**与 Tool 执行表抢「唯一注册表」叙事
- [ ] `iteration_tools` 中无「search 与 native 同体」的空分支
- [ ] Chat / RAG / Search 各至少一条定向测试绿

### 5.6 风险与回滚

| 风险 | 缓解 |
|------|------|
| 工具权限/enforcement 回归 | 保留现有 enforcement 函数，迁入 `ToolExec` 包装层，单测锁定 |
| mode tool_pool 漏注册 | 启动时 assert：pool 中每个 id 必须在 registry |
| 披露文案变化 | schema description 字节级对比快照（insta） |

回滚：保留旧 registry 文件一波次，经 feature 或模块 `legacy` 可切回（尽量避免 feature flag 长驻；优先短分支）。

---

## 6. Wave 2 — Execute-plan 内栈清零

对齐：`docs/adr/0006-execute-plan-removal-inventory.md`（HTTP 已删；本波完成 **内部** 删除期限前的实质收敛）。

### 6.1 问题

产品只认 AgentLoop + ToolCall，但：

- `contracts/src/rag_execute.rs` 仍持有完整 `ExecutePlanRequest`（~471 行）
- `rag-core/src/runtime/execute.rs`（~902 行）仍以 plan 为中心
- `execute_plan_policy.rs` 校验/转换仍在
- `app-chat/src/prompts/plan.rs` 仍解析 legacy ExecutePlan JSON

### 6.2 目标检索模型

```text
LLM / planner
    → Vec<ToolCall>   (dense_retrieval | lexical_retrieval | graph_retrieval | …)
    → ToolRegistry / Rag tool exec
    → 单工具实现内部做 channel 检索、融合、降级
    → ToolResult
```

**禁止**：产品路径再出现 `ExecutePlanRequest` 反序列化。

### 6.3 步骤

| 步骤 | 动作 | 说明 |
|------|------|------|
| 2.1 | 盘点 `ExecutePlanRequest` 全引用，分 **产品 / 单测 harness / 文档** | 更新 inventory ADR 勾选 |
| 2.2 | Prompt：删除 legacy ExecutePlan 解析分支；只产出 `ToolCall` | `prompts/plan.rs` |
| 2.3 | 将 `RagRuntime` 多 channel 能力暴露为 **按 tool 的方法**（或已有 tool 入口），测试改走 tool | 行为对齐现网 RAG |
| 2.4 | `execute_plan` 若需保留，**仅** `#[cfg(test)]` 或 `rag-core` dev harness，不进 lib 公共 API | 避免生产误用 |
| 2.5 | `execute_plan_policy`：删除 validate/convert 公共导出；有用的预算逻辑内联到 tool | |
| 2.6 | contracts：`ExecutePlanRequest` 标 `#[deprecated]` → 下个 minor 删除；或先移到 `contracts::legacy` | 与 typeshare/前端生成物同步 |
| 2.7 | 文档与 ADR inventory 勾选「semble 仅剩注释/changelog」 | |

### 6.4 验收

- [x] `rg ExecutePlanRequest` 产品路径仅剩 contracts 定义 + crate-private harness（`execute_plan_policy` / `execute.rs` / `response.rs`）
- [x] `RagRuntime::execute_plan` = `#[cfg(test)] pub(crate)`；policy 模块不公开 re-export
- [x] Prompt 无 fallback/normalize/convert helper；parse 拒收 legacy JSON
- [x] `answer_context` 使用 `RetrievalBundle`
- [x] Contracts `ExecutePlanRequest`/`Response`/`from_tool_calls` 标 `#[deprecated]`（删除日 2026-09-30）
- [x] `cargo test -p app-chat --lib` / `avrag-rag-core --lib` / `contracts` 绿
- [x] DTO **物理删除**（2026-07-09，提前于原 2026-09-30 目标）

### 6.5 与 Wave 1 的衔接

Wave 1 把 RAG 工具挂进统一 registry 后，Wave 2 删除 plan 入口时 **调用面更少**。若必须并行：先做 2.2 prompt 侧，再动 runtime。

---

## 7. Wave 3 — 拆除 `AppState` 透传墙

### 7.1 问题

`app-bootstrap/src/app_state/*_delegates.rs` 等约 **1.5k 行** identity wrapper，例如：

```rust
pub async fn list_sessions(...) {
    self.chat_ctx().list_sessions(...).await
}
```

字段已分 context，方法层却把边界抹平，handler 无法表达依赖。

### 7.2 目标

**方案 A（推荐，改动面可控）**：

```rust
// AppState 只暴露分面访问，不再堆业务方法
impl AppState {
    pub fn chat(&self) -> &ChatContext { &self.chat }
    pub fn documents(&self) -> &DocumentContext { &self.documents }
    pub fn billing(&self) -> &BillingContext { &self.billing }
    pub fn admin(&self) -> &AdminContext { &self.admin }
    // auth / storage 同理
}
```

Handlers：

```rust
state.chat().list_sessions(Some(&notebook_id)).await
// 或
let chat = state.chat();
```

**方案 B（更干净，改动更大）**：Axum 按路由 `State` 注入子 context（需 `FromRef`）。可二期做。

### 7.3 步骤

| 步骤 | 动作 |
|------|------|
| 3.1 | 为 `AppState` 增加 `chat()` / `documents()` / … 访问器 |
| 3.2 | `transport-http` 内机械替换 `state.list_sessions` → `state.chat().list_sessions`（可脚本化） |
| 3.3 | 删除 `chat_delegates.rs`、`share_delegates.rs` 等纯透传 impl |
| 3.4 | 测试与 e2e helper 同步替换 |
| 3.5 | 约定：新增领域能力 **只**加在对应 Context，禁止再往 `AppState` 堆方法 |

### 7.3.1 落地状态（2026-07-09）

| 项 | 状态 |
|----|------|
| `chat()` / `documents()` / `admin()` / `storage()` / `orchestrator()` 访问器 | **已做** |
| transport-http + app 测试：chat / citation 改 `state.chat()` | **已做** |
| 删除 `chat_delegates.rs`、`citation_delegates.rs` | **已做** |
| **`BoundDocuments` / `BoundAdmin`**（`state.docs()` / `state.admin_api()`） | **已做** |
| 删除 `notebooks.rs` / `documents.rs` / `url_imports` AppState 透传 | **已做** |
| admin API keys / notifications → `admin_api()` | **已做** |
| **`BoundShare` / `BoundPrefs`**（`state.share()` / `state.prefs()`） | **已做** |
| 删除 `share_delegates.rs` / `preferences.rs` / `admin_delegates.rs`（`create_share_token` 并入 BoundShare） | **已做** |
| **`BoundBilling` / `billing_api()`**；billing handlers 迁出 `postgres_delegates` | **已做** |
| workspace API key → `BoundAdmin::validate_workspace_api_key`；删 `auth_delegates.rs` | **已做** |
| `postgres_delegates` 仅 E2E + JWT version + upload_state | **保留**（非产品透传墙） |
| 约定 3.5 | **生效** |

### 7.4 验收

- [ ] `app_state` 下 `pub async fn` 数量显著下降（目标：**< 30** 个真正跨域/bootstrap 方法）
- [ ] `*_delegates.rs` 删除或只剩无法下沉的跨 context 编排（若有，应改名 `facade_*.rs` 并写清为何不能下沉）
- [ ] `cargo test -p transport-http` + 关键 chat/notebook 测试绿

### 7.5 非目标

不在本波重写 auth 中间件；不改变 URL 与 JSON 契约。

---

## 8. Wave 4 — 删除幽灵六边形 + `AgentRequest` 强类型

### 8.1 幽灵 `ChatService` / ports

**现状**：

| 类型 | 路径 | 谁用 |
|------|------|------|
| 端口版 `ChatService` | `app/src/services/chat/service.rs` | **仅** `chat_service_contract` 测试 |
| AppState 壳 | `app/src/services/chat_service.rs` | 测试辅助 |
| 真路径 | `ChatContext` / `app_chat::ChatService` | transport-http 生产 |

**方案（二选一，禁止并行）**：

| 选项 | 做法 | 适用 |
|------|------|------|
| **D1 删除（推荐）** | 删除 `app/src/ports/**`、`services/chat/**` 及仅服务它们的 contract 测试；文档注明生产架构是 Context 组合 | 当前无接线计划 |
| D2 真接线 | 生产改为端口注入，AppState 只组装 adapter | 成本高，与 Wave 3 冲突大 |

**默认执行 D1**。若未来要端口化，从 `ChatContext` 边界重做，不要复活空 ports。

### 8.2 `AgentRequest` 类型边界

**现状**：

```rust
pub auth_context: serde_json::Value,
pub user_preferences: Option<serde_json::Value>,
```

**目标**：

```rust
pub auth: AuthContext,  // 或最小化 AgentAuth { owner_user_id, user_id, roles… }
pub user_preferences: Option<UserPreferences>,
// metadata 仅保留真正开放扩展的 BTreeMap，并文档化允许 key
```

**步骤**：

| 步骤 | 动作 |
|------|------|
| 4.1 | 定义 `AgentAuth`（若不愿依赖完整 AuthContext）并在 `build_agent_request` 填充 |
| 4.2 | 全库替换 `auth_context.get("owner_user_id")…` 为字段访问 |
| 4.3 | preferences 改为 contracts 类型 |
| 4.4 | 序列化需求（eval/红队）用显式 DTO，`AgentRequest` 本身可 `Serialize` 派生时 skip runtime-only 字段（已有 cancellation_token 模式） |

### 8.3 验收

- [ ] 生产路径仅保留 **一种** Chat 编排入口文档化说明
- [ ] `rg "auth_context: serde_json" avrag-rs` 为 0
- [ ] 删除幽灵 ports 后 `cargo test -p app` 仍表达真实契约（若有需要，改为测 `ChatContext`）

---

## 9. Wave 5 — 命名收敛与依赖倒置

### 9.1 `notebook_id` → `workspace_id`（契约一刀切）

**错误做法（现状）**：每个 `frontend_next/lib/workspace/client.ts` 函数手写 map。

**正确做法**：

1. **contracts** 对外 JSON 字段统一为产品名 `workspace_id`  
   - 过渡：`#[serde(alias = "notebook_id")]` 读旧写新，或双写一个 minor  
2. 重新 `generate-contracts` / typeshare → `frontend_next/lib/contracts/generated`  
3. 删除 client 内 `mapNotebook` 仪式中 **session/source 级** 重复映射；保留最多 **一处** `mapNotebook` 若 DB 列名仍为 notebook  
4. 后端内部表名/列名可继续 notebook（存储稳定），**API 边界**用 rename 或 view DTO

**验收**：

- [ ] `frontend_next/lib/workspace/client.ts` 中 `notebook_id` 出现次数接近 0（或仅注释说明存储别名）
- [ ] 公开 API OpenAPI/契约示例使用 `workspace_id`

### 9.2 `rag-core` / `llm` 端口化

| 依赖 | 目标 |
|------|------|
| `rag-core` → `avrag-llm` | 改为 `EmbeddingPort` / `RerankPort` trait（可放 `rag-core-ports`），adapter 在 bootstrap |
| `rag-core` → `cache-redis` | 已有 `CachePort` 则 **删掉** 对 `CacheStore` 具体类型的字段 |
| `llm` → `cache-redis` | `EmbeddingClient` 接受 `Arc<dyn CachePort>` 或泛型，默认 no-op |

**验收**：

- [ ] `rag-core/Cargo.toml` 无 `avrag-llm` / `avrag-cache-redis` 直接依赖（测试可用 dev-dep mock）
- [ ] 单测可用内存 fake，无需 Redis/真实 embedding HTTP

### 9.3 分析 handler 并行

`get_notebook_analysis`：对独立 IO（documents / sessions / preferences / notes）使用 `tokio::join!`（access 已部分 join）。

**验收**：无行为变更；延迟在多依赖场景下降（可选基准）。

### 9.4 体积拆分（贴 1k 文件）

| 文件 | 动作 |
|------|------|
| `document_pipeline.rs` (~957) | 拆 `pipeline/{parse,persist_index,profile}.rs`，主文件只编排 |
| `execute.rs` | 随 Wave 2 自然变薄或按 channel 拆模块 |
| `admin-ops-surfaces.tsx` | 按 billing / flags / audit 拆组件文件 |
| `admin-i18n.ts` | 按 surface 拆 messages 或迁 i18n 体系 |

---

## 10. Wave 6 — `app-chat` 再拆（可选，1–2 稳定后）

在 ToolRegistry 与 execute-plan 清零后，按 **编译边界** 拆（延续 ADR-0006 §4）：

| Crate | 内容 | 依赖方向 |
|-------|------|----------|
| `agent-tools` | ToolRegistry、builtin tools | search, rag-core-ports, guardrails… |
| `agent-loop` | ReActLoop、policy、iteration | agent-tools, llm |
| `chat-orchestrator` 或保留 `app-chat` | pipeline、session、streaming、ChatContext | agent-loop, write-core |
| `write-core` | 已有 | 保持 |

**验收**：`app-chat`（或 orchestrator）LOC 降到可扫视；改 tool 不必重编整个 orchestrator 测试矩阵的无关部分。

**非目标**：一次 PR 完成大搬家；禁止与 Wave 1 语义改动混在同一提交。

---

## 11. 前端专项（嵌入 Wave 0/5，非独立史诗）

| 项 | 波次 | 动作 |
|----|------|------|
| 文件门禁纳入 `client.ts` / admin 大文件 | 0 | |
| notebook 映射删除 | 5.1 | |
| `tool-result-card` 去掉 `any` | 5 或穿插 | 使用 contracts `ToolResult` |
| admin surfaces 拆分 | 5.4 | |
| API 错误路径测试保持 | 已有基础 | 回归时不删 |

---

## 12. 明确不做什么（防范围膨胀）

1. **不**重写产品模式集合（Chat / RAG / Search / Write 保持）
2. **不**把 Write 强行塞进 UnifiedAgent（已有意分流）
3. **不**为整改引入长期 feature flag 森林；短分支 + 本地 commit
4. **不**默认 push/PR/GitHub CI 作为进度（solo 本地 trunk）
5. **不**在本方案内处理 `frontend_rust`
6. **不**把「再写一套 ports 空壳」当作完成解耦
7. **不**把 Capability / Skill / Tool **产品三层**合并为一套类型或一个对外名词（ADR-0006 §5a）；执行单点 ≠ 产品词合并

---

## 13. 成功度量（Definition of Done）

### 13.1 结构度量（波次全部完成后）

| 度量 | 基线（2026-07-09） | 目标 |
|------|-------------------|------|
| **工具执行**调度入口 | 多入口（catalog + atomic + loop match + 双写 map） | **1**（`ToolCatalog` + `dispatch_tool`） |
| **产品分层** Capability / Skill / Tool | 混谈 / 文档曾误写「合并为 1」 | **三层保留**（ADR-0006 §5a）；禁止再写「概念数→1」抹平产品词 |
| Execute-plan 产品/内部主路径 | 内栈仍在 | **无**（仅 test harness 可选） |
| `AppState` 业务透传方法 | ~125 | **&lt; 30** 或仅 accessor |
| 生产 ChatService 实现数 | 3（含幽灵） | **1** |
| `AgentRequest.auth` 类型 | `Value` | **强类型** |
| 前端 client `notebook_id` 映射 | 大量 | **契约层统一** |
| 文件体积门禁 | 损坏 | **可用且反映热点** |
| `app-chat` 体量 | ~28k LOC | 拆分后主 crate **显著下降**（Wave 6） |

### 13.2 质量门（每波）

- 定向 `cargo test` / `pnpm test` 绿
- 无新增「第二套」同名抽象
- 净删行优先：wave 合并后 `git diff --stat` 以删除/收缩为主叙事

---

## 14. 建议提交 / 波次检查清单

### Wave 0

- [ ] 重写 `scripts/check_file_size_limits.sh`
- [ ] （可选）在 `AGENTS.md` 或 SOLO 文档链到本方案

### Wave 1

- [ ] ToolRegistry + ToolExec
- [ ] Loop 单点 dispatch
- [ ] 删除/收缩 atomic_tools 与假 match 分支
- [ ] PromptRegistry 降级为 loader

### Wave 2

- [ ] Prompt 去掉 ExecutePlan legacy
- [ ] Runtime 公共 API 去 plan
- [ ] contracts deprecated → 删除计划日期写死（建议 **≤ 2026-09-30**，与 inventory 一致）
- [ ] 更新 `0006-execute-plan-removal-inventory.md` 勾选

### Wave 3

- [ ] Context accessor
- [ ] transport-http 替换
- [ ] 删除 `*_delegates.rs` 透传

### Wave 4

- [ ] 删除幽灵 ports/ChatService（D1）
- [ ] AgentRequest 强类型

### Wave 5

- [ ] workspace 契约命名
- [ ] rag-core/llm 端口
- [ ] 大文件拆分与 analysis join

### Wave 6（可选）

- [ ] agent-loop / agent-tools crate 边界
- [ ] workspace members 与依赖方向文档化

---

## 15. 文档与索引关系

| 文档 | 关系 |
|------|------|
| 本文件 | **执行方案（唯一主清单）** |
| `docs/adr/0006-product-architecture-decisions-post-tn.md` | 产品/架构终裁；本方案落实 §4/§5 |
| `docs/adr/0006-execute-plan-removal-inventory.md` | Wave 2 明细 inventory；完成后更新勾选 |
| `docs/HEALTH_OPTIMIZATION_HANDOFF_2026-06-11.md` | 上一代健康整改；本方案为续篇 |
| `docs/CODEBASE_HEALTH_DASHBOARD_2026-06-11.md` | 历史评分；不在此重算分数 |
| `docs/t13-app-split-inventory.md` | app 拆分基线；Wave 3/6 时对照过时条目 |
| `docs/engineering/SOLO_DISCIPLINE.md` | 本地 trunk / 测试节奏 |

---

## 16. 首周可执行切片（solo 友好）

若只能先做 **3 个本地 commit**，按杠杆排序：

1. **Wave 0**：修 file size gate（半日）  
2. **Wave 1.1–1.3**：ToolRegistry 吃掉 builtin + loop 单点 dispatch（主切片）  
3. **Wave 2.2**：prompt 删除 ExecutePlan legacy 分支（立刻减双栈）

三刀落地后，复审应看到：**工具概念下降**、**plan 双栈开口堵住**、**门禁复活**——再开 Wave 3/4 不迟。

---

## 17. 变更记录

| 日期 | 说明 |
|------|------|
| 2026-07-09 | 初版：基于 TN 全库复审（排除 frontend_rust）落成可执行波次方案 |
| 2026-07-09 | **产品补裁**：Capability/Skill/Tool **三层保留**（ADR-0006 §5a）；作废「三注册表合并为 1」目标；执行单点 ≠ 产品词合并 |
| 2026-07-09 | **落地开始**：Wave 0 门禁重写；Wave 1 新增 `agents/tool_registry.rs`，loop 单点 dispatch；Wave 2.2 prompt 拒收 ExecutePlan legacy |
| 2026-07-09 | **Wave 2 完成**：execute_plan `cfg(test)`；policy crate-private；删 prompt ExecutePlan helpers；contracts deprecated；citations 改 RetrievalBundle |
| 2026-07-09 | **DTO 物理删除**：去掉 ExecutePlan* 类型、from_tool_calls、execute_plan_policy、multi-channel harness；`build_rag_chat_response_from_bundle` 改用 RetrievalBundle |
| 2026-07-09 | **Wave 3 启动并完成 chat 面**：`state.chat()` 访问器；删除 chat/citation 纯透传；transport-http 改调 Context |
| 2026-07-09 | **Wave 3 续**：`BoundDocuments`/`BoundAdmin`；`state.docs()`/`admin_api()`；删 notebooks/documents/url_imports 透传 |
| 2026-07-09 | **Wave 3 续**：`BoundShare`/`BoundPrefs`；`state.share()`/`prefs()`；删 share_delegates/preferences/admin_delegates |
