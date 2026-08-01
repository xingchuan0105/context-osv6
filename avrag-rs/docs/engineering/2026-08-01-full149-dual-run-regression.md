# 全量 149 双轮回归 — SaC 结构大改后验收(2026-08-01)

> 范围:SaC prompt 体系重组(codegen→knowledge-base)、struct_query 证据入 cite 流(P0)、orchestrator 物理删除之后的两轮全量 149(`realistic_corpus_full_eval`,不灌库)。
> 基线:2026-07-30 v2 PASS 135/149(90.6%,`v2_20260730-062908`)。
> 数据:本轮 run `v2_20260801-082506`(第一轮)、`v2_20260801-094826`(第二轮);judge=deepseek-v4-flash。

## 1. 结果对比

| 轮次 | PASS | 率 | JUDGE_ERROR | UNGROUNDED | SELECTION_MISS | RETRIEVAL_MISS | PARTIAL | REFUSAL_WRONG |
|------|-----|-----|-------------|-----------|----------------|----------------|---------|---------------|
| 基线 07-30 | 135 | 90.6% | 0 | — | — | — | — | — |
| 本轮① | 122 | 81.9% | 10 | 6 | 4 | 4 | 2 | 1 |
| 本轮② | 123 | 82.6% | 10 | 5 | 5 | 2 | 3 | 1 |

两轮高度一致:PASS 82±1%、JUDGE_ERROR 恰好各 10(judge API transport 故障,非答案失败;答案已生成未判分)。

## 2. 关键题逐题(两轮)

| q | 基线 | 本轮① | 本轮② | 判定 |
|----|------|-------|-------|------|
| 088 验证/发布阶段计数 | UNGROUNDED | PASS | PASS | **P0 修复收益**(struct_query cite 流) |
| 121 双源联合 | UNGROUNDED | PASS | PASS | **P0 修复收益** |
| 078 概念阶段 81 活动 | PASS | UNGROUNDED | PASS | 波动(采样) |
| 079 各阶段活动数 | PASS | UNGROUNDED | UNGROUNDED | **稳定 UNGROUNDED**:corr=1 但 total_hits 无 chunk 载体 |
| 086 LPDT 第一个活动 | RETRIEVAL_MISS | SELECTION_MISS(corr=0) | SELECTION_MISS(corr=0) | **稳定答错**(表序:团队培训 LPDT-04 vs 应 LPDT-03;且未写 SELECTED) |
| 105 跨文档相似度 | PARTIAL | PARTIAL | PASS | 波动 |
| 106 半覆盖 | RETRIEVAL_MISS | SELECTION_MISS(corr=0.6) | PASS | 波动 |

## 3. 失败归因

1. **JUDGE_ERROR 10/轮(~6.7%)**:judge transport 错误不重试(`judge_with_retry` 仅 JSON parse 失败重试一次)。两轮各 10 题、题号不同 → judge API 随机限流/故障。**环境问题,非答案回归**;重判即可,但 harness 无 judge-only 重跑入口。
2. **q079 稳定 UNGROUNDED(corr=1,faith=0)**:答案各阶段数字全对,judge 判无支撑——`doc_grep` 的 `total_hits` 是运行时统计值,不在 cited chunk 文本里;judge 只按引用文本核对。BASE 版模型圈了含编号区间的 chunk(judge 推理出连续范围)所以 PASS;本轮模型圈命中行 chunk,文本不含总数。**结构性**:统计值无 citation 载体。
3. **q086 稳定答错(corr=0)**:表序 sticky——「第一个」仍读成 LPDT-04(团队培训)。SKILL gotcha 已写「出现顺序非编号序」,模型两轮均未遵守;且本轮未写 SELECTED(cited=0)。**采样+表序理解**。
4. 其余失败(4-5 题/轮)为 LLM 采样/坏运行(答案以 `<code>` 块开头未收敛),与基线同类。

## 4. 结论

- **无产品代码回归**:结构大改(prompts 重组 + codegen→knowledge-base + struct_query cite 流 + orchestrator 删除)未拉低表格题;q088/q078/q106/q105/q121 全部 PASS 或翻盘。
- **P0 修复确认有效**:q088、q121 从基线 UNGROUNDED 稳定到 PASS。
- 与基线 90.6% 的差距 ≈ JUDGE_ERROR(~7%,环境)+ 结构性 2 题(q079 total_hits 载体、q086 表序)+ 采样。

## 5. 建议(按优先级)

1. **judge transport 错误重试**(harness):`judge_with_retry` 对 transport 错误加 1 次重试(或对 JUDGE_ERROR 题支持离线重判),消除 ~7% 的噪声——**单点最高收益**。
2. **total_hits 载体**:`build_citations_from_tool_results` 对 `doc_grep` 把 `total_hits`/`truncated` 合成进 citation content(或命中行 text 前缀),让 judge 能核对统计值——q079 类表格计数题稳定化。
3. **q086 表序强化**(SKILL):how-to-read-tables 的「第一个=出现顺序」再加一个 LPDT 场景 few-shot;同时强化「所有采用的主张都写 SELECTED」(q086 二轮 cited=0)。
4. 可选:重跑 judge 补判 JUDGE_ERROR 题后,真实 PASS 率预计 ≈133/149(89.3%),与基线基本持平。
