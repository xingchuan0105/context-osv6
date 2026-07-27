# LoopOptimizer 设计方案

> ⚠️ **ARCHIVED 2026-06-13** — 本文档已实现并归档。
> 实施位置：`avrag-rs/crates/app-chat/src/agents/loop/optimizer.rs`。
> 现行交付记录：`docs/agents/ARCHITECTURE-REVIEW-2026-06-09-SUMMARY.md`（§2.3）。
> 保留供历史追溯（决策动机 / 风险矩阵 / chunk_id 提取 helper 设计）。

> Status: **Implemented** (2026-06-09, commit `1c89852`)
> Author: 架构审核讨论（候选 1：Evaluator 漂移）  
> Date: 2026-06-09  
> Related: `evaluator.rs` 废弃计划、ADR-0006 (ReActLoop 迁移)

---

## 1. 背景与动机

### 1.1 问题陈述

`ReActLoop`（`loop/mod.rs`）当前采用**信使模型**：LLM 是最终决策者，代码负责组装上下文、执行 tool、返回结果。每轮迭代后，循环仅做硬拦截（`should_block_content_early_stop`）和 budget 检查，**不做任何跨轮信号分析**。

与此并存的是 `evaluator.rs`——一个基于信号的"指挥官"模块，返回 `EvalAdvice`（`Replan`、`BroadenQuery`、`EscalateToSearch` 等）。但该模块的决策输出**从未被主循环消费**，属于架构漂移（设计文档 §4.2-4.4 与实际实现脱节）。

### 1.2 产品决策

经讨论确认：
- **不强制切换模式**（如 `EscalateToSearch`）——保留 LLM 自主权；
- **不做质量趋势分析**——rerank/BM25/triplets 评分体系不统一，代码层做趋势是灾难；
- **不做进展停滞检测**——ReActLoop 的 reasoning 能力足以自我反思；
- **需要 budget 预警**——倒逼 agent 在最后一轮用力思考；
- **需要重复 chunk 检测**——节省 token，避免重复证据多次塞入上下文。

### 1.3 核心洞察

> 不给 LLM 下命令，但给 LLM 更好的情报。

`LoopOptimizer` 不夺权（不是指挥官），而是通过**向 messages 注入 system/user 提示**，把"跨轮压缩信息"提供给 LLM，由 LLM 自主决定是否采纳。这是信使模型下"主动优化门控"的唯一正确姿势。

---

## 2. 设计原则

| 原则 | 说明 |
|------|------|
| **不夺权** | 所有建议以自然语言提示注入，LLM 保留最终决策权。 |
| **省 token** | 不传输 chunk 原文，只传 chunk_id；不做冗余信号计算。 |
| **纯函数** | `LoopOptimizer` 是无状态的纯函数模块，输入输出完全可单元测试。 |
| **分层防守** | L1 硬拦截（`exit_policy`）不变；L2 软引导（`LoopOptimizer`）新增；L3 LLM 自治不变。 |

---

## 3. 模块定义

### 3.1 位置

```
avrag-rs/crates/app/src/agents/loop/optimizer.rs
```

### 3.2 核心类型

```rust
/// 跨迭代状态。由 ReActLoop 持有，每轮 tool execution 后更新。
/// 只追踪"省 token"的信号——不存储 chunk 原文，不存储评分。
pub struct IterationProgress {
    /// chunk_id -> 首次出现的迭代轮次（0-based）
    chunk_first_seen: HashMap<String, u8>,
    /// 当前迭代序号
    current_iteration: u8,
}

/// 参谋建议。LoopOptimizer 的唯一输出接口。
pub enum ContextAdjustment {
    /// 无需干预
    None,
    /// 检测到重复 chunk：只传 chunk_id 列表和首次出现轮次，不传原文。
    /// 让 LLM 自己决定是忽略、换查询还是换 tool。
    DuplicateChunksHint {
        chunk_ids: Vec<String>,
        first_seen_at: Vec<u8>,
    },
    /// Budget 预警：告知这是倒数第 N 轮，倒逼用力思考。
    BudgetWarning { remaining: u8, max: u8 },
}
```

### 3.3 公共接口

```rust
impl LoopOptimizer {
    /// 根据当前迭代状态、本轮 chunk_id 列表、budget 余量，生成上下文调整建议。
    ///
    /// # 参数
    /// - `progress`: 跨迭代累积状态（由 ReActLoop 持有并维护）
    /// - `current_chunk_ids`: 本轮 tool execution 返回的所有 chunk_id（去重后）
    /// - `remaining_iterations`: budget 剩余轮数（`max_iterations - current_iteration`）
    /// - `max_iterations`: budget 上限
    ///
    /// # 返回
    /// `ContextAdjustment`——可能为 `None`，表示无需干预。
    pub fn advise(
        &self,
        progress: &IterationProgress,
        current_chunk_ids: &[String],
        remaining_iterations: u8,
        max_iterations: u8,
    ) -> ContextAdjustment;
}

impl IterationProgress {
    pub fn new() -> Self;

    /// 记录本轮迭代的 chunk_id。内部更新 `chunk_first_seen` 映射（仅记录首次出现）。
    pub fn record_iteration(&mut self, iteration: u8, chunk_ids: &[String]);
}
```

---

## 4. 决策规则

### 4.1 规则 1：重复 chunk 检测（优先级高）

**触发条件**：本轮返回的 chunk_id 中，存在在**前序迭代**中已出现过的 id。

**输出**：
```rust
ContextAdjustment::DuplicateChunksHint {
    chunk_ids: vec!["c2".into(), "c5".into()],
    first_seen_at: vec![0, 2],  // c2 首次出现在第 0 轮，c5 首次出现在第 2 轮
}
```

**注入提示**（system 角色，仅供参考）：
```
[系统提示] 本轮检索返回的 chunk 中，以下 ID 在前序迭代中已出现过：c2（第1轮）、c5（第3轮）。
若你认为这些 chunk 已足够支撑回答，可直接进入总结；
若需要补充新证据，建议尝试不同查询或工具。
```

**措辞约束**：
- 使用问句和选项（"若你认为...可直接..."），不用祈使句；
- 不明确指定 tool 名，让 LLM 自主选择；
- 不塞入 chunk 原文。

### 4.2 规则 2：Budget 预警（优先级中）

**触发条件**：`remaining_iterations == 1`（最后一轮）。

**输出**：
```rust
ContextAdjustment::BudgetWarning {
    remaining: 1,
    max: 4,
}
```

**注入提示**（system 角色，紧迫但不下命令）：
```
[系统提示] 这是最后一轮迭代机会（共4轮）。请评估当前证据是否充分：
若已足够，优先给出完整回答；若仍不足，本轮请选择最高置信度的检索策略。
```

**为什么只在 remaining == 1 触发**：
- remaining > 1 时，LLM 还有调整空间，预警反而增加 noise；
- remaining == 1 时，这是唯一一次"倒逼用力思考"的机会。

### 4.3 规则互斥

若同时满足两条规则（最后一轮且出现重复 chunk），**只触发重复 chunk 提示**——因为重复 chunk 的信息密度更高，且 budget 预警的语义（"这是最后一轮"）已隐含在迭代上下文中。

---

## 5. 接入方案

### 5.1 接入位置

`ReActLoop::run()` 中，每次 **native tool call 执行完成后**、`iteration += 1` 之前：

```rust
// 当前代码位置：loop/mod.rs，tool execution 后（约第 414 行附近）

// 1. 从本轮 collected_tool_results 提取 chunk_ids
let current_chunk_ids = extract_chunk_ids(&collected_tool_results);

// 2. 更新 progress
progress.record_iteration(iteration, &current_chunk_ids);

// 3. 计算 budget 余量（直接算，不依赖 LoopBudget 新增方法）
let remaining = max_iterations.saturating_sub(iteration + 1);

// 4. 调用参谋
let adjustment = optimizer.advise(&progress, &current_chunk_ids, remaining, max_iterations);

// 5. 应用调整（只注入提示，不修改任何运行状态）
match adjustment {
    ContextAdjustment::DuplicateChunksHint { chunk_ids, first_seen_at } => {
        let hint = build_duplicate_hint(&chunk_ids, &first_seen_at);
        messages.push(ChatMessage::system(hint));
    }
    ContextAdjustment::BudgetWarning { remaining, max } => {
        let hint = build_budget_warning(remaining, max);
        messages.push(ChatMessage::system(hint));
    }
    ContextAdjustment::None => {}
}

iteration += 1;
```

### 5.2 chunk_id 提取 helper

需新增 `extract_chunk_ids(results: &[ToolResult]) -> Vec<String>`，处理以下 tool 的返回格式：

| Tool | 数据路径 |
|------|----------|
| `dense_retrieval` | `data["chunks"][*]["chunk_id"]` |
| `lexical_retrieval` | `data["chunks"][*]["chunk_id"]` |
| `graph_retrieval` | `data["chunks"][*]["chunk_id"]`（待确认，若 graph 无 chunk_id 则跳过） |

提取时去重、过滤空值、按出现顺序保留。

### 5.3 对 code execution 路径的处理

CodeBlocks 路径（`LlmOutput::CodeBlocks`）不调用 `LoopOptimizer`。原因：
- code interpreter 的观察结果不是结构化 chunk，无法提取 `chunk_id`；
- sandbox 错误链已有自己的退出逻辑（连续 2 次错误 break）。

---

## 6. 与现有模块的关系

### 6.1 分层防守图

```
┌─────────────────────────────────────────────────────────────┐
│  L3: LLM 自治                                                │
│  LLM 基于完整上下文（含 LoopOptimizer 注入的提示）决定下一步   │
├─────────────────────────────────────────────────────────────┤
│  L2: LoopOptimizer（新增）                                   │
│  软引导：重复 chunk 提示、budget 预警                         │
├─────────────────────────────────────────────────────────────┤
│  L1: exit_policy（不变）                                     │
│  硬拦截：should_block_content_early_stop                     │
│         decide_synthesis_gate / post_fallback_gate           │
└─────────────────────────────────────────────────────────────┘
```

### 6.2 evaluator.rs 废弃计划

`LoopOptimizer` 上线后，`evaluator.rs` 彻底废弃。废弃内容：

| 废弃项 | 替代方案 | 说明 |
|--------|----------|------|
| `EvaluationSignals` | 删除 | 质量趋势分析不再做 |
| `EvalAdvice` | `ContextAdjustment` | 从"强制行为"降级为"提示注入" |
| `AccumulatedRagResults` | 部分迁移 | `chunk_first_seen` 接替去重追踪；评分/accumulator 逻辑删除 |
| `evaluate_rag_iteration` | 删除 | 指挥官模型废弃 |
| `evaluate_search_iteration` | 删除 | 指挥官模型废弃 |

**注意**：`AccumulatedRagResults` 中的 `into_top_n` 和去重逻辑若在其他地方（如 synthesis）使用，需确认迁移。若仅被 evaluator 消费，一并删除。

---

## 7. 测试策略

`LoopOptimizer` 是纯函数，完全可单元测试，无需 mock LLM/tool。

### 7.1 测试用例清单

| 用例 | 输入 | 期望输出 |
|------|------|----------|
| 首次迭代无重复 | iteration=0, chunks=["c1","c2"], remaining=3 | `None` |
| 跨轮重复 chunk | 第0轮 ["c1","c2"], 第1轮 ["c2","c3"] | `DuplicateChunksHint { chunk_ids: ["c2"], first_seen_at: [0] }` |
| 多轮累积重复 | 第0轮 ["c1"], 第1轮 ["c2"], 第2轮 ["c1","c2","c3"] | `DuplicateChunksHint { chunk_ids: ["c1","c2"], first_seen_at: [0,1] }` |
| 同轮重复不触发 | 第0轮 ["c1","c1"]（去重后只剩 c1） | `None` |
| Budget 预警 | remaining=1, max=4, 无重复 | `BudgetWarning { remaining: 1, max: 4 }` |
| 重复优先于 budget | remaining=1, 且有重复 chunk | `DuplicateChunksHint`（互斥规则） |
| Budget 不预警 | remaining=2 | `None` |

### 7.2 chunk 提取 helper 测试

| 用例 | 输入 ToolResult | 期望 chunk_ids |
|------|----------------|----------------|
| dense 有 chunks | `{"chunks": [{"chunk_id": "c1"}]}` | `["c1"]` |
| dense 空 chunks | `{"chunks": []}` | `[]` |
| 非 RAG tool | `web_search` 结果 | `[]` |
| 无 data 字段 | `data: None` | `[]` |

---

## 8. 术语表

| 术语 | 定义 |
|------|------|
| **LoopOptimizer** | ReActLoop 的参谋模块。基于跨迭代信号向 LLM 上下文注入优化提示，不替代 LLM 决策。 |
| **IterationProgress** | LoopOptimizer 的跨轮状态追踪器。仅记录 chunk_id 的首次出现轮次，不存储评分或原文。 |
| **ContextAdjustment** | LoopOptimizer 的输出：可能是重复 chunk 提示、budget 预警，或无需干预。 |
| **信使模型** | Agent 架构模式：LLM 是最终决策者，代码只负责传递上下文和执行 tool。 |
| **指挥官模型** | Agent 架构模式：代码基于信号评估强制改变 LLM 的检索策略（如 evaluator.rs 原设计）。本方案废弃此模式。 |

---

## 9. 实施 Checklist

- [ ] 新建 `loop/optimizer.rs`，实现 `IterationProgress`、`LoopOptimizer`、`ContextAdjustment`
- [ ] 新增 `extract_chunk_ids` helper（处理 dense/lexical/graph 返回格式）
- [ ] 修改 `loop/mod.rs`：在 tool execution 后接入 `LoopOptimizer`
- [ ] 修改 `loop/mod.rs`：在 `ReActLoop` 结构体中持有 `LoopOptimizer`
- [ ] 废弃 `evaluator.rs`：删除模块文件，清理 `mod.rs` 中的引用
- [ ] 确认 `AccumulatedRagResults` 是否有外部消费者，确认后删除或迁移
- [ ] 新增 `optimizer.rs` 单元测试（覆盖 7.1 清单）
- [ ] 新增 `extract_chunk_ids` 单元测试（覆盖 7.2 清单）
- [ ] 运行 `cargo test` 确保现有测试通过
- [ ] 更新 `CONTEXT.md`：加入 LoopOptimizer / IterationProgress / ContextAdjustment 术语
