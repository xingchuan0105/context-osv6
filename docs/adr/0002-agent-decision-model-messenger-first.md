# ADR 0002: Agent 决策权模型——信使模型优先

## Status

Decided

## Context

`ReActLoop`（`loop/mod.rs`）迁移完成后，代码库中存在两套冲突的决策模型：

1. **指挥官模型（Commander Model）**：`evaluator.rs` 的设计意图。代码基于信号（recall_count、max_score、term_coverage）评估迭代质量，返回 `EvalAdvice`（`Replan`、`BroadenQuery`、`EscalateToSearch` 等），强制改变 LLM 的下一步检索策略。这是 v5 状态机时代的思维残留。

2. **信使模型（Messenger Model）**：`ReActLoop` 的实际实现。LLM 是最终决策者，代码只负责组装上下文、执行 tool、返回结果。迭代策略完全由 LLM 的 reasoning 驱动。只有底线 guardrail（`should_block_content_early_stop`、`decide_synthesis_gate`）在危险时拦截。

两套模型不能共存：`evaluator.rs` 的 `EvalAdvice` 从未被主循环消费，但 690 行代码（含测试）持续给维护者带来认知负担——"这个看起来很复杂的模块为什么主循环不用它？"

### 两种模型的根本区别

| 维度 | 指挥官模型（废弃） | 信使模型（采用） |
|------|-------------------|-----------------|
| 决策权 | 代码夺权，LLM 服从 | LLM 保留完整自主权 |
| 输出 | `EvalAdvice`（强制行为切换） | `ContextAdjustment`（提示注入） |
| 干预方式 | 强制改变 tool 调用 / 查询参数 | 向 messages 注入 system 提示 |
| 代表模块 | `evaluator.rs` | `LoopOptimizer`（新建） |
| 适用场景 | 需要严格约束 LLM 行为的封闭系统 | 需要 LLM 创造性推理的开放对话 |

### 为什么指挥官模型不适合当前产品

- 用户选择了信使模型（"LLM 自治"）；
- `EscalateToSearch`（RAG 零召回时强制转搜索）不是当前想要的产品行为；
- 质量趋势分析不可行（rerank/BM25/triplets 评分体系不统一，代码层做趋势是灾难）；
- ReActLoop 的 reasoning 能力足以自我反思，不需要外部进展停滞检测。

## Decision

**废弃指挥官模型，全面采用信使模型。**

具体措施：

1. 删除 `evaluator.rs` 及其全部测试（`EvaluationSignals`、`EvalAdvice`、`evaluate_rag_iteration`、`evaluate_search_iteration`、`AccumulatedRagResults`）；
2. 新建 `LoopOptimizer` 参谋模块（`loop/optimizer.rs`），基于跨迭代信号向 LLM 上下文注入优化提示，不替代 LLM 决策；
3. `LoopOptimizer` 的干预只限于两条规则：
   - **重复 chunk 检测**：提示 LLM 当前轮次返回的 chunk 已在前序迭代中出现；
   - **Budget 预警**：最后一轮迭代前提示 LLM 珍惜机会；
4. 所有提示以自然语言注入，措辞柔和（问句和选项），明确保留 LLM 的选择权；
5. 现有的 guardrail（`should_block_content_early_stop`、`decide_synthesis_gate`）保留不变，作为 L1 硬拦截层。

## Consequences

- **代码清理**：删除 690 行废弃代码（`evaluator.rs`），消除架构漂移；
- **行为可预测**：LLM 的决策逻辑集中在其 reasoning 中，不再被外部代码强制覆盖；
- **测试简化**：`LoopOptimizer` 是纯函数，单元测试覆盖两条规则即可；
- **Locality 提升**：迭代退出逻辑集中在 `exit_policy.rs` + `LoopOptimizer`，bug 不需要在 evaluator 和 loop 之间跳转排查；
- **未来扩展**：如果未来确实需要强制切换模式（如 `EscalateToSearch`），应在产品层面明确需求后，以新的 `ContextAdjustment` 规则加入，而非恢复指挥官模型。

## Related

- `docs/agents/loop-optimizer-design.md`
- `docs/agents/v5-state-machine-cleanup-design.md`
