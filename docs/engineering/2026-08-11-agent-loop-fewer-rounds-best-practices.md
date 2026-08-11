# 减少 Agent Loop 轮次：产品落地（对照主流最佳实践）

- **日期**: 2026-08-11  
- **范围**: 产品 search / rag 的 SaC retrieve → synthesis → verify  
- **原则**: 不单靠砍 `max_iterations`；用架构路径 + 真并行 + 提示/观察 + 运行时防护，让任务**本身**用更少圈完成。

## 1. 对照表（实践 → 本仓库）

| 最佳实践 | 本产品现状 | 落地选择 |
|----------|------------|----------|
| **ReWOO**（一次规划→并行工具→一次合成） | Retrieve 多轮 ReAct；沙箱 gather **名义并行、实际串行** | **Search 快路径语义**：首块扇出全部独立 query；宿主 **真并发** 执行；材料够则 handoff synthesis |
| **Plan-and-Execute** | 无独立 planner；query-card 极简 | 题卡保留；**不**再加第二套 planner（避免多一轮 LLM） |
| **并行 tool call** | 提示写了 gather；**pipe RPC 一问一答串行** | **P0 必修**：异步 `_rpc` + host 并发 `bridge.call` |
| **少步骤提示** | 有同块并行文案；缺「信息够就停」 | 强化 web/agent-base 第三人称效率事实 |
| **硬上限** | `max_iterations` + tokens | 保留；search 8 / rag 12 |
| **接近上限警告** | 仅 C5 耗尽 | 加 `remaining_rounds≤1` 软收束观察 |
| **软基线 2** | 已上 `baseline_rounds` + 3/2 文案 | 保留；配合真并行才有意义 |
| **停滞/重复检测** | 未做 | **P2**（下一刀）：相似 web query 哈希 |
| **工具减法** | search 仅 web/fetch；已较好 | 维持 |
| **历史 prune** | working_set / history_cleared | 维持 |

## 2. 产品目标形态（search 定义/事实类）

```text
[题卡可选] → [retrieve 第 1 圈：同块 gather 多 query，宿主并行 deepseek/brave]
         → observation 有 answer-grade 命中
         → synthesis → verify（ ideally 一次 pass）
```

**不应成为常态**：2–3 圈几乎同义的 `web` + 空 `fetch` + 再合成。

## 3. 切片优先级

| # | 切片 | 预期墙钟杠杆 | 状态 |
|---|------|--------------|------|
| P0 | Bridge **真并发**（gather 才有意义） | 2×web ~25s → ~12s | 本轮 |
| P0b | 提示：首波扇出 + 信息够停 | 少第 2 波重复 web | 本轮 |
| P1 | near-ceiling / 已有命中的环境观察 | 少第 3 圈 | 本轮部分 |
| P2 | 相似 query 重复熔断 | 防刷 web | 后续 |
| P3 | search 定义题跳过 verify 或轻量 verify | 再砍 5–15s | 后续（产品拍板） |

## 4. 权衡

- 并发加深 seek 配额/限流压力 → 单块 query 数建议 ≤4–5。  
- 轮次越少，实时纠错越弱；探索性任务仍允许多圈 ReAct，用软基线与硬顶约束。  
- 不把 verify 在本切片关掉（你方先前要求 search 保留 verify）。

## 5. 验证

- 单元：concurrent bridge 墙钟 ≈ max(latency) 非 sum  
- 实机：同一「什么是 BYOK？」→ 首块 2×web 日志时间戳应接近；product_rounds 趋向 1–2 retrieve + synthesis + verify  
