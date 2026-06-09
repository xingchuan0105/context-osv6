# ADR-0008: Query 消解、内部 JSON 答案协议与 ReAct 出口规则

| 项目 | 内容 |
|---|---|
| 状态 | **已采纳**（v0.2） |
| 决策日期 | 2026-06-08 |
| 关联 | ADR-0007 v0.6（RAG codegen 唯一检索入口）、ADR-0006-revised、ADR-0005-revised |
| 背景 | ADR-0007 实现后 `llm_real` 仍暴露两类结构性缺陷：(1) 多轮指代未消解导致 turn2 检索/query 与 turn1 脱节；(2) Synthesis 纯文本输出无法可靠校验 cite，出现 `citations[]` 有值但正文无 `[[cite:…]]`。同时 ReAct 允许 **Content 早停** 且 **无 observation 仍进 Synthesis**，与 RAG/Search 产品契约冲突。 |

---

## 1. 问题陈述

### 1.1 当前 ReAct 出口语义（实现现状）

```
loop:
  LLM → tool_calls → observation → 继续
  LLM → Content      → break        // 任意 mode、任意轮次均可早停
→ 几乎总是 → assemble_synthesis → complete_stream → final_answer
```

| 现象 | 根因 | 产品影响 |
|------|------|----------|
| `multi_turn` turn2 未提 Taleb | 原始 query `"Who wrote the book about it?"` 直接进入检索；`[prior_user_query]` 仅作弱提示，**无服务端强制消解** | 多轮 RAG 退化为单轮 |
| `rag_real` 有 `citations[]` 但正文无 cite | Synthesis 流式纯文本；后端 `filter_citations_by_answer_references` 在**无 marker 时保留全部 citations**（legacy fallback） | 引用不可校验、E2E 失败 |
| iter0 Content 直接结束 | `LlmOutput::Content` → `break`，跳过检索 | 空证据合成、礼貌套话 |
| `memory` 簇靠模型 `skill_request` | 无服务端触发；`conversation_history_load` 未默认进 tool_pool | 历史加载不可靠 |
| `auto_fallback` 仅在 budget/沙箱错误后 | 正常 Content 早停路径不触发 fallback | 简单 RAG 也可能零 observation |
| RAG 误走 `dense_retrieval` native tool | v0.5 漂移：`tool_pool: [dense_retrieval]` | 与 codegen 唯一入口冲突（ADR-0007 §2.2.1 已废止） |

### 1.2 与 ADR-0007 的分工

| 维度 | ADR-0007 | ADR-0008（本文） |
|------|----------|------------------|
| 每轮注入什么 | Per-iteration Context Assembler、skill 簇、tool 按需 | **进入 loop 前** query 是否自足；**离开 loop 时** 是否允许 Synthesis |
| 答案格式 | orchestrator §5 散文 cite 契约 | **Synthesis 内部 JSON** + 服务端渲染为用户可见 prose |
| 历史 | PG `[prior_user_query]` 注入 messages | **强制消解**为 `resolved_query` 并持久化 |
| 框架 | 不引入外部引擎 | **借鉴** pi-agent-core 的 hook/事件/队列**语义**，Rust 内实现 |

> **不引入 Pi 源码或 Node runtime**（见 §5 非目标）。仅吸收其接口设计，降低 `ReActLoop` 继续膨胀为不可维护单文件的风险。

---

## 2. 决策（已采纳）

在 **保留单一 ReActLoop + Synthesis** 前提下，新增三层能力：

1. **Pre-Loop：Query Normalization**（服务端强制，非 pull 式 memory）
2. **Loop Hooks + 出口规则**（借鉴 pi-agent-core，Rust trait）
3. **Synthesis：内部 JSON 答案协议**（可校验）→ 外部 prose 渲染

```
                    ┌─────────────────────────┐
  HTTP request      │  QueryNormalizer (新)    │
  query + session ─►│  → resolved_query       │
                    │  → 写入 PG metadata      │
                    └───────────┬─────────────┘
                                ▼
                    ┌─────────────────────────┐
                    │  ReActLoop               │
                    │  hooks.transform_context   │
                    │  hooks.convert_to_llm      │
                    │  + ADR-0007 assembler      │
                    │  + 出口规则 (§4)           │
                    └───────────┬─────────────┘
                                ▼
                    ┌─────────────────────────┐
                    │  SynthesisPhase          │
                    │  输出 InternalAnswerJson │
                    │  → validate → render     │
                    └───────────┬─────────────┘
                                ▼
                    AgentRunResult.answer (prose)
                    AgentRunResult.citations (校验后)
```

---

## 3. Query Normalization（强制消解）

### 3.1 原则

| 原则 | 说明 |
|------|------|
| **Push，非 Pull** | 不由模型在 loop 内决定是否加载历史；服务端在 **Round 0 之前**判定并执行 |
| **消解产物可持久化** | 派生字段 `resolved_query` 写入 session turn metadata（PG），供审计与后续 turn 复用 |
| **检索用 resolved，展示用 raw** | SDK 代码与**服务端** `auto_fallback` 一律使用 `resolved_query`；**不向 LLM 暴露** `dense_retrieval` schema（ADR-0007 §2.2.1） |
| **触发基于指称槽位，非句法残缺** | 检测代词/指称词/省略主语（anaphora），而非「缺主谓宾」类 NLP 句法分析 |

### 3.2 自足性判定（`QuerySelfContained`）

**输入**：`raw_query`、`prior_user_turns[]`（PG，仅 user 轮，最近 N 条，默认 N=6）

**输出**：`SelfContained | NeedsResolution { slots: Vec<ReferentSlot> }`

**启发式槽位（v0.1，可配置）**：

| 槽位类型 | 触发示例 | 非触发（反例） |
|----------|----------|----------------|
| `Pronoun` | it, this, that, they, 它, 这, 那 | 「什么是反脆弱性？」 |
| `DefiniteWithoutAntecedent` | the book, the author, 这本书 | 「Taleb 写了什么？」 |
| `Ellipsis` | Who wrote it? / 谁写的？ | 完整名词短语作宾语 |
| `Demonstrative` | that concept, 那个概念 | 无 prior turn 时降级为 `SelfContained` |

实现位置：`crates/app/src/agents/loop/query_normalize.rs`（新模块），**纯函数 + 可选轻量 LLM 兜底**（仅 `NeedsResolution` 时一次 `complete`，非 ReAct 轮次）。

### 3.3 消解流程

```
if !self_contained(raw_query, prior_turns):
    context = format_prior_turns(prior_turns)   // 不含 assistant/tool
    resolved = normalize_llm(raw_query, context) // 或规则模板
    persist(session_id, turn_id, { raw_query, resolved_query, slots })
else:
    resolved_query = raw_query
```

**`normalize_llm` 系统指令（摘要）**：

- 将指称替换为 prior turn 中最相关实体（名词短语）
- 不得编造 prior 中不存在的实体
- 输出单行 `resolved_query`；无法消解时输出 `CLARIFY: <question>`

**`AgentRequest` 扩展**：

```rust
pub struct AgentRequest {
    // ...
    pub query: String,              // 用户原始问题（不变）
    pub resolved_query: String,     // 消解后，默认 = query
    pub query_resolution: Option<QueryResolutionMeta>,
}
```

Loop 内凡使用 `request.query` 作检索/fallback 之处改为 `request.resolved_query`（`mod.rs` fallback 段、telemetry、evaluator）。

### 3.4 与 `memory` 簇的关系

| 能力 | ADR-0007 | ADR-0008 |
|------|----------|----------|
| `anaphora-resolution` skill | 模型自选加载 | **降级为细则**；消解已由服务端完成，skill 仅解释边界情况 |
| `conversation_history_load` | 可选 tool | 默认**不依赖**；Normalizer 已读 PG。长历史（>N 轮）可保留为扩展 |

### 3.5 Clarify 短路

若 `normalize_llm` 返回 `CLARIFY:` 前缀：

- **不进入** ReAct 检索循环
- 直接返回澄清问句（`AgentRunResult.answer`，无 Synthesis）
- SSE：`Activity { stage: "query_clarify" }`

---

## 4. ReAct 出口规则与 Pi 风格 Hooks

### 4.1 借鉴 pi-agent-core 的映射（Rust 内实现）

| pi-agent-core 概念 | 本项目映射 | 挂载点 |
|--------------------|------------|--------|
| `transformContext` | `LoopHooks::transform_context(&mut Vec<ChatMessage>, &LoopContext)` | 每次 `complete_with_tools` **之前** |
| `convertToLlm` | `LoopHooks::convert_to_llm(&[ChatMessage]) -> Vec<ChatMessage>` | 过滤 tool trace / 裁剪 ReAct 步 |
| `steer` / `followUp` 队列 | `LoopMessageQueue`（`steering`, `follow_up`） | v0.1 **仅数据结构 + drain 钩子**；用户中途插话 v0.2 |
| `subscribe(AgentEvent)` | 扩展现有 `AgentEventSink` | 见 §4.3 |
| `agent_start` / `turn_start` / `turn_end` | 新增细粒度事件 | 可观测性、Replay |

```rust
pub trait LoopHooks: Send + Sync {
    fn transform_context(&self, messages: &mut Vec<ChatMessage>, ctx: &LoopContext) {}
    fn convert_to_llm(&self, messages: &[ChatMessage]) -> Vec<ChatMessage> {
        messages.to_vec()
    }
}

pub struct LoopContext<'a> {
    pub mode: &'a ModeConfig,
    pub request: &'a AgentRequest,
    pub iteration: u8,
    pub phase: LoopPhase,           // Retrieve | Synthesis
    pub has_retrieval_observation: bool,
}
```

默认实现 `StandardLoopHooks`：封装现有 `MAX_REACT_MESSAGES` drain 逻辑（从 `mod.rs` 内联迁入）。

### 4.2 出口规则（`LoopExitPolicy`）

**定义**：决定 loop `break` 后是否允许进入 Synthesis、是否强制 fallback、是否拒绝空证据答案。

| Mode | 规则 ID | 条件 | 动作 |
|------|---------|------|------|
| **rag** | `RAG_REQUIRE_EVIDENCE` | `break` 时 `!has_retrieval_observation` | 先执行 `auto_fallback`（若 enabled）；仍无 observation → **禁止 Synthesis**，返回 `degraded_no_evidence` 或重试检索轮 |
| **rag** | `RAG_NO_CONTENT_EARLY_STOP` | iter=0 且 `LlmOutput::Content` 且无 observation | **不 break**；注入 system nudge 或强制 fallback |
| **search** | `SEARCH_REQUIRE_EVIDENCE` | 同 RAG，observation = web_search / web_fetch 成功结果 | 同上 |
| **chat** | `CHAT_ALLOW_DIRECT` | `LlmOutput::Content` | 允许早停；可 **跳过 Synthesis**（简单路径） |
| **all** | `BUDGET_EXHAUSTED` | `iteration >= max_iterations` | 现有行为 + fallback；fallback 后仍无证据 → 应用 mode 证据规则 |

**`has_retrieval_observation` 判定**：

```rust
fn has_retrieval_observation(messages: &[ChatMessage], mode: &ModeConfig) -> bool {
    // RAG: <code_execution_result> 含 chunk 证据，或 collected_tool_results 来自
    //       SDK/fallback runtime（非 LLM native tool_call）
    // Search: web_search / web_fetch 成功 observation
}
```

> RAG 的 `dense_retrieval` 仅出现在 **SDK 执行结果** 或 **服务端 auto_fallback** 的 `collected_tool_results` 中，**不**作为 LLM 侧 `evidence_tools` schema。

配置扩展 `modes/*.yaml`：

```yaml
loop_exit:
  require_evidence: true
  allow_content_early_stop: false   # rag/search
evidence_signals:
  rag: [code_execution_result, auto_fallback_runtime]
  search: [web_search, web_fetch]
```

### 4.3 事件扩展（兼容现有 SSE）

在 **不破坏** 现有 `AgentEvent` 变体前提下新增（前端可忽略）：

| 事件 | 时机 |
|------|------|
| `TurnStart { iteration, phase }` | 每轮 LLM 调用前 |
| `TurnEnd { iteration, exit_reason }` | 每轮结束；`exit_reason`: `tool_calls` / `content` / `budget` / `cancelled` |
| `QueryResolved { raw, resolved, slots }` | Normalizer 完成后 |
| `SynthesisContract { schema_version }` | 进入 Synthesis 前 |

`exit_reason` 写入 `ReActIterationRecord` 与 replay checkpoint。

### 4.4 消息队列（v0.1 占位）

```rust
pub struct LoopMessageQueue {
    steering: VecDeque<ChatMessage>,
    follow_up: VecDeque<ChatMessage>,
    steering_mode: QueueDrainMode,  // OneAtATime | All
}
```

v0.1：结构体 + `drain_steering_before_turn()` 空实现；为 SSE 中途用户插话预留，**不阻塞**本 ADR 主路径交付。

---

## 5. 内部 JSON 答案协议（Synthesis Contract）

### 5.1 动机

| 纯文本 Synthesis（现状） | 内部 JSON（本文） |
|--------------------------|-------------------|
| cite 靠模型自觉写 `[[cite:…]]` | `citations[].chunk_id` 与 `answer_text` 分离，**服务端校验** |
| 流式 delta 难以 parse | 流式仍可对 `answer_text` 字段 delta（或 synthesis 非流式 parse） |
| `filter_citations` 无 marker 时全保留 | 无 marker 视为合约违反，**丢弃未引用 citations** |

用户可见层仍为 **散文 + `[[cite:CHUNK_ID]]`**（与 ADR-0007 §2.0.2 一致）；JSON 仅 **Synthesis 轮 LLM 输出 + 服务端内部**，不暴露给前端 SSE 原始 payload。

### 5.2 Schema（`InternalAnswerV1`）

```json
{
  "schema_version": "internal_answer_v1",
  "answer_text": "Antifragility is ... [[cite:abc-123]]",
  "citations": [
    {
      "chunk_id": "abc-123",
      "quote_span": "optional short excerpt",
      "confidence": "high"
    }
  ],
  "coverage": "full",
  "refusal_reason": null
}
```

| 字段 | 约束 |
|------|------|
| `answer_text` | 面向用户的散文；RAG 必须含与 `citations[].chunk_id` 一致的 `[[cite:…]]` |
| `citations[].chunk_id` | 必须来自 **本轮 loop 收集的** `collected_tool_results` |
| `coverage` | `full` \| `partial` \| `none` |
| `refusal_reason` | `coverage=none` 时必填 |

Search mode 平行 schema `InternalSearchAnswerV1`：`citations[].index` 对应 observation 序号，`answer_text` 用 `[[n]]`。

### 5.3 Synthesis 执行流程

```
1. assembler.assemble_synthesis(...)   // ADR-0007 不变
2. system += synthesis_contract_block(mode)  // 注入 JSON schema 说明（≤30 行）
3. llm.complete (非流式优先 v0.1) → 解析 JSON
4. validate_internal_answer(json, collected_tool_results, mode)
5. render_prose(json) → final_answer   // 默认 = answer_text；可加后处理
6. build_citations(json.citations)     // 严格交集，废除 no-marker fallback
7. emit AgentEvent::Citations { ... }
```

**校验失败策略**：

1. 一次 repair `complete`（附带 validation errors）
2. 仍失败 → `degraded` + `Activity { stage: "synthesis_contract_violation" }` + 模板拒答（不编造 cite）

### 5.4 与 `rag-answer` / orchestrator §5 的关系

| 层级 | 职责 |
|------|------|
| orchestrator §5 | 用户可见 cite **符号**权威 |
| `rag-answer` body | 示例、证据等级、partial 措辞 |
| Synthesis contract block | **机器可解析** JSON 字段说明；不重复散文风格细则 |

### 5.5 流式策略（v0.1 / v0.2）

| 版本 | 策略 |
|------|------|
| v0.1 | Synthesis **非流式** `complete`，parse 后一次性 `MessageDelta` 回放（或单段 `Done`） |
| v0.2 | 约束模型先输出 JSON 闭合，再流式 `answer_text`；或 JSON mode / tool_call 承载 |

前端 SSE 协议 **不变**；仅 Synthesis 段 latency 可能略增（可接受，因检索轮已流式 Activity）。

---

## 6. 配置扩展

`crates/app/src/agents/loop/config.rs` 新增：

```yaml
# modes/rag.yaml 摘录
query_normalization:
  enabled: true
  max_prior_turns: 6
  llm_fallback: true

loop_exit:
  require_evidence: true
  allow_content_early_stop: false
  skip_synthesis_on_direct_answer: false
synthesis:
  contract: internal_answer_v1   # rag
  # search: internal_search_answer_v1
  # chat: prose_only             # 无 JSON 合约
```

---

## 7. 实现步骤（建议顺序）

1. **`query_normalize.rs`**：`SelfContained` 启发式 + PG 读取 + `resolved_query` 持久化
2. **`AgentRequest.resolved_query`** 接线；fallback / telemetry 改用 resolved
3. **`LoopHooks` trait** + `StandardLoopHooks`；从 `mod.rs` 迁出 truncate 逻辑
4. **`LoopExitPolicy`** + `has_retrieval_observation`；收紧 RAG/Search Content 早停
5. **`InternalAnswerV1`** 类型 + `validate_internal_answer` + 废除 no-marker citation fallback
6. **`synthesis.rs`** 改为 contract-first；更新 `rag-answer` 顶部增 JSON 输出说明
7. **事件**：`TurnStart` / `TurnEnd` / `QueryResolved` 发射
8. **`LoopMessageQueue` 占位**（空 drain）
9. **测试**：单元（normalize、validate、exit policy）+ `llm_real` 回归

---

## 8. 验收标准

- [x] `multi_turn`：turn2 `"Who wrote the book about it?"` → `resolved_query` 含 antifragility/Taleb；答案 mention Taleb（heuristic + PG `turn_metadata` 读回）
- [x] `rag_real`：`answer` 含 `[[cite:chunk_id]]` 且 `citations[]` 与 marker **严格一致**（`InternalAnswerV1` + 严格 filter）
- [x] RAG iter0 Content 早停：**不再**出现零 observation 进 Synthesis（`LoopExitPolicy.require_evidence`）
- [x] `filter_citations_by_answer_references`：无 marker 时返回 **空**（或触发 repair），不再 legacy 全保留
- [x] Clarify 路径：含歧义指称且 prior 不足时返回澄清问句，不检索（`normalize_query` → `clarify_answer`）
- [x] Chat mode：简单问答可跳过 Synthesis（配置 `skip_synthesis_on_direct_answer: true`）
- [x] SSE：现有前端不 broken；新事件仅增不减；流式 `citations_emitted` 防双发
- [x] mock E2E smoke 全绿；`llm_real` 保留 `#[ignore]` 手动回归门

---

## 9. 非目标

- **不**引入 LangGraph / Rig / DSPy / pi-agent-core 源码或 Node sidecar
- **不**替换 `avrag-llm` 或前端 SSE 协议破坏性变更
- **不**恢复 `session_summary` 压缩注入
- **不**在本 ADR 实现用户中途插话（steering 队列仅预留）
- **不**做 PDF `grounded_spans` / bbox 级引用（后续 ADR）
- **不**将内部 JSON 暴露给终端用户 raw 查看（仅 debug 模式可选）

---

## 10. 风险与缓解

| 风险 | 缓解 |
|------|------|
| Normalizer LLM 额外延迟 | 仅 `NeedsResolution` 触发；规则命中率高时零 LLM |
| Synthesis 非流式体验变差 | v0.1 接受；检索阶段 Activity 已提供进度；v0.2 流式 JSON |
| 过严出口规则误杀 Chat | `loop_exit` 按 mode 配置；Chat 保持宽松 |
| JSON parse 失败 | repair 一次 + 模板降级；telemetry 记 `synthesis_contract_violation` |

---

## 11. 文档与代码同步

| 路径 | 动作 |
|------|------|
| `docs/adr/0007-react-phased-context-disclosure.md` | v0.6 RAG codegen 唯一入口；§4 测试映射 cite/multi_turn 靠 ADR-0008 |
| `modes/rag.yaml` | `tool_pool: []`；`auto_fallback.tool_id: dense_retrieval` |
| `prompts/skills/rag-system/SKILL.md` | 移除 dense native tool 指引 |
| `prompts/skills/rag-codegen-guide/SKILL.md` | 简单检索亦走 SDK |
| `docs/e2e-gates.md` | 更新 cite 断言说明（严格 marker） |
| `prompts/skills/anaphora-resolution/SKILL.md` | 改为「服务端已消解时的边界说明」 |
| `crates/app/src/agents/unified/helpers.rs` | 移除 no-marker 全保留 fallback（实现本 ADR 时） |
| `crates/app/src/agents/loop/mod.rs` | 出口规则 + hooks 接线 |

---

## 12. 参考

- [ADR-0007: ReAct 循环分阶段上下文注入](0007-react-phased-context-disclosure.md)
- [ADR-0006-unified-agent-loop-revised.md](0006-unified-agent-loop-revised.md)
- [pi-agent-core Agent / hooks 概念](https://badlogic-pi-mono.mintlify.app/agent/overview)（设计参考，非依赖）
- `crates/app/tests/product_e2e/llm_real/{multi_turn,rag_real}.rs`
- `crates/app/src/agents/loop/mod.rs` — Content 早停与 Synthesis 无条件入口（现状）
