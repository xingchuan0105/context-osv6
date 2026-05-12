# Codebase Gap Review (2026-05-10)

> 状态：2026-05-10 review，**2026-05-11 修订**。基于代码现状对照设计文档，识别功能与架构缺口。
> **修订说明**：P0-1 与 P0-2 已在 2026-05-11 前修复，详见下方 ✅ 标记。当前剩余缺口见 P1/P2/P3。
> 权威依据：当冲突出现时以最新日期文档为准 → 优先级 `2026-05-11` > `2026-05-09` > `2026-04-27` > `2026-04-26`。
> 取代 / 补充 `2026-04-27-codebase-gap-review.md`。

## 0. 总览

`2026-05-09-runtime-tool-dispatch-architecture.md` 描述的五阶段迁移（Phase 1–5）在**类型 / 后端 / API 端点 / production chat 路径**层面已全部完成。RagAgent 已全面走通 `parse_rag_plan_decision → execute_tools → synthesize_from_tool_results` 新路径，SSE 取消链路也已全通。当前剩余缺口集中在安全（凭证隔离、输出硬化）、测试覆盖（graph 跨租户 e2e）、UX（前端取消）与质量优化（graph rerank、URL 摄取）。

| Phase | 设计描述 | 代码现状 | 落地度 |
|-------|----------|----------|--------|
| 1. 类型与适配层 | `tool_call.rs`、ExecutePlan ↔ ToolCall 适配 | `crates/common/src/tool_call.rs` 584 行齐备；`rag_prompts.rs::execute_plan_request_to_tool_calls` 适配器存在 | ✅ 完成 |
| 2. Planner prompt 重写 | tool catalog 格式 + `next_step` | `prompts/rag_plan_system.txt` 已对齐 v1 catalog | ✅ 完成 |
| 3. Runtime 拆分 + Synthesizer 升级 | 6 tool pipelines + dispatcher + `synthesize_from_tool_results` | `crates/rag-core/src/runtime/tools/` 6 个文件 + `dispatch`/`dispatch_all` + `synthesizer.rs::synthesize_from_tool_results` 全部存在；RagAgent 已 production 接入 | ✅ 完成 |
| 4. TOC 索引 + `index_lookup` | `document_toc` 表 + 工具 pipeline | migration 0031、worker `replace_document_toc`、`doc_summary`/`doc_metadata` 读 TOC、`runtime/tools/index_lookup.rs` 走 `pg_repo.get_chunks_by_ids` 直取 | ✅ 完成 |
| 5. 外部 API 端点 | `POST /v1/runtime/execute` | `runtime_execute_handler` 已 wire 在 transport-http | ✅ 完成 |

---

## P0 — 关键正确性 / 成本风险

> **2026-05-11 修订**：本节原有两项 P0（P0-1 RagAgent 工具调用范式、P0-2 SSE 取消链路）均已修复。保留标题作为历史锚点，下方直接标注完成状态。

### ✅ P0-1：RagAgent 工具调用范式 — 已修复

`rag_agent.rs` 已全面走通新路径：`call_planner` → `parse_rag_plan_decision` → `execute_tools(dispatch_all)` → `synthesize_stream_text_from_tool_results`。`synthesize_from_tool_results` / `synthesize_stream_text_from_tool_results` 已有 production 调用方（`rag_agent.rs:573`）。

### ✅ P0-2：SSE 流式取消链路 — 已修复

`AgentRequest` 已携带 `cancellation_token` 字段。`ChatAgent`（`chat_agent.rs:79`）、`WebSearchAgent`（`web_search_agent.rs:73`）、`RagAgent`（`rag_agent.rs:565`）均将 token 透传到底层 `complete_stream` / `synthesize_stream_text_from_tool_results`。

---

## P1 — 安全与架构债务

### P1-1：Guard Pipeline 缺语义层

**事实**

`crates/guardrails/src/input/mod.rs:6` 自带 TODO：*"Evaluate LLM-based semantic guard for production hardening"*。当前 `InputGuardPipeline` 三件套：

| Guard | 类型 |
|-------|------|
| `PromptInjectionGuard` | 关键字正则 |
| `PrivilegeEscalationGuard` | 关键字正则 |
| `ScopeGuard` | 范围/路径正则 |

`OutputGuardPipeline`：`citation_provability` + `pii_scrubber` + `harmful_content`（同样基于规则）。

**冲突点**：2026-05-06 安全方案中规划的"分层防御"包含语义级 LLM Guard，**未实现**。

**影响**

- 高级提示注入（混淆、多语言、间接注入）规则层不可能拦完，缺乏语义后盾。
- 与 RagAgent 走新工具调用范式后的 `next_step` 自反规划耦合：planner 输出的 `ToolCall` 若被注入污染，缺一道兜底。

**修复方向**

1. 新增 `LlmSemanticInputGuard`（独立轻量分类器或同模型小模式），异步 / 软超时模式接入 pipeline。
2. 输出端增加 *Canary token* 校验（见 P1-3）。

**优先级**：P1

---

### P1-2：凭证隔离仅完成一半

**事实**

- ✅ `AppState`（`crates/app/src/lib_impl/state_types.rs`）已无 `AppConfig` 句柄，原始密钥不直接暴露给 chat 路径。
- ❌ `MemoryState` 仍持有 `api_keys`（per 早期 review；最新代码也未拆分）。
- ❌ 所有 LLM client 仍由 `AppConfig` 在启动时注入；运行期 rotate / 多租户独立 key 场景没有 hook。

**冲突点**：2026-05-06 安全方案 §1 "Credential Isolation" 要求凭证只在受限运行域可见且支持热轮换。

**修复方向**

1. 抽出 `KeyVault` trait（`Box<dyn KeyVault>`），`AppConfig` 启动期 build vault；LLM client 获取 key 时走 vault 调用。
2. `MemoryState::api_keys` 迁出，按租户作用域索引。

**优先级**：P1

---

### P1-3：输出硬化未完工

**事实**

- 无 `Canary` 字符串注入与 SSE 出口扫描机制（grep `canary|Canary|sysvec|SysVec` 全 workspace 0 命中）。
- 无 `SysVec` 系统 prompt 编码层（防 prompt 反射）。
- `OutputGuardPipeline` 只覆盖 PII + harmful + citation provability。

**冲突点**：2026-05-06 §3 "Output Sanitization & Canary" 要求注入随机 canary、检测回流。

**修复方向**

1. `system_prompt_builder` 注入随机 canary token，`OutputGuard` 扫描泄漏。
2. `SysVec` 编码：将敏感系统指令转向量片或非自然语言形式以降低反射概率（spec §2 提议）。

**优先级**：P1

---

### P1-4：`graph_retrieval` 工具的多租户校验路径需落 e2e 测试

**事实**

`crates/rag-core/src/runtime/tools/graph.rs` 已传 `tenant_org_id`、`hop_limit`、`fan_out_limit`、`relation_limit`、`supporting_chunk_limit`，`storage-milvus/src/schema.rs::doc_filter` 在 `doc_id` 列表为空时回退为 `doc_id == 'none'`（fail-closed）。

**缺口**：尚无显式的"跨租户访问应被拒绝"集成测试覆盖 graph 工具路径（仅在 storage 层有部分单测）。

**修复方向**：在 `crates/rag-core/tests/` 新增 graph tool 跨租户拒绝用例。

**优先级**：P1（修复风险低、回归代价大）

---

## P2 — 可观测性 / UX

### P2-1：前端没有 SSE 取消能力

**事实**

`crates/web-ui/src/sse/mod.rs::use_chat_stream` 用 `EventSource` 起 SSE，闭包用 `on_message.forget()`，且 hook 的返回签名 `(ReadSignal<bool>, ReadSignal<String>, ReadSignal<Vec<Citation>>, Callback<String>)` 没有暴露 close handle。即便 P0-2 修好后端取消，前端也无入口触发。

**修复方向**

1. 把 `EventSource` 包装成 `RwSignal<Option<EventSource>>` 并暴露 `cancel: Callback<()>`；
2. UI 添加"取消"按钮；
3. 后端约定取消事件（如客户端关闭连接 → 触发 token cancel）。

**优先级**：P2

---

### P2-2：可观测性

**事实**

- OpenTelemetry tracing instrumentation 在 `RagAgent::run`、`UnifiedAgentService::run`、`graphflow_tasks_core` 已部分加 `#[tracing::instrument]`，但 tool pipelines（`runtime/tools/*.rs`）没有 span 规范化命名。
- 无 prompt registry / CRUD：所有 prompt 走 `prompts/*.tmpl` 文件 + `include_str!`，运行期无法 A/B 切换或灰度（影响 2026-05-06 安全方案 §2 中"prompt 灰度回滚"的可行性）。

**修复方向**

1. tool pipelines 统一 `tracing::instrument(name = "tool.dense_retrieval", ...)`。
2. prompt 落 DB（`prompts(id, name, version, body, status)`）+ 管理 API + 启动时缓存。

**优先级**：P2

---

## 文档卫生

| 文件 | 问题 | 处理 |
|------|------|------|
| `2026-04-27-codebase-gap-review.md` | 内文实际更新到 2026-05-09 时间戳；P0 列表已过期 | 加 banner *"已被 2026-05-10-codebase-gap-review.md 取代"* 或归档 |
| `2026-04-26-current-product-rag-architecture.md` | §1 "RagAgent 待接线"、§16 "GraphFlow 退场" 已过时 | ✅ 2026-05-11 已更新 |
| `2026-05-10-codebase-gap-review.md` | P0-1 / P0-2 基于旧代码 | ✅ 2026-05-11 已修订，标记为完成 |
| `prompts/rag_plan_system.txt` | 内容已对齐 v1 catalog | 已生效：RagAgent 生产路径已走 tool-call 范式 |

---

## 修复优先级总表

| ID | 主题 | 优先级 | 状态 | 预估 | 依赖 |
|----|------|--------|------|------|------|
| ~~P0-1~~ | ~~RagAgent 切换到 tool-call 范式~~ | ~~P0~~ | ✅ 已完成 | — | — |
| ~~P0-2~~ | ~~SSE 取消链路打通~~ | ~~P0~~ | ✅ 已完成 | — | — |
| P1-1 | Semantic LLM Guard | P1 | ❌ 不修复（沙盒环境，规则层已足够） | — | — |
| P1-2 | 凭证隔离收尾 | P1 | 🔧 待修复 | 2 d | `KeyVault` trait |
| P1-3 | Canary / SysVec 输出硬化 | P1 | 🔧 待修复 | 3 d | OutputGuard 扩展点 |
| P1-4 | graph 跨租户 e2e | P1 | 🔧 待修复 | 1 d | 现有 tools/graph |
| P2-1 | 前端 SSE 取消能力 | P2 | 🔧 待修复 | 1 d | — |
| P2-2 | OTel + prompt registry | P2 | ❌ 不修复（等上线，prompt 不频繁更新） | — | — |
| P3-1 | Semantic memory vectors | P3 | ❌ 不修复（长期画像全存 md 文档） | — | — |
| P3-2 | Graph relation rerank | P3 | 🔧 待修复 | — | — |
| P3-3 | Web URL 摄取 | P3 | 🔧 待修复 | — | — |

---

> 审阅 checklist（2026-05-11 更新）：
> - [x] P0-1 RagAgent 工具调用范式 — ✅ 已完成
> - [x] P0-2 SSE 取消链路 — ✅ 已完成
> - [x] P1-1 Guard 语义层 — ❌ 明确不修复（沙盒环境）
> - [ ] P1-2 凭证隔离收尾 — 🔧 待推进
> - [ ] P1-3 Canary / SysVec — 🔧 待推进
> - [ ] P1-4 graph 跨租户 e2e — 🔧 待推进
> - [ ] P2-1 前端 SSE 取消 — 🔧 待推进
> - [x] P2-2 OTel + prompt registry — ❌ 明确不修复（等上线）
> - [x] P3-1 Semantic memory vectors — ❌ 明确不修复（全存 md）
> - [ ] P3-2 Graph relation rerank — 🔧 待推进
> - [ ] P3-3 Web URL 摄取 — 🔧 待推进
