# 架构改进实施总结 2026-06-09

> **范围**: `/improve-codebase-architecture` 审核 + 实施  
> **分支**: `feat/pricing-tiers-revamp`  
> **提交数**: 4（不含 graphify-out 清理）  
> **影响文件**: 352（后端 334 + 前端 10 + contracts/docs 8）

---

## 一、Commit 清单

| Commit | 范围 | 文件数 | 核心内容 |
|--------|------|--------|----------|
| `a117db4` | 仓库根 | 397 | 删除 graphify-out 自动生成缓存（噪音清理） |
| `1c89852` | `avrag-rs/` | 334 | **后端架构大重构**：v5 残留清理、RouterPolicy 移除、LoopOptimizer、ModeSchema 术语对齐、prompt 目录重组 |
| `ba05601` | `frontend_next/` | 10 | **前端 Chat Hook 拆分**：God Component → useChatSession + 子组件 + Mapping 清理 |
| `247769b` | `contracts/` + 根 | 8 | Contracts schema 扩展、CONTEXT.md 更新、脚本 |

---

## 二、每项架构改进的详情

### 2.1 v5 状态机残留清理（候选 4）

**删除的符号**：
- `rig_adapter.rs` — 未实现占位符模块
- `LoopBudget.max_search_rounds` / `current_search_rounds` / `tick_search_round()` / `search_rounds_exhausted()`
- `AgentRunResult.state_history` / `StateRecord`
- `AgentEvent::StateTransition` / `StateTransitionType`

**修改文件**：
- `crates/app/src/agents/react_loop.rs`
- `crates/app/src/agents/runtime.rs`
- `crates/app/src/agents/events.rs`
- `crates/app/src/agents/sse_sink.rs`
- `crates/app/src/agents/loop/mod.rs`
- `crates/app/src/agents/mod.rs`

**验证**: `cargo test -p app --lib` 459 passed

---

### 2.2 RouterPolicy 移除（候选 2）

**删除**：
- `crates/app/src/agents/capability/router.rs`（~660 行浅层透传规则引擎）

**修改**：
- `crates/app/src/agents/unified/mod.rs` — 直接基于 `request.kind` 生成数据
- `crates/app/src/agents/runtime.rs` — `routing_decision` 简化为 `Option<String>`
- `crates/app/src/agents/events.rs` — 保留 `AgentEvent::RoutingDecision` telemetry
- 保留 audit 记录和 SSE sink 中的 `RoutingDecision` 事件

**验证**: 470 passed, clippy clean

---

### 2.3 LoopOptimizer 实现（候选 1）

**新建**：
- `crates/app/src/agents/loop/optimizer.rs`
  - `IterationProgress` — 跨迭代 chunk 首次出现追踪
  - `LoopOptimizer` — 参谋模式（不夺权），注入软提示
  - `ContextAdjustment` — `None` / `DuplicateChunksHint` / `BudgetWarning`

**删除**：
- `crates/app/src/agents/evaluator.rs`（漂移的指挥官模型，输出从未被消费）

**修改**：
- `crates/app/src/agents/loop/mod.rs` — tool execution 后接入 optimizer
- `crates/app/src/agents/mod.rs` — 删除 `pub mod evaluator;`

**验证**: 460 passed

---

### 2.4 ModeSchema 术语对齐（候选 3 + 5）

**删除的状态机语义**：
- `StrategySchema.states` / `transitions` / `max_budget`
- `TransitionSchema` struct

**重命名**：
- `StrategySchema` → `ModeSchema`
- `strategy()` → `mode()`
- `list_strategies()` → `list_modes()`
- `strategy_count()` → `mode_count()`
- `strategy_id` → `mode_id`（`RoutingDecision`、`events.rs`、`sse_sink.rs`）
- `api_version: "v5"` → `"v6"`

**修改文件**：
- `crates/app/src/agents/capability/schemas.rs`
- `crates/app/src/agents/capability/api.rs`
- `crates/app/src/agents/capability/registry.rs`
- `crates/app/src/agents/capability/mod.rs`
- `crates/app/src/agents/events.rs`
- `crates/app/src/agents/sse_sink.rs`
- `crates/app/src/agents/unified/mod.rs`
- `crates/app/tests/unified_agent_contract.rs`
- `crates/app/tests/agent_catalog_contract.rs`

**验证**: 459 passed

---

### 2.5 Frontend Mapping 清理（候选 7）

**删除**：
- `lib/workspace/client.ts` 中全部 `RawWorkspace*` 类型
- `lib/workspace/client.ts` 中全部 `mapWorkspace*` 函数（8 个）
- `lib/workspace/stream.ts` 中孤儿 `ChatEvent` 类型

**修改**：
- `WorkspaceChatMessage.answer_blocks` / `citations` → 改为 optional（后端字段并非总是存在）
- API 函数内联字段重命名 + 消费层 `??` 默认值

**验证**: tsc --noEmit clean（chat/workspace 相关）

---

### 2.6 Frontend Chat Hook 拆分（候选 6）

**新建**：
- `frontend_next/hooks/use-chat-session.ts` (~951 行)
  - SSE 流生命周期、typewriter 动画引擎、消息累积、error handling
- `frontend_next/components/workspace/chat-composer.tsx` (~300 行)
  - 输入框、mode menu、resize handle、keyboard 快捷键
- `frontend_next/components/workspace/chat-message-list.tsx` (~1405 行)
  - Markdown 渲染、citation tokenizer、answer blocks、tool results、HTML fallback

**修改**：
- `frontend_next/components/workspace/workspace-chat-pane.tsx`
  - 2514 行 → **174 行**，改为薄壳编排组件

**关键修复**：
- `use-chat-session.ts` 中 `useEffect` 依赖数组从 `[token, locale, sessionId, chatStream, messageHistory, progressTracker]` 收窄为 `[token, locale, sessionId]`，消除无限渲染循环导致的 OOM

**验证**:
- `workspace-chat-pane.test.tsx`: **15/15 passed** (1.27s)
- tsc: chat/workspace clean

---

## 三、Prompt 目录重组（ADR-0007 预备）

本次提交同时完成了 prompt/skills 目录的重组，为 ADR-0007 的簇化披露做准备：

| 新目录 | 内容 |
|--------|------|
| `prompts/atomic-tools/` | 有 schema 的运行时工具（calculator、code_interpreter、dense-retrieval 等） |
| `prompts/clusters/` | 簇定义：codegen、format、memory、search、writing |
| `prompts/orchestrators/` | 三 mode 全局 system prompt（chat-system、rag-system、search-system） |
| `prompts/pipeline/` | Loop 外工具（session-summary、triplet-extraction、user-profile-extraction） |
| `prompts/synthesis/` | Answer 强制 skill（chat、rag-answer、search-answer、grounded-answer） |
| `prompts/templates/` | 用户级模板（synthesizer-user、summary-user 等） |

**退役/删除的 skill**：
- `rag-plan` / `search-plan` / `chat-plan`
- `rag-eval` / `search-eval`
- `rag-memory-mgmt`
- `rag-citation-format` / `url-citation-format`（语义合并进 system orchestrator）

---

## 四、验证矩阵

| 检查项 | 结果 |
|--------|------|
| `cargo test -p app --lib` | **459 passed** |
| `cargo clippy -p app` | 0 errors（33 pre-existing warnings） |
| `workspace-chat-pane.test.tsx` | **15/15 passed** (1.27s) |
| 前端 tsc（chat/workspace 范围） | **clean** |
| 前端全量 vitest | 193 passed；4 失败（dashboard/settings/share/stream，均 pre-existing） |
| `agent_catalog_contract` | `chat_conversation_history_tools_in_catalog` 仍失败（pre-existing） |

---

## 五、Review 文档索引

### 设计方案（实施前撰写，已于 2026-06-13 归档到 `docs/archive/`）

| 路径 | 内容 |
|------|------|
| `docs/archive/v5-state-machine-cleanup-design.md` | 候选 4：v5 残留清理范围 |
| `docs/archive/router-policy-removal-design.md` | 候选 2：RouterPolicy 移除 |
| `docs/archive/loop-optimizer-design.md` | 候选 1：LoopOptimizer 设计方案 |
| `docs/archive/schema-terminology-alignment-design.md` | 候选 3+5：ModeSchema 术语对齐 |
| `docs/archive/frontend-mapping-cleanup-design.md` | 候选 7：Mapping 透传层清理 |
| `docs/archive/frontend-chat-god-component-design.md` | 候选 6：Chat Hook 拆分设计 |

### ADR（决策记录）

| 路径 | 内容 |
|------|------|
| `avrag-rs/docs/adr/0007-react-phased-context-disclosure.md` | ReAct 循环分阶段上下文注入（Per-Iteration Disclosure）— **提议中** |
| `avrag-rs/docs/adr/0008-query-normalization-and-answer-contract.md` | 查询归一化与回答契约 |

### 归档/冲突说明

| 路径 | 内容 |
|------|------|
| `avrag-rs/docs/agents/ARCHIVE-superseded-by-adr-0007.md` | 被 ADR-0007 取代的旧文档索引 |
| `avrag-rs/docs/agents/cds-v1.1.md` | 渐进披露框架 v1.1（已归档参考） |

### 代码入口（Review 起点）

```
# 后端核心改动
avrag-rs/crates/app/src/agents/loop/optimizer.rs          # 新建
avrag-rs/crates/app/src/agents/loop/mod.rs                # LoopOptimizer 接入点
avrag-rs/crates/app/src/agents/capability/schemas.rs      # ModeSchema
avrag-rs/crates/app/src/agents/capability/registry.rs     # strategy->mode 重命名
avrag-rs/crates/app/src/agents/unified/mod.rs             # RouterPolicy 移除后
avrag-rs/crates/app/src/agents/react_loop.rs              # v5 残留清理
avrag-rs/crates/app/src/agents/runtime.rs                 # state_history 删除
avrag-rs/crates/app/src/agents/events.rs                  # StateTransition 删除

# 前端核心改动
frontend_next/hooks/use-chat-session.ts                   # 新建
frontend_next/components/workspace/workspace-chat-pane.tsx # 174 行薄壳
frontend_next/components/workspace/chat-composer.tsx      # 新建
frontend_next/components/workspace/chat-message-list.tsx  # 新建
frontend_next/lib/workspace/client.ts                     # RawWorkspace 清理
frontend_next/lib/workspace/stream.ts                     # ChatEvent 清理
```

---

## 六、已知风险与后续

| 风险 | 状态 |
|------|------|
| `agent_catalog_contract::chat_conversation_history_tools_in_catalog` 失败 | pre-existing，非本次引入 |
| 前端 vitest 4 个失败（dashboard/settings/share/stream） | pre-existing，已逐一核对排除 |
| `avrag-rs/crates/storage-milvus/src/ops/search.rs` clippy 错误（collapsible_if） | pre-existing，未在本次改动范围内 |
| ADR-0007（Per-Iteration Context Assembler）尚未实施 | 当前代码已完成架构清理，为 0007 打下基础 |

---

*Generated: 2026-06-09*  
*Branch: feat/pricing-tiers-revamp*  
*Commits: a117db4 → 1c89852 → ba05601 → 247769b*
