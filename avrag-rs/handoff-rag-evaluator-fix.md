# Handoff: 修复 RAG Evaluator 无限 "insufficient" 循环

## 背景

正在调查 RAG evaluator 总是返回 "insufficient" 的根因（路径A的后续任务）。

## 已完成的调查

### 根因已确认

RAG PPT E2E 测试（`rag__presentation-html__根据文档_生成一个_ppt_总结其核心观点`）经历 6 次 Evaluate 迭代，每次 evaluator 都返回 `insufficient`，最终 budget 耗尽，耗时 145 秒。

**核心根因**：Evaluator 的覆盖度标准与文档实际内容脱节。

1. **Evaluator 决策规则过于严格**（`prompts/skills/rag-eval/SKILL.md:69-72`）：
   - `sufficient` 要求"all major dimensions are at least covered_weak and none are missing"
   - `insufficient` 只要"one or more major dimensions are missing or weak"

2. **Evaluator 基于世界知识推断"应该"有哪些维度**：
   - 用户问"生成 PPT 总结核心观点"，evaluator 知道《反脆弱》应该有 barbell strategy, via negativa, optionality, Lindy effect, skin in the game 等概念
   - 虽然 SKILL.md 第 92 行禁止 "Do not use prior world knowledge"，但 LLM 实际行为违反此约束

3. **文档实际内容极其有限**：
   - E2E 测试只 ingest 了 4 条 chunks（都是 antifragility 基本定义）
   - 但每次 dense_retrieval 返回 36 results（从 3 个不同 source 重复返回同样的 3 条内容）

4. **循环无法终止**：
   - `rag.rs:753`：`EvalDecision::Insufficient => { /* Convert next_actions to tool calls */ }`
   - 每次执行 next_actions 的 dense_retrieval 仍返回相同内容
   - 循环持续到 budget=4 耗尽

### 证据文件

- 最新 E2E 运行结果：`./crates/app/tests/e2e_output/e2e_20260528-025424_f7d2414a/rag__presentation-html__/meta.json`
- 包含 6 次 evaluator LLM 调用的完整 system_prompt / user_messages / response_content
- 每次 response_content 的 JSON 都显示 `decision: "insufficient"`

## 待修复内容

### P0: 修改 evaluator SKILL.md（方案A）

文件：`prompts/skills/rag-eval/SKILL.md`

在 "Decision rules" 部分增加文档边界约束：
- 告诉 evaluator "只基于实际检索到的内容和文档元数据做判断"
- "如果文档本身内容有限，不要基于外部知识期望文档中'应该'包含更多维度"
- "当检索到的内容直接 relevant 于用户问题的核心需求时，即使不是百科全书式的完整覆盖，也应标记为 sufficient"

同时考虑修改 "Dimension rules" 部分，降低对"全面覆盖"的期望。

### P1: 代码兜底（方案C，如方案A不够）

文件：`crates/app/src/agents/strategy/rag.rs` 的 `step_evaluate`

增加强制终止逻辑：当 evaluator 连续返回 `insufficient` 但 accumulated chunks 数量不再增长时，强制降级为 `sufficient` 或 `give_up`。

### P2: 验证

1. 重新运行 E2E RAG PPT 测试，验证 evaluator 不再无限循环
2. 运行 `cargo test -p app --lib` 确保单元测试通过
3. 检查其他 RAG E2E 测试是否受影响

## 相关文件

- `prompts/skills/rag-eval/SKILL.md` — evaluator 提示词（主修复目标）
- `crates/app/src/agents/strategy/rag.rs:753` — Insufficient 分支处理代码
- `crates/app/src/rag_prompts.rs:661-770` — `build_rag_strategy_evaluation_prompt` 函数
- `crates/app/tests/e2e_rag.rs` — RAG E2E 测试
- `crates/app/tests/e2e_output/` — E2E 运行结果目录

## 之前的提交

- `42349a2` — fix(e2e-analyzer): resolve false positives in diff engine and registry determinism

## 分支

当前在 `worktree-e2e-analyzer` 分支（基于 master）。
