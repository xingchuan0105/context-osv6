# 架构重构开发计划

> 基于 `docs/architecture-review-2026-06.md` 最终决策
> 日期：2026-06-01
> 状态：待执行

---

## 总览

| 步骤 | 内容 | 改动类型 | 预估 LOC | 依赖 | 风险 |
|:----:|------|---------|:--------:|------|:----:|
| 1 | 实现 Evidence Gate | 代码（新增模块） | ~200 | 无 | 低 |
| 2 | RAG：移除独立 Evaluate 状态，合并为 grounded answer | 代码 + prompt | ~400 | 1 | 中 |
| 3 | WebSearch：移除独立 Evaluate 状态，合并为 grounded answer + 2 轮止损 | 代码 + prompt | ~500 | 1, 2 | 中 |
| 4 | focus mode 条件触发（RAG + WebSearch） | 代码（新增模块） | ~250 | 1, 2, 3 | 中 |
| 5 | Chat：自然语言模式提醒 + 格式检测 | prompt | ~50 | 无 | 低 |
| 6 | 人工 E2E 回归验收 | 测试 | — | 全部 | — |

---

## 步骤 1：实现 Evidence Gate

### 1.1 目标

新增 `EvidenceGate` 纯代码门控层，对检索元数据做硬性条件检查，**不调用 LLM**。

### 1.2 新增文件

**`crates/rag-core/src/evidence_gate.rs`**

```rust
pub struct EvidenceGateInput {
    pub chunk_count: usize,
    pub top_score: f32,
    pub score_variance: f32,
    pub context_usage_ratio: f32, // 0.0 - 1.0
    pub doc_metadata_themes: Vec<String>, // 文档主题标签
    pub query_themes: Vec<String>,         // 查询主题关键词
}

pub enum EvidenceGateOutcome {
    Pass,                          // 通过，直接进入 grounded answer
    NeedsFocus,                    // 评分分散/过多，触发 focus mode
    Degrade(DegradeReason),        // 降级
}

pub trait EvidenceGate: Send + Sync {
    fn check(&self, input: &EvidenceGateInput) -> EvidenceGateOutcome;
}

pub struct DefaultEvidenceGate {
    pub min_chunk_count: usize,        // 默认 1
    pub min_top_score: f32,            // 默认 0.3
    pub max_score_variance: f32,       // 默认 0.15（越大越分散）
    pub max_context_usage: f32,        // 默认 0.8
    pub focus_chunk_threshold: usize,  // 默认 20
    pub min_theme_overlap: f32,        // 默认 0.0（不强制）
}
```

### 1.3 验收

- 单元测试覆盖每种 outcome 的触发条件
- 集成测试：检索为空时返回 `Degrade(EvidenceInsufficient)`，不调用 LLM
- 集成测试：召回过多且评分分散时返回 `NeedsFocus`

### 1.4 测试文件

**新增** `crates/rag-core/tests/evidence_gate.rs`（约 80 行，覆盖 6 个判定分支）

---

## 步骤 2：RAG 合并 Evaluate/Answer

### 2.1 目标

移除 `RagStrategy` 独立的 `Evaluate` 状态。检索后直接进入 `Answer`，由 Evidence Gate 决定是否进入 grounded answer 或降级。

### 2.2 改动文件

#### 2.2.1 `crates/app/src/agents/strategy/rag.rs`

- **删除** `step_evaluate` 函数（1492 行文件中第 671-877 行）
- **删除** `evaluate_retrieval_strategy` 函数（993-1051 行）
- **修改** `step_execute`：在完成检索后立即调用 `EvidenceGate::check()`
  - 通过 → 直接进入 `Answer`
  - `NeedsFocus` → 调用 focus mode 压缩后进入 `Answer`
  - `Degrade(reason)` → 调用 `finalize_degrade`
- **修改** `RagContext`：
  - 移除 `current_plan_calls` 中由 evaluator 决定的 replan 部分
  - 保留 `replan_directive` 作为 planner 输入

#### 2.2.2 `crates/app/src/rag_prompts.rs`

- **修改** `build_rag_strategy_evaluation_prompt`：标记为 **deprecated**，新代码不应调用
- **保留** `parse_rag_strategy_evaluation` 以备后用，但不再作为主链路

#### 2.2.3 `prompts/skills/rag-answer/SKILL.md`

- 在 answer 流程中**增加前置评估逻辑**：

```
Before generating the answer, internally assess:
1. Do the retrieved chunks actually address the user's question?
2. If chunks are irrelevant or insufficient, output a fallback answer that:
   - Clearly states "the retrieved documents do not contain sufficient information"
   - Identifies what the documents DO contain (e.g., "they discuss X, Y, Z topics")
   - Offers to answer from general knowledge with explicit disclaimer
3. If chunks are sufficient, answer normally with [[cite:CHUNK_ID]] citations.
4. Include the exact phrase `EVIDENCE_INSUFFICIENT_FALLBACK` in your response
   if you must answer from general knowledge, so the system can record this
   as a degraded path.
```

### 2.3 验收

- `RagContext` 不再使用 `EvalDecision` 枚举（重构后移到 Evidence Gate）
- `step_evaluate` 被删除，状态机变成 `Plan → Execute → Answer`（3 步）
- chunk 只在 `Answer` 阶段被读取一次
- E2E 测试 `rag_empty_document_degrades_gracefully` 仍然通过

### 2.4 状态机迁移

| 旧状态 | 新状态 | 触发条件 |
|--------|--------|---------|
| Plan | Plan | 起始 |
| ExecuteRetrieve | ExecuteRetrieve | Plan 输出后 |
| Evaluate | **删除** | Evidence Gate 取代 |
| Answer | Answer | ExecuteRetrieve 后立即 |
| Replan | Plan（directive 注入） | Evidence Gate 失败但可补救 |

---

## 步骤 3：WebSearch 合并 Evaluate/Answer + 2 轮止损

### 3.1 目标

与 RAG 类似：移除独立 `Evaluate`，合并为 grounded answer。**额外**：硬约束搜索最多 2 轮。

### 3.2 改动文件

#### 3.2.1 `crates/app/src/agents/strategy/search.rs`

- **删除** `step_evaluate` 函数（1896 行文件中第 696-919 行）
- **修改** `step_parallel_search` / `step_single_search`：在搜索完成后调用 `EvidenceGate::check()`
- **修改** `LoopBudget`：
  - 新增 `max_search_rounds: u8` 字段（默认 2）
  - 在 `step_parallel_search` 入口检查 `ctx.budget.current_search_rounds >= max_search_rounds` → 跳过搜索直接进入 `Answer` 聚合
- **修改** `SearchContext`：移除 `current_plan_calls` 中 evaluator 决定的部分

#### 3.2.2 `crates/app/src/agents/strategy/loop_budget.rs`（如不存在则新建）

```rust
pub struct LoopBudget {
    pub current: u8,
    pub max: u8,
    pub current_search_rounds: u8,    // 新增
    pub max_search_rounds: u8,        // 新增，默认 2
}
```

#### 3.2.3 `prompts/skills/web-grounded-answer/SKILL.md`

同 RAG answer skill：在 answer 流程中增加前置评估逻辑，fallback 时输出 `EVIDENCE_INSUFFICIENT_FALLBACK` 标记。

#### 3.2.4 `prompts/skills/web-search-planner/SKILL.md`

在 reference 中增加 budget-aware 规则：

```
Budget awareness:
- This is a 2-round maximum search budget. Plan sub-queries that are
  likely to be sufficient in the first round. Avoid near-duplicate
  queries that would waste the second round.
- If you know the topic is narrow (e.g., a specific company's financials),
  prefer one precise sub-query over multiple broad ones.
```

### 3.3 验收

- `step_evaluate` 删除，状态机变成 `Decompose → ParallelSearch → Aggregate → Answer`（或单轮版本：Search → Aggregate → Answer）
- 搜索轮次硬限制为 2
- E2E 测试 `search_vertical_escalation_state_machine` 仍然通过

---

## 步骤 4：focus mode 条件触发

### 4.1 目标

实现 `FocusMode` 压缩层。当 Evidence Gate 返回 `NeedsFocus` 时，对检索结果做句段级压缩。

### 4.2 新增文件

**`crates/rag-core/src/focus_mode.rs`**

```rust
pub trait FocusMode: Send + Sync {
    async fn compress(
        &self,
        chunks: Vec<RetrievedChunk>,
        query: &str,
        target_count: usize,
    ) -> Result<Vec<RetrievedChunk>, FocusError>;
}

pub struct ScoreBasedFocusMode {
    pub keep_top_n: usize,             // 默认 10
    pub trim_to_chars: usize,          // 默认 500
    pub extract_relevant_sentences: bool, // 默认 true
}

impl FocusMode for ScoreBasedFocusMode {
    // 1. 按 score 排序取 top N
    // 2. 每个 chunk 截断到 top 字符
    // 3. 可选：句段级提取（用 query 的关键词匹配，保留最相关句子）
}
```

### 4.3 改动文件

#### 4.3.1 `crates/app/src/agents/strategy/rag.rs` 和 `search.rs`

在 `step_execute` / `step_parallel_search` 调用 Evidence Gate 后：
- 若返回 `NeedsFocus`，调用 `FocusMode::compress()`，然后进入 `Answer`
- 若返回 `Pass`，直接进入 `Answer`

### 4.4 验收

- 召回 50 条、评分分散时，focus mode 压缩到 10 条
- 召回 5 条、评分集中时，focus mode 不被调用
- 压缩后 grounding 质量不下降（人工评估 5 个场景）

---

## 步骤 5：Chat 提示词优化

### 5.1 目标

不改代码，只改 prompt 增强 Chat 的"导医台"和格式检测能力。

### 5.2 改动文件

#### 5.2.1 `prompts/skills/chat-plan/reference/decision-rules.md`

新增规则：

```markdown
## Mode recommendation (natural language, not structured field)

When the user's request is best served by a different mode, include this
in the `intent` field as natural language. The answer agent will pick
this up and add a brief suggestion to the user.

Examples:
- intent: "用户查询公司上季度营收 — 建议切换到文档搜索（RAG）"
- intent: "用户询问今日 AI 新闻 — 建议切换到网络搜索（WebSearch）"

Do NOT add a new structured field. The intent string carries the hint.
```

#### 5.2.2 `prompts/skills/chat/reference/voice-and-behavior.md`

新增段落：

```markdown
## Format detection (automatic, no confirmation)

- "PPT" / "presentation" / "slide" / "slides" → apply presentation-html
- "HTML page" / "formatted output" / "styled document" → apply html-renderer
- "teach me" / "explain step by step" / "tutorial" → apply step-by-step-tutor

Just apply the format naturally, don't ask the user to confirm.

## Mode awareness (CRITICAL)

- If the user asks about current events / real-time data, and you don't
  have web search active, say: "I don't have live web access right now.
  Would you like me to search the web for this?"
- If the user asks about documents / files / workspace knowledge, and
  no RAG evidence is available, say: "I don't see the relevant documents
  in our current context. Would you like me to search your uploaded files?"
- Do NOT guess or use training data for questions that clearly require
  external retrieval.
```

### 5.3 验收

- Chat 面对"我们公司营收"问题，主动说"建议切换到文档搜索"
- 用户说"做个 PPT"时，answer 自动应用 `presentation-html` 格式
- 用户问"今日 AI 新闻"时，主动说"我没有实时网络访问"

### 5.4 测试

人工 E2E：3 个场景手测即可（不写自动化测试，依赖第五步人工回归）

---

## 步骤 6：人工 E2E 回归验收

### 6.1 目标

以人工 E2E 回归作为最终验收，覆盖典型检索、降级、错误路由场景。

### 6.2 测试场景清单

| # | 场景 | 预期行为 | 自动化测试 |
|:--:|------|---------|:---------:|
| 1 | RAG：相关文档 + 充分召回 | Synthesized，含 citation | ✅ `rag_single_pass_sufficient_state_machine` |
| 2 | RAG：相关文档 + 部分召回 | focus mode 压缩后 Synthesized | ✅ `rag_replan_insufficient_state_machine` |
| 3 | RAG：不相关文档 | Degrade(EVIDENCE_TOPIC_MISMATCH) | ✅ `rag_empty_document_degrades_gracefully` |
| 4 | RAG：损坏 PDF | Degrade(ParserFailed) | ✅ `bad_file` |
| 5 | WebSearch：充分结果 | Synthesized | ✅ `search_single_pass_state_machine` |
| 6 | WebSearch：第一次空 + 第二次成功 | Synthesized，2 轮 | ✅ `search_vertical_escalation_state_machine` |
| 7 | WebSearch：始终空 | Degrade(DEGRADED_BRAVE_EMPTY_RESULT) | ✅ `search_budget_exhaustion_degrades` |
| 8 | Chat：闲聊 | Synthesized，无 mode 提示 | ✅ `chat_simple_conversation_state_machine` |
| 9 | Chat：事实性问题 | 主动推荐 RAG/WebSearch | 🆕 人工 E2E |
| 10 | Chat：口头格式要求 | 自动应用 format skill | 🆕 人工 E2E |
| 11 | 全局：注入攻击 | Content guard 阻断 | ✅ `*_content_guard_redacts_injection` |

### 6.3 验收门槛

- 自动化测试：18 个策略 E2E + 14 个产品 E2E = 32 个用例全部通过
- 人工 E2E：场景 9、10 的人工判断通过
- 无新增的 `DegradeReason` 字符串漂移

---

## 跨步骤依赖图

```
Step 1 (Evidence Gate)
   ↓
Step 2 (RAG 合并) ←─── Step 4 (focus mode)
   ↓
Step 3 (WebSearch 合并)
   ↓
Step 5 (Chat prompt) ── 独立，可并行
   ↓
Step 6 (人工 E2E 验收) ←─── 全部
```

---

## 文件改动清单（汇总）

### 新增

| 路径 | 用途 | 步骤 |
|------|------|:----:|
| `crates/rag-core/src/evidence_gate.rs` | Evidence Gate 模块 | 1 |
| `crates/rag-core/src/focus_mode.rs` | focus mode 压缩 | 4 |
| `crates/rag-core/tests/evidence_gate.rs` | 单元测试 | 1 |
| `crates/rag-core/tests/focus_mode.rs` | 单元测试 | 4 |

### 修改

| 路径 | 改动 | 步骤 |
|------|------|:----:|
| `crates/app/src/agents/strategy/rag.rs` | 删 step_evaluate、改 step_execute | 2 |
| `crates/app/src/agents/strategy/search.rs` | 删 step_evaluate、改 step_parallel_search、2 轮止损 | 3 |
| `crates/app/src/agents/strategy/loop_budget.rs` | 加 max_search_rounds | 3 |
| `crates/app/src/rag_prompts.rs` | 标记 eval_prompt deprecated | 2 |
| `prompts/skills/rag-answer/SKILL.md` | 增加前置评估逻辑 + fallback 标记 | 2 |
| `prompts/skills/web-grounded-answer/SKILL.md` | 同上 | 3 |
| `prompts/skills/web-search-planner/SKILL.md` | budget-aware 规则 | 3 |
| `prompts/skills/chat-plan/reference/decision-rules.md` | 模式推荐（自然语言） | 5 |
| `prompts/skills/chat/reference/voice-and-behavior.md` | 格式检测 + 边界意识 | 5 |

### 测试改动

| 路径 | 改动 | 步骤 |
|------|------|:----:|
| `crates/app/tests/e2e_rag.rs` | 调整断言适配新行为 | 2 |
| `crates/app/tests/e2e_search.rs` | 调整断言 + 验证 2 轮止损 | 3 |
| `crates/app/tests/e2e_chat.rs` | 验证 mode 推荐和格式检测 | 5 |
| `crates/app/tests/product_e2e/` | 验证 EVIDENCE_INSUFFICIENT_FALLBACK 标记传递 | 2, 3 |

---

## 风险与回滚

| 风险 | 缓解措施 | 回滚方案 |
|------|---------|---------|
| Evidence Gate 阈值过严/过松 | 单元测试覆盖 6 个分支 + 集成测试调阈值 | 改 `DefaultEvidenceGate` 字段默认值 |
| 合并 Evaluate/Answer 后 grounding 质量下降 | 人工 E2E 评估 5 个典型场景 | 恢复 `step_evaluate`，Chat-Plan 不变 |
| focus mode 压缩后丢失关键信息 | 不压缩到 < 10 条，保留 evidence_id | 临时关闭 `NeedsFocus` 触发条件 |
| 2 轮搜索止损导致 WebSearch 成功率下降 | 调高 `max_search_rounds`（如果需要） | 改 `LoopBudget::max_search_rounds` 默认值 |
| Chat 模式推荐对用户造成打扰 | prompt 中明确"只在确实需要时" | 删除 voice-and-behavior.md 中 Mode awareness 段 |

---

## 时间预估

| 步骤 | 工作量 | 备注 |
|:----:|:------:|------|
| 1 | 0.5 天 | Evidence Gate 较简单 |
| 2 | 1.5 天 | RAG 是核心，需仔细处理状态机迁移 |
| 3 | 1.5 天 | WebSearch 类似 RAG + 额外止损 |
| 4 | 0.5 天 | focus mode 是新模块 |
| 5 | 0.5 天 | 纯 prompt 改动 |
| 6 | 1 天 | 人工 E2E |
| **合计** | **~5.5 天** | |

---

## 启动执行

确认计划后，按 Step 1 → 2 → 3 → 4 → 5 → 6 顺序执行。

每步完成后：
1. 跑相关单元/集成测试，确认通过
2. 跑对应 E2E 测试套件（不依赖其他步骤的可以先跑）
3. 更新本文档的"实际产出"列
4. 提交 commit

---

## 下一步

- ⬜ 用户确认本计划
- ⬜ 开始执行 Step 1：Evidence Gate
