# Prompt Cache 优化策略

**日期**：2026-07-30  
**状态**：调研完成，待执行 P0  
**数据来源**：`avrag_rs_e2e_smoke` 库（`realistic_corpus_full_eval` E2E，13508 条 billable）

---

## 1. 背景与约束

起因：与 reasonix（Go 写的 cache-first 单机 coding agent）对比，评估本项目的 agent 缓存机制。

**核心结论**：reasonix 的整套 cache 策略（`cold_resume_prune`、`tool_result_snip_ratio`、stable env prefix）建立在"单用户、连续请求命中自己 prefix cache"的前提上。本项目是**多租户 SaaS + 多功能多 key**，这个前提大部分不成立：

- **跨 execute 命中不可靠**：多租户请求交织，provider 端 cache 被 LRU 互相挤出。
- **唯一可靠 cache 窗口**：单次 `execute` 内的串行多轮 ReAct（几秒到几十秒、TTL 内、前缀稳定）。
- **合并 key 无益且有害**：cache 命中的是"逐 token 相同的前缀"，不是"同 key 归属"。不同功能的 prompt 前缀不同（第一字就分叉），合并只让它们共享 LRU 池互相挤出。**现状分 key 是对的。**

---

## 2. 现状数据（E2E smoke，billable）

### 按 feature 的 prompt token 占比 + cache 命中率

| feature | calls | prompt_tok | 占比 | cached_tok | 命中率 |
|---|---|---|---|---|---|
| chat | 8571 | 86.3M | **77.8%** | 21.9M | 25.6% |
| agent_loop | 642 | 23.7M | **21.3%** | **0** | **0.0%** ⚠️ |
| summary | 82 | 787K | 0.7% | 0 | 0% |

### chat 按 stage 拆分（命中率极度不均匀）

| stage | calls | avg prompt | 命中率 |
|---|---|---|---|
| rag | 6893 (79%) | 7.6k | **38.0%** |
| chat | 1500 (17%) | 17.8k | 7.1% |
| search | 312 (4%) | 25.4k | 5.9% |
| agent_loop | 642 | **36.9k** | **0.0%** |

**铁律**：prompt 越长，命中率越低。短小模板化调用（query rewrite/rerank）38%，长调用（完整历史+工具结果）0-7%。

---

## 3. 已完成（commit `1a02e645`）

| 项 | 文件 | 内容 |
|---|---|---|
| **BUG-2** Anthropic 末尾断点 | `llm/protocols/anthropic_messages.rs` | 给滚动后缀（末条消息末 block）加 `cache_control:ephemeral`，让多轮 ReAct 第 2-N 轮命中 prefix cache |
| **BUG-1** cache_creation 计费 | 同上 | `AnthropicUsage` 解析 `cache_creation_input_tokens`，折入 `prompt_tokens` 计费（此前完全漏掉） |
| 工具 | `scripts/cache-diagnostics.sql` | 4 个只读查询：cache 命中率 / execute 轮数 / token 占比 / 按租户命中率 |

---

## 4. 决策矩阵

| # | 优化项 | token 占比 | 当前命中 | 性质 | 决策 |
|---|---|---|---|---|---|
| **P0** | agent_loop `budget_hint` 移出 system | 21.3% | 0% → 预期 5-15% | 根因明确，可避免的 miss | 🔴 **做** |
| **P1** | session_id/request_id 埋点修复 | — | — | 数据质量，阻碍精确分析 | 🟡 **做** |
| P2 | BUG-1b：Anthropic 专属 rate + 1.25× 溢价 | (Anthropic 量) | — | 计费精确性 | ⏸️ 等 Anthropic 真实用户 |
| — | chat stage=rag 短调用 | 6% | 38% | 已近天花板 | 不动 |
| — | chat stage=chat/search 长调用 | 71% | 5-7% | 大部分是内容天然差异，不可改 | 观望 |
| — | ingestion 合并 summary+triplet | 0.7% | — | 数据证明不值得 | ❌ 不做 |
| — | 合并多功能到单 key | — | — | 前缀不同无效，且 LRU 互挤有害 | ❌ 不做 |
| — | GAP-2 drain 前摘要 | — | — | 架构级（sync trait→async），多租户延迟敏感 | ❌ 不做 |
| — | GAP-3 cold_resume_prune | — | — | 多租户下前提不成立，与现有 L1 window 重叠 | ❌ 不做 |

---

## 5. P0：agent_loop `budget_hint` 移出 system（唯一确定动作）

### 根因

`crates/agent-loop/src/react_loop/assembler.rs` 的 `assemble_retrieve`：
```rust
let system_content = format!("{base}\n\n{rendered}\n\n{budget_hint}");
//                                              ^^^^^^^^^^^
//  budget_hint = <loop_budget round="X" tokens_used="Y" tokens_remaining="Z" />
//  ↑ round / tokens_used 每轮递增 → system 每轮都变
```

`budget_hint` 塞在 system prompt 里，其 `round`/`tokens_used` **每轮递增**。后果：第 N 轮的 system 和第 N-1 轮不同，请求从 system 内部就前缀断裂 → execute 内多轮无法命中 prefix cache。

**对比证据**：同类长调用（chat stage=chat/search）好歹有 5-7% 命中，agent_loop 是 0%——这个异常就是 budget_hint 导致的"本该命中却被人为破坏"。

### 动作

把 `budget_hint`（及第一轮注入的 `Retrieval query`）等**每轮变化的内容**移出 system prompt，改注入到每轮的 user message（或 system 末尾的明确动态段）。目标：让 system 的**静态前缀**（`base` + 固定 disclosure）跨轮稳定，使 messages 历史累积的前缀能被 cache 命中。

**注意**：单独移 budget_hint 不够——`base`（capability-rag.md ≈ 915 token）低于 DeepSeek 的 1024 token 最小缓存前缀。需同时确保稳定前缀 ≥ 1024 token（补充有用的静态内容，或前置固定 disclosure）。验证：chat stage=rag 的 38% 命中证明"稳定 system + 历史前缀"机制有效，agent_loop 应能复现。

### 收益预估

agent_loop 占 21.3% prompt token（23.7M）。若命中率从 0% 提到 5-15%（向 chat 长调用看齐），省 1.2M-3.6M token 的全价输入（cache_hit 按 0.02 计）。

### 风险与验证

- **行为变更**：LLM 看到 budget_hint 的位置变了（system → user）。需确认 LLM 仍能正确感知预算约束。
- **验证**：`scripts/test-l1.sh` + RAG eval 回归（synthesis 质量不掉）；重跑 `cache-diagnostics.sql` 确认 agent_loop 命中率从 0% 上升。
- **T5 合规**：行为保持切片，daily verify with L1。

---

## 6. P1：session_id / request_id 埋点修复

### 问题

E2E smoke 库中 chat feature 的 **8705 条记录，session_id 和 request_id 100% 为 NULL**。导致无法分析"execute 内几轮""跨 session 命中"等精细维度。生产 billing 也可能丢这个关联维度。

### 动作

排查 `MeteringContext`（`app-billing`）在 chat/agent 路径的填充，确认 `session_id`/`request_id` 是否被正确传入 `insert_llm_usage_event`。修复后解锁：
- 精确的 execute 轮数分布（哪些 execute 没吃满 cache）
- 跨 session 的 cache 命中归因

### 风险

低。只是多填两个字段，不改计费逻辑。

---

## 7. 不做项的理由（备查）

| 项 | 否决理由 |
|---|---|
| ingestion 合并 summary+triplet | 数据：summary 只占 0.7% prompt token。即使全砍也省 <0.35%。ingestion 是一次性写时成本，非持续。 |
| 合并多功能到单 key | cache 命中看"前缀逐 token 相同"，不看 key 归属。summary/triplet/index 的 system 第一字就不同，合并零命中，且共享 LRU 互挤有害。 |
| cold_resume_prune（reasonix 招牌） | 多租户下跨 execute 的 provider cache 早被别的租户挤出，前提不成立。已有 chatmemory L1 window 做历史裁剪，功能重叠。 |
| drain 前摘要（GAP-2） | `LoopHooks::transform_context` 是同步签名，摘要需异步 LLM 调用 → 架构级破坏（改 trait + 所有实现 + 调用方）。多租户延迟敏感。 |

---

## 8. 执行顺序

1. **P0** budget_hint 移出 system（改 `assembler.rs`，L1 回归）
2. **P1** session/request 埋点（排查 `MeteringContext`）
3. 重跑 `cache-diagnostics.sql`，确认 agent_loop 命中率提升
4. P2 待 Anthropic 有真实用户量级后再评估

---

## 9. 验证查询（复用 `scripts/cache-diagnostics.sql`）

执行后对比 agent_loop feature 的 `cache_hit_pct`：
```sql
-- 查询 A：按 feature/provider/model 的命中率
-- 执行前：agent_loop | deepseek | deepseek-v4-flash | 0.0%
-- 执行后目标：agent_loop 提升至 5%+
```
