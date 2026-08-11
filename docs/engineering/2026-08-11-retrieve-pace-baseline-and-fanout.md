# Retrieve 效率：软基线回合 + 同块扇出引导

- **日期**: 2026-08-11  
- **原则**: 不强制压缩终答；不把 `max_iterations` 砍成 2；用**提示 + 环境反馈**引导更少 LLM 往返、更多并发检索。

## 1. 问题

Search 单问 ~80s 的分解里，retrieve 侧常见浪费是：

1. 可并行的 `client.web` 被拆成多轮（每轮一整圈 LLM→沙箱→回传）。
2. 预算只暴露 `round/max_rounds`，模型缺少「常见路径要多紧」的紧迫感。

## 2. 方案

| 层 | 机制 | 非目标 |
|----|------|--------|
| **提示** | agent-base / web / KB：独立调用同块 `gather`；有依赖才下一回合 | 命令式「必须两轮内答完」 |
| **环境** | `<loop_budget round baseline_rounds max_rounds …>`；`round > baseline` 时附第三人称进度（如 **3/2**） | 到 2 强制停 / 拒继续 |
| **硬顶** | 仍用既有 `max_iterations` + `max_tokens` | 把硬顶改成 2 |

默认 `baseline_iterations: 2`（YAML 可配；`0` = 关闭软基线）。

## 3. 代码

- `BudgetConfig.baseline_iterations`
- `build_loop_budget_hint(..., baseline_rounds)` + `prompts/loop/budget-pace-over-baseline.tmpl.md`
- `modes/search.yaml` / `rag.yaml` 显式 `baseline_iterations: 2`

## 4. 验证

```bash
cargo test -p agent-loop --lib budget_hint
# 本机重启 avrag-api 后：search 简单定义题观察首块是否 fan-out、超基线是否出现 3/2 文案
```
