# 编排计划：Retrieve 预算从「轮次」改为「Token 为主」

| 项目 | 内容 |
|---|---|
| 日期 | 2026-07-30 |
| 动机 | q058：双文档第二篇已定位 section，轮次耗尽未 fetch → Answer 用记忆补全 |
| 原则 | **Token 驱动成本与上下文**；轮次只防死循环 / 占槽过久 |
| 状态 | **v1 已落地代码**（BudgetConfig / yaml / run_retrieval / loop_budget hint）；全量 nightly 后校准数字 |

---

## 1. 目标

| 角色 | 单位 | 说明 |
|---|---|---|
| **主硬预算** | `max_tokens`（累计 LLM `total_tokens`） | 超则进入 exit / grace / 失败答复 |
| **安全软硬顶** | `max_iterations` | 防止 usage=0 或异常时无限转；默认抬高，使常态由 token 先触顶 |
| **无 chunk grace** | `no_chunk_grace_tokens` + 最少再给 1～2 轮 complete | 替换「固定 +2 轮」为 token 额度（仍保证至少能再跑几轮） |

## 2. 配置形状

```yaml
budget:
  max_iterations: 12          # 安全顶（轮）
  by_user_tier: { free: 8, pro: 12, enterprise: 16 }
  max_tokens: 28000           # 主预算
  max_tokens_by_user_tier: { free: 16000, pro: 28000, enterprise: 40000 }
  no_chunk_grace_tokens: 10000
```

- `max_tokens` 缺省 / `null`：仅轮次（兼容旧配置）。
- Dual 能力：`add_budget` 对 iterations **与** tokens **求和**（与现网 dual 一致）。

## 3. 停机条件（retrieve loop）

```
stop = (tokens_max > 0 && tokens_used >= tokens_max)
     || (iteration >= max_iterations)

若 stop && require_evidence && !has_chunks && !grace_used:
  grace_used = true
  tokens_max += no_chunk_grace_tokens
  max_iterations = max(max_iterations, iteration + 2)  # 至少再 2 次 complete
  注入 NO_CHUNK 提示
  continue

若 stop && !(可 grace):
  budget_exhausted → fallback / 检索失败答复
```

## 4. 对 LLM 可见提示

```xml
<loop_budget
  round="1" max_rounds="12" remaining_rounds="11"
  tokens_used="4200" tokens_max="28000" tokens_remaining="23800" />
```

替代仅 `iteration_budget` 的旧标签（或并存一轮后删）。

## 5. 实现切片

| 切片 | 内容 | 验证 |
|---|---|---|
| A | `BudgetConfig` 字段 + resolve | config 单测 |
| B | yaml（rag/search/chat） | load_mode_config |
| C | `add_budget` 合并 tokens | mode_assemble 单测 |
| D | `run_retrieval` 双条件停机 + grace | agent-loop lib |
| E | budget hint | assembler 单测 |
| F | 交接文档更新 | — |
| G（后置） | 定向复测 q058/q088；全量 nightly | e2e |

## 6. 非目标（本批不做）

- 按 tool observation 字符折算 token（无稳定 tokenizer 时易偏）
- 用户级全局日配额（已有 billing/quota 另一条线）
- 合成阶段 token 预算（仅 retrieve 先改）

## 7. 校准

跑 1～2 次 nightly 后看：

- 平均 `tokens_used` / 是否仍触顶 rounds  
- q058 类双文档是否有足够深度  
- 再调 `max_tokens_*` 与 `no_chunk_grace_tokens`
