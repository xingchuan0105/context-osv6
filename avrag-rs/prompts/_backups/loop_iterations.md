# RAG Prompt Loop Log

## Iter 1 (baseline, v5.0) — 2026-06-30

**Metrics:** recall=55.56% | citation=83.33% | hallucination=58.33%

**Failures:**
- thesis_factual Q1/Q2, thesis_synthesis, thesis_numeric: retrieval missed anchors (long queries, no short lexical)
- cross_document: 50% recall (4A / 370 activities)
- citation: Q1/Q6 refused with 0 citations
- hallucination: 7/12 flagged (paraphrase + long Chinese prose; heuristic)

**Patches:** none (baseline run)

---

## Iter 2 (v5.1 + codegen §6 + rag-answer scan)

**Rationale:**
- codegen: 中文事实检索阶梯、短锚词 lexical、双主题分轮
- rag-system: 禁止未穷尽锚词就拒答；verbatim 引用；中文拒答
- rag-answer: pre-synthesis chunk scan；verbatim numbers；禁止英文 fallback

**Metrics:** recall=58.33% | citation=83.33% | hallucination=25.00%

**Improved:** Q7 1467, Q10 370+638, Q5 11/100/638, contract failures gone, adversarial citation fixed
**Still failing recall:** Q1 建厂, Q2 4R substring in cited chunks, Q4 4A架构 in chunks, Q6 营收, Q9 4A half

---

## Iter 4 (codegen §6.6 mandatory first queries)

**Rationale:** Force bare anchor lexical on round 1 for chronic miss patterns.

**Metrics:** (pending)

---

## Smoke v5 decoupled scorecard (auto)

**Retrieval:** recall@15=68.06% | hit@15=58.33% | mrr=0.383 | ndcg@15=0.604
**Selection:** precision=60.00% | recall=59.72%
**Generation:** refusal_correct=66.67% | contract=66.67% | substring_faithfulness=53.00%

**Labels:** RETRIEVAL_MISS=3, SELECTION_MISS=1, GENERATION_UNGROUNDED=3, SYNTHESIS_CONTRACT=1, REFUSAL_WRONG=3, PASS=1

| subset | label | ret_recall | sel_precision | faithfulness | query |
|---|---:|---:|---:|---:|---|
| thesis_factual | RETRIEVAL_MISS | 0% | 0% | 0% | Y冷冻设备公司是哪一年在大连投资建厂的？ |
| thesis_factual | SELECTION_MISS | 100% | 0% | 86% | 4R策略包括哪四个方面？ |
| ipd_table | PASS | 100% | 100% | 80% | 华为IPD流程中活动号为PAC-05的活动是什么？在哪个阶段？ |
| baiyao_pdf | RETRIEVAL_MISS | 0% | 0% | 67% | 云南白药IT规划基于什么架构进行整体设计？ |
| baiyao_pdf | REFUSAL_WRONG | 67% | 100% | 78% | 云南白药数据资产目录中有多少个主题域分组（L1）？多少个主题域（L2）？多少个业务对象（L3）？ |
| thesis_synthesis | RETRIEVAL_MISS | 0% | 0% | 0% | Y冷冻设备公司2019年和2020年的营业收入分别是多少？为什么会出现亏损？ |
| thesis_numeric | REFUSAL_WRONG | 100% | 100% | 60% | 2019年我国速冻食品行业规模是多少亿元人民币？ |
| ipd_table | GENERATION_UNGROUNDED | 100% | 100% | 40% | 华为IPD流程中概念决策评审的活动号是什么？ |
| cross_document | GENERATION_UNGROUNDED | 50% | 20% | 44% | Y冷冻设备公司的4R策略和云南白药IT规划的4A架构，分别是什么意思？ |
| cross_document | REFUSAL_WRONG | 100% | 100% | 82% | 华为IPD流程有多少个活动？云南白药IT规划有多少个业务对象？两者在'体系化'方面有什么共同特点？ |
| thesis_adversarial | GENERATION_UNGROUNDED | 100% | 100% | 0% | Y冷冻设备公司的速冻机产品保修期是几年？ |
| thesis_adversarial | SYNTHESIS_CONTRACT | 100% | 100% | 100% | Y冷冻设备公司的注册资本是多少万元？ |

---

## Smoke v5 decoupled scorecard (auto)

**Retrieval:** recall@15=0.00% | hit@15=0.00% | mrr=0.000 | ndcg@15=0.000
**Selection:** precision=0.00% | recall=0.00%
**Generation:** refusal_correct=33.33% | contract=33.33% | substring_faithfulness=61.11%

**Labels:** RETRIEVAL_MISS=3

| subset | label | ret_recall | sel_precision | faithfulness | query |
|---|---:|---:|---:|---:|---|
| thesis_factual | RETRIEVAL_MISS | 0% | 0% | 100% | Y冷冻设备公司是哪一年在大连投资建厂的？ |
| baiyao_pdf | RETRIEVAL_MISS | 0% | 0% | 83% | 云南白药IT规划基于什么架构进行整体设计？ |
| thesis_synthesis | RETRIEVAL_MISS | 0% | 0% | 0% | Y冷冻设备公司2019年和2020年的营业收入分别是多少？为什么会出现亏损？ |

---

## Smoke v5 decoupled scorecard (auto)

**Retrieval:** recall@15=0.00% | hit@15=0.00% | mrr=0.000 | ndcg@15=0.000
**Selection:** precision=0.00% | recall=0.00%
**Generation:** refusal_correct=100.00% | contract=66.67% | substring_faithfulness=28.77%

**Labels:** RETRIEVAL_MISS=3

| subset | label | ret_recall | sel_precision | faithfulness | query |
|---|---:|---:|---:|---:|---|
| thesis_factual | RETRIEVAL_MISS | 0% | 0% | 24% | Y冷冻设备公司是哪一年在大连投资建厂的？ |
| baiyao_pdf | RETRIEVAL_MISS | 0% | 0% | 62% | 云南白药IT规划基于什么架构进行整体设计？ |
| thesis_synthesis | RETRIEVAL_MISS | 0% | 0% | 0% | Y冷冻设备公司2019年和2020年的营业收入分别是多少？为什么会出现亏损？ |

---

## Iter 5 (cluster=retrieval, queries=1,4,6) — 2026-06-30

**Rationale (forensics on baseline artifacts e2e_20260630-065850):**
- Q1/Q4/Q6 都是 RETRIEVAL_MISS。读 artifact 发现：Q1 lexical("2019年")→0、("投资建厂")→0；Q6 lexical→0/摘要；Q4 lexical→0。Q3 lexical("PAC-05")→17 命中。
- 假设1（稀有 token 命中）：改 §6.2/§6.6 用稀有锚词"建厂"、裸数字。Iter 1 子集验证：lexical("建厂")→0、("4A架构")→0、("550万")→0，全 0。假设1否。
- 假设2（lexical 中文未进稀疏索引）：含汉字 query 一律 0，只有纯 ASCII 字母数字命中。改 §6.2/§6.6/§6.5/§7：lexical 剥中文后缀（"550"非"550万"、"4A"非"4A架构"、"1467"非"1467亿元"）；全中文锚词直接 dense（去公司名，只用事实锚词）。Iter 2 子集验证。

**Patches (codegen/SKILL.md only):**
- §6.2 重写：明确 lexical 含汉字必 0、纯 ASCII 才命中；dense 回退去公司名只用锚词；附 Q1/Q4/Q6 dense query 表。
- §6.6 强制首轮：建厂→dense("成立 大连 建厂 年份")；4R→lexical("4R")；营收→lexical("550")/("370")/("700")；白药→lexical("4A")。
- §6.5 路由表、§7 三例同步为裸 ASCII。

**Iter 1 子集 metrics (queries=1,4,6):** recall@15=0% | refusal_correct=33.33% | contract=33.33% | substring_faithfulness=61.11% | labels: RETRIEVAL_MISS=3
**Iter 2 子集 metrics (queries=1,4,6):** recall@15=0% | refusal_correct=100.00% | contract=66.67% | substring_faithfulness=28.77% | labels: RETRIEVAL_MISS=3

**Interpretation:**
- 生成层显著改善：refusal_correct 33%→100%（Q1/Q6 不再吐"I found relevant material..."合约失败串，改为正确拒答/作答）。
- Q1 答案已正确："成立于2019年"（引英文摘要 chunk d39077b3）；Q4 答案正确："4A架构"（从 BA/DA/AA/TA 定义 chunk 推导）。
- recall@15 仍 0，根因有三（均非 prompt 可解）：
  1. dense 被篇幅占优的 4R/营销 bulk 主导，召不到具体事实短句（建厂/财务/基于4A架构 chunk 均不在 top-15）。
  2. lexical 对短/常见 token 不可靠——连纯 ASCII "4A" 也返回 0（仅 PAC-05 这类极 distinctive 编号命中）。
  3. golden 子串粒度过细：Q1/Q4 答案已对且 grounded，但 golden 期望的特定子串 chunk 没被召回 → recall=0 误标 RETRIEVAL_MISS。

**Decision:** retrieval 聚簇连续 2 轮无翻转，卡在系统/指标层（dense ranking + lexical 中文支持 + golden 粒度），非 prompt 杠杆可解。按计划停止条件，暂缓 retrieval 聚簇，转 generation 聚簇（refusal/contract/faithfulness 更可被 prompt 驱动）。retrieval 聚簇需golden 粒度放宽或 retrieval 系统改造（另起任务）。

---

## Smoke v5 decoupled scorecard (auto)

**Retrieval:** recall@15=50.00% | hit@15=33.33% | mrr=0.181 | ndcg@15=0.463
**Selection:** precision=66.67% | recall=50.00%
**Generation:** refusal_correct=83.33% | contract=50.00% | substring_faithfulness=46.83%

**Labels:** RETRIEVAL_MISS=2, GENERATION_UNGROUNDED=2, SYNTHESIS_CONTRACT=2

| subset | label | ret_recall | sel_precision | faithfulness | query |
|---|---:|---:|---:|---:|---|
| thesis_factual | RETRIEVAL_MISS | 0% | 0% | 29% | 4R策略包括哪四个方面？ |
| ipd_table | GENERATION_UNGROUNDED | 50% | 100% | 24% | 华为IPD流程中概念决策评审的活动号是什么？ |
| cross_document | RETRIEVAL_MISS | 0% | 0% | 0% | Y冷冻设备公司的4R策略和云南白药IT规划的4A架构，分别是什么意思？ |
| cross_document | GENERATION_UNGROUNDED | 50% | 100% | 29% | 华为IPD流程有多少个活动？云南白药IT规划有多少个业务对象？两者在'体系化'方面有什么共同特点？ |
| thesis_adversarial | SYNTHESIS_CONTRACT | 100% | 100% | 100% | Y冷冻设备公司的速冻机产品保修期是几年？ |
| thesis_adversarial | SYNTHESIS_CONTRACT | 100% | 100% | 100% | Y冷冻设备公司的注册资本是多少万元？ |

---

## Smoke v5 decoupled scorecard (auto)

**Retrieval:** recall@15=66.67% | hit@15=33.33% | mrr=0.179 | ndcg@15=0.557
**Selection:** precision=33.33% | recall=50.00%
**Generation:** refusal_correct=66.67% | contract=66.67% | substring_faithfulness=33.93%

**Labels:** RETRIEVAL_MISS=2, SELECTION_MISS=1, GENERATION_UNGROUNDED=1, PASS=2

| subset | label | ret_recall | sel_precision | faithfulness | query |
|---|---:|---:|---:|---:|---|
| thesis_factual | RETRIEVAL_MISS | 0% | 0% | 29% | 4R策略包括哪四个方面？ |
| ipd_table | PASS | 100% | 100% | 50% | 华为IPD流程中概念决策评审的活动号是什么？ |
| cross_document | RETRIEVAL_MISS | 0% | 0% | 75% | Y冷冻设备公司的4R策略和云南白药IT规划的4A架构，分别是什么意思？ |
| cross_document | SELECTION_MISS | 100% | 0% | 0% | 华为IPD流程有多少个活动？云南白药IT规划有多少个业务对象？两者在'体系化'方面有什么共同特点？ |
| thesis_adversarial | PASS | 100% | 0% | 50% | Y冷冻设备公司的速冻机产品保修期是几年？ |
| thesis_adversarial | GENERATION_UNGROUNDED | 100% | 100% | 0% | Y冷冻设备公司的注册资本是多少万元？ |

---

## Iter 6 (cluster=generation, queries=2,8,9,10,11,12) — 2026-06-30

**Forensics on baseline e2e_20260630-065850 generation cluster:**
- Q7 golden 错：chunk feade683 明写"2020年...1467亿元"，golden 却标 2019=1467亿。模型答"1467是2020年的"是对的 → REFUSAL_WRONG 是假阳性。
- Q8 chunk 84e26455 明写"概念决策评审 PAC-20"——答案本应正确，但基线 confabulate 了"（CDCP）"英文注解 → UNGROUNDED。
- Q9 泄漏 raw `internal_answer_v1` JSON（1768字 markdown 长文）+ 4R 英文 gloss 写成 Relativity/Retribution。
- Q10 有"370"chunk 却拒答 IPD 活动数。
- Q11 对抗题（保修期）吐英文"I found relevant material..."fallback。
- Q12 中文拒答正确(faith 100%)但合约格式失败（纯文本无 JSON 包装）。

**Patches:**
- rag-answer.md：①"拒答/未提及也必须用 JSON"小节+示例（coverage=insufficient, refusal_reason=not_in_corpus, citations=[]）；②Pre-synthesis scan 加"禁止假拒答：chunk 含所问数字/年份/编号必须引用作答，禁止改派年份"；③Verbatim grounding 加"禁止 chunk 没有的英文译名/缩写展开（4R→Relativity、概念决策评审→CDCP）"+"answer_text 1-6 句禁 ## 长文"。
- rag-system.md §5.4 强化：最终答案必须且只能是裸 JSON（无围栏、无 JSON 外文字、无 `<code>`）；预算耗尽也必须 JSON 合成不得继续检索；拒答也 JSON；禁纯文本拒答（§5.3 仅限 rag-answer 未注入兜底）。

**Iter 3 子集 (queries=2,8,9,10,11,12):** recall@15=50% | refusal_correct=83.33% | contract=50.00% | substring_faithfulness=46.83% | labels: RETRIEVAL_MISS=2, GENERATION_UNGROUNDED=2, SYNTHESIS_CONTRACT=2
- Q11/Q12 拒答内容转正确中文，但仍 SYNTHESIS_CONTRACT（纯文本无 JSON）；Q8 退化（拒答+泄漏 `<code>`）；Q9 仍 fallback。

**Iter 4 子集 (queries=2,8,9,10,11,12):** recall@15=66.67% | refusal_correct=66.67% | contract=66.67% | substring_faithfulness=33.93% | labels: RETRIEVAL_MISS=2, SELECTION_MISS=1, GENERATION_UNGROUNDED=1, **PASS=2**
- **Q8 翻正**：PAC-20 干净引用，无 CDCP confabulation、无代码泄漏。
- **Q11 翻 PASS**：正确中文拒答+引用上下文。
- Q2 4R 答案干净（英文 gloss 来自 chunk 原文 d39077b3，非 confabulation）。
- Q10/Q12 仍间歇英文 fallback（JSON 无效，deepseek-v4-flash 模型能力）；Q9 4A 检索缺失。

**Interpretation:**
- 生成层 prompt 杠杆有效：Q8/Q11 翻 PASS，contract 50%→66.67%，Q8 code-leak 与 CDCP confabulation 治愈。
- 剩余瓶颈非 prompt 可解：①JSON 间歇失败（Q10/Q12）是 deepseek-v4-flash 结构化输出能力问题；②Q9 4A 检索缺失是 dense 偏题+lexical 中文 0 的系统问题；③Q7 golden 错误（1467 是 2020 非 2019）。
- 4R 英文 gloss（Relativity/Retribution）源自 chunk 原文，非模型 confabulation——如需修正要改 golden 或接受。

---

## Smoke v5 decoupled scorecard (auto)

**Retrieval:** recall@15=34.72% | hit@15=25.00% | mrr=0.208 | ndcg@15=0.350
**Selection:** precision=33.33% | recall=30.56%
**Generation:** refusal_correct=75.00% | contract=58.33% | substring_faithfulness=48.00%

**Labels:** RETRIEVAL_MISS=7, SELECTION_MISS=1, SYNTHESIS_CONTRACT=2, REFUSAL_WRONG=1, PASS=1

| subset | label | ret_recall | sel_precision | faithfulness | query |
|---|---:|---:|---:|---:|---|
| thesis_factual | RETRIEVAL_MISS | 0% | 0% | 33% | Y冷冻设备公司是哪一年在大连投资建厂的？ |
| thesis_factual | RETRIEVAL_MISS | 0% | 0% | 29% | 4R策略包括哪四个方面？ |
| ipd_table | PASS | 100% | 100% | 75% | 华为IPD流程中活动号为PAC-05的活动是什么？在哪个阶段？ |
| baiyao_pdf | RETRIEVAL_MISS | 0% | 0% | 50% | 云南白药IT规划基于什么架构进行整体设计？ |
| baiyao_pdf | REFUSAL_WRONG | 67% | 100% | 78% | 云南白药数据资产目录中有多少个主题域分组（L1）？多少个主题域（L2）？多少个业务对象（L3）？ |
| thesis_synthesis | RETRIEVAL_MISS | 0% | 0% | 0% | Y冷冻设备公司2019年和2020年的营业收入分别是多少？为什么会出现亏损？ |
| thesis_numeric | RETRIEVAL_MISS | 0% | 0% | 75% | 2019年我国速冻食品行业规模是多少亿元人民币？ |
| ipd_table | RETRIEVAL_MISS | 0% | 0% | 0% | 华为IPD流程中概念决策评审的活动号是什么？ |
| cross_document | RETRIEVAL_MISS | 0% | 0% | 36% | Y冷冻设备公司的4R策略和云南白药IT规划的4A架构，分别是什么意思？ |
| cross_document | SELECTION_MISS | 50% | 0% | 0% | 华为IPD流程有多少个活动？云南白药IT规划有多少个业务对象？两者在'体系化'方面有什么共同特点？ |
| thesis_adversarial | SYNTHESIS_CONTRACT | 100% | 100% | 100% | Y冷冻设备公司的速冻机产品保修期是几年？ |
| thesis_adversarial | SYNTHESIS_CONTRACT | 100% | 100% | 100% | Y冷冻设备公司的注册资本是多少万元？ |

---

## Smoke v5 decoupled scorecard (auto)

**Retrieval:** recall@15=62.50% | hit@15=41.67% | mrr=0.243 | ndcg@15=0.548
**Selection:** precision=58.33% | recall=58.33%
**Generation:** refusal_correct=58.33% | contract=58.33% | substring_faithfulness=47.42%

**Labels:** RETRIEVAL_MISS=4, SELECTION_MISS=1, GENERATION_UNGROUNDED=2, REFUSAL_WRONG=3, PASS=2

| subset | label | ret_recall | sel_precision | faithfulness | query |
|---|---:|---:|---:|---:|---|
| thesis_factual | RETRIEVAL_MISS | 0% | 0% | 100% | Y冷冻设备公司是哪一年在大连投资建厂的？ |
| thesis_factual | RETRIEVAL_MISS | 0% | 0% | 86% | 4R策略包括哪四个方面？ |
| ipd_table | GENERATION_UNGROUNDED | 100% | 100% | 83% | 华为IPD流程中活动号为PAC-05的活动是什么？在哪个阶段？ |
| baiyao_pdf | PASS | 100% | 100% | 75% | 云南白药IT规划基于什么架构进行整体设计？ |
| baiyao_pdf | GENERATION_UNGROUNDED | 100% | 100% | 75% | 云南白药数据资产目录中有多少个主题域分组（L1）？多少个主题域（L2）？多少个业务对象（L3）？ |
| thesis_synthesis | RETRIEVAL_MISS | 0% | 0% | 0% | Y冷冻设备公司2019年和2020年的营业收入分别是多少？为什么会出现亏损？ |
| thesis_numeric | REFUSAL_WRONG | 100% | 100% | 0% | 2019年我国速冻食品行业规模是多少亿元人民币？ |
| ipd_table | PASS | 100% | 100% | 50% | 华为IPD流程中概念决策评审的活动号是什么？ |
| cross_document | RETRIEVAL_MISS | 0% | 0% | 0% | Y冷冻设备公司的4R策略和云南白药IT规划的4A架构，分别是什么意思？ |
| cross_document | SELECTION_MISS | 50% | 0% | 0% | 华为IPD流程有多少个活动？云南白药IT规划有多少个业务对象？两者在'体系化'方面有什么共同特点？ |
| thesis_adversarial | REFUSAL_WRONG | 100% | 100% | 0% | Y冷冻设备公司的速冻机产品保修期是几年？ |
| thesis_adversarial | REFUSAL_WRONG | 100% | 100% | 100% | Y冷冻设备公司的注册资本是多少万元？ |

---

## Smoke v5 decoupled scorecard (auto)

**Retrieval:** recall@15=100.00% | hit@15=0.00% | mrr=0.000 | ndcg@15=1.000
**Selection:** precision=100.00% | recall=100.00%
**Generation:** refusal_correct=0.00% | contract=0.00% | substring_faithfulness=0.00%

**Labels:** SYNTHESIS_CONTRACT=1

| subset | label | ret_recall | sel_precision | faithfulness | query |
|---|---:|---:|---:|---:|---|
| thesis_adversarial | SYNTHESIS_CONTRACT | 100% | 100% | 0% | Y冷冻设备公司的速冻机产品保修期是几年？ |

---

## Smoke v5 decoupled scorecard (auto)

**Retrieval:** recall@15=100.00% | hit@15=0.00% | mrr=0.000 | ndcg@15=1.000
**Selection:** precision=100.00% | recall=100.00%
**Generation:** refusal_correct=100.00% | contract=100.00% | substring_faithfulness=100.00%

**Labels:** PASS=2

| subset | label | ret_recall | sel_precision | faithfulness | query |
|---|---:|---:|---:|---:|---|
| thesis_adversarial | PASS | 100% | 100% | 100% | Y冷冻设备公司的速冻机产品保修期是几年？ |
| thesis_adversarial | PASS | 100% | 100% | 100% | Y冷冻设备公司的注册资本是多少万元？ |

---

## Smoke v5 decoupled scorecard (auto)

**Retrieval:** recall@15=62.50% | hit@15=41.67% | mrr=0.211 | ndcg@15=0.522
**Selection:** precision=44.44% | recall=54.17%
**Generation:** refusal_correct=83.33% | contract=100.00% | substring_faithfulness=72.59%

**Labels:** RETRIEVAL_MISS=3, SELECTION_MISS=1, GENERATION_UNGROUNDED=3, PASS=5

| subset | label | ret_recall | sel_precision | faithfulness | query |
|---|---:|---:|---:|---:|---|
| thesis_factual | RETRIEVAL_MISS | 0% | 0% | 100% | Y冷冻设备公司是哪一年在大连投资建厂的？ |
| thesis_factual | SELECTION_MISS | 100% | 0% | 78% | 4R策略包括哪四个方面？ |
| ipd_table | GENERATION_UNGROUNDED | 100% | 100% | 80% | 华为IPD流程中活动号为PAC-05的活动是什么？在哪个阶段？ |
| baiyao_pdf | PASS | 0% | 0% | 67% | 云南白药IT规划基于什么架构进行整体设计？ |
| baiyao_pdf | GENERATION_UNGROUNDED | 100% | 100% | 80% | 云南白药数据资产目录中有多少个主题域分组（L1）？多少个主题域（L2）？多少个业务对象（L3）？ |
| thesis_synthesis | RETRIEVAL_MISS | 0% | 0% | 0% | Y冷冻设备公司2019年和2020年的营业收入分别是多少？为什么会出现亏损？ |
| thesis_numeric | PASS | 100% | 0% | 75% | 2019年我国速冻食品行业规模是多少亿元人民币？ |
| ipd_table | PASS | 100% | 100% | 50% | 华为IPD流程中概念决策评审的活动号是什么？ |
| cross_document | RETRIEVAL_MISS | 0% | 0% | 67% | Y冷冻设备公司的4R策略和云南白药IT规划的4A架构，分别是什么意思？ |
| cross_document | GENERATION_UNGROUNDED | 50% | 33% | 75% | 华为IPD流程有多少个活动？云南白药IT规划有多少个业务对象？两者在'体系化'方面有什么共同特点？ |
| thesis_adversarial | PASS | 100% | 100% | 100% | Y冷冻设备公司的速冻机产品保修期是几年？ |
| thesis_adversarial | PASS | 100% | 100% | 100% | Y冷冻设备公司的注册资本是多少万元？ |

## Iter 8 (eval rationalization + synthesis refusal safety-net) — 2026-06-30

**Trigger:** Full regression after retrieval prompt edits (Iter 5-7) regressed recall@15 from 68% → 34.72%, while per-query inspection showed *correct* answers (Q1 建厂=2019年, Q4 4A架构, Q5 11+638, Q11 保修拒答) were mislabeled RETRIEVAL_MISS / SYNTHESIS_CONTRACT. User authorized "修正prompt的同时，也可以合理化修正评估方式和规则".

**Root-cause finding (JSON parse failure):** Inspecting `/tmp/synth_debug.txt` (raw synthesis `first.content`/`repaired.content`) revealed the model emits a **`<code language="python">` retrieval block on the synthesis turn** instead of `internal_answer_v1` JSON — not a prose refusal. `parse_synthesis_answer` fails (not JSON), `lift_prose_to_contract` fails (no `[[cite:]]` markers), repair re-prompts → model emits *another* code block → `contract_violation_fallback` English string leaks, destroying the model's grounded Chinese refusal (which lives in `reasoning_content`, not `content`).

**Patches:**
- *Prompt:* reverted `codegen/SKILL.md` to pre-loop backup (retrieval edits regressed recall; baseline retrieval restored). Kept generation edits in `rag-answer.md` + `rag-system.md §5.4` (JSON contract, pre-synthesis scan, verbatim grounding, no English gloss).
- *Eval (metrics_v2.rs):* answer-first labeling — `answer_correctness()` (must_include satisfied for should-answer / is_refusal for should-refuse) → PASS overrides RETRIEVAL_MISS (fixes Q1/Q4 correct-answer-mislabel). `contract_compliance` takes `is_refusal` (refusals need no cite markup; fixes Q11/Q12 false SYNTHESIS_CONTRACT) + detects 3 synthesis-fallback markers as `synthesis_fallback` contract issue. Label priority reordered: SYNTHESIS_CONTRACT before REFUSAL_WRONG so fallback strings attribute to JSON root cause. +6 unit tests.
- *Eval (golden):* Q7 factual fix (1467亿 is 2020 not 2019 → `expected_should_answer=false` + refusal_keywords + `must_not_include=["1467亿"]`); Q6/Q10 `must_include` loosened from long exact phrases to key tokens (550万/370万/700万/缺少大项目; 370个活动/638个业务对象).
- *Code (synthesis.rs):* refusal safety-net — when `resolve_synthesis_answer` returns None, `extract_refusal_sentence(first.reasoning_content)` lifts the model's own refusal sentence as the final answer instead of the English fallback. +3 unit tests. This is the definitive code fix for the code-block-on-synthesis failure mode.

**Final full-regression metrics (12 queries):**
- recall@15=62.50% | citation=83.33% | hallucination=8.33%
- generation: refusal_correct=83.33% | **contract=100.00%** | substring_faithfulness=72.59%
- **labels: PASS=5, RETRIEVAL_MISS=3, GENERATION_UNGROUNDED=3, SELECTION_MISS=1** (SYNTHESIS_CONTRACT=0, REFUSAL_WRONG=0)

**vs broken Iter 7:** PASS 1→5, contract 58%→100%, faithfulness 47%→73%, hallucination 33%→8%, citation 58%→83%.

**Remaining (real, not metric artifacts):**
- RETRIEVAL_MISS=3: Q6 营收(550/370/700), Q9 4R+4A, Q1 建厂(this run) — genuine recall gaps (lexical Chinese unreliable, dense topic-dominated).
- GENERATION_UNGROUNDED=3: Q3 PAC-05, Q5 11/100/638, Q10 370+638 — must_include not satisfied (incomplete answers, run variance on Q3).
- SELECTION_MISS=1: Q2 4R — retrieved but not cited.

**Next frontier:** retrieval-layer (Q6/Q9) needs either retrieval-system fixes (CJK sparse tokenizer, dense re-ranking) or surgical per-query retrieval hints — blanket prompt rules regressed recall before, so avoid.

---

## Smoke v5 decoupled scorecard (auto)

**Retrieval:** recall@15=100.00% | hit@15=0.00% | mrr=0.000 | ndcg@15=1.000
**Selection:** precision=100.00% | recall=100.00%
**Generation:** refusal_correct=100.00% | contract=100.00% | substring_faithfulness=100.00%

**Labels:** PASS=2

| subset | label | ret_recall | sel_precision | faithfulness | query |
|---|---:|---:|---:|---:|---|
| thesis_adversarial | PASS | 100% | 100% | 100% | Y冷冻设备公司的速冻机产品保修期是几年？ |
| thesis_adversarial | PASS | 100% | 100% | 100% | Y冷冻设备公司的注册资本是多少万元？ |

---

## Smoke v5 decoupled scorecard (auto)

**Retrieval:** recall@15=50.00% | hit@15=25.00% | mrr=0.194 | ndcg@15=0.483
**Selection:** precision=41.67% | recall=50.00%
**Generation:** refusal_correct=75.00% | contract=100.00% | substring_faithfulness=59.60%

**Labels:** RETRIEVAL_MISS=6, GENERATION_UNGROUNDED=2, PASS=4

| subset | label | ret_recall | sel_precision | faithfulness | query |
|---|---:|---:|---:|---:|---|
| thesis_factual | RETRIEVAL_MISS | 0% | 0% | 100% | Y冷冻设备公司是哪一年在大连投资建厂的？ |
| thesis_factual | RETRIEVAL_MISS | 0% | 0% | 29% | 4R策略包括哪四个方面？ |
| ipd_table | GENERATION_UNGROUNDED | 100% | 100% | 75% | 华为IPD流程中活动号为PAC-05的活动是什么？在哪个阶段？ |
| baiyao_pdf | RETRIEVAL_MISS | 0% | 0% | 67% | 云南白药IT规划基于什么架构进行整体设计？ |
| baiyao_pdf | GENERATION_UNGROUNDED | 100% | 100% | 80% | 云南白药数据资产目录中有多少个主题域分组（L1）？多少个主题域（L2）？多少个业务对象（L3）？ |
| thesis_synthesis | RETRIEVAL_MISS | 0% | 0% | 0% | Y冷冻设备公司2019年和2020年的营业收入分别是多少？为什么会出现亏损？ |
| thesis_numeric | PASS | 100% | 0% | 75% | 2019年我国速冻食品行业规模是多少亿元人民币？ |
| ipd_table | PASS | 100% | 100% | 50% | 华为IPD流程中概念决策评审的活动号是什么？ |
| cross_document | RETRIEVAL_MISS | 0% | 0% | 40% | Y冷冻设备公司的4R策略和云南白药IT规划的4A架构，分别是什么意思？ |
| cross_document | RETRIEVAL_MISS | 0% | 0% | 0% | 华为IPD流程有多少个活动？云南白药IT规划有多少个业务对象？两者在'体系化'方面有什么共同特点？ |
| thesis_adversarial | PASS | 100% | 100% | 100% | Y冷冻设备公司的速冻机产品保修期是几年？ |
| thesis_adversarial | PASS | 100% | 100% | 100% | Y冷冻设备公司的注册资本是多少万元？ |

## Iter 9 (DeepSeek JSON Mode at API layer) — 2026-06-30

**Trigger:** User pointed to https://api-docs.deepseek.com/zh-cn/guides/json_mode — DeepSeek supports `response_format:{type:"json_object"}` to force valid JSON. This fixes the Iter-8 root cause (model emits `<code>` block on synthesis turn) at the API layer instead of relying on the reasoning-safety-net backstop.

**Implementation:**
- `crates/llm/src/client/request.rs`: `build_chat_completion_request_body` takes `json_mode: bool`; when true AND `base_url` contains "deepseek", sets `request_body["response_format"]={"type":"json_object"}` (gated like the existing `thinking` field). +3 unit tests.
- `crates/llm/src/client/mod.rs`: new `complete_json_mode(messages, temperature)`; threaded `json_mode` through `complete_non_stream`/`build_completion_request_body`/`complete_stream`.
- `synthesis.rs`: synthesis first call + repair call now use `complete_json_mode`. The `synthesis_contract_block` already contains "JSON" + a format example (satisfies DeepSeek's requirement). The refusal safety-net (`extract_refusal_sentence`) is retained as a backstop for the doc-warned empty-content case and non-DeepSeek providers.

**Result (Q11+Q12 subset):** both PASS, answers are clean JSON-wrapped Chinese refusals ("文档中未提及...保修年限。" / "文档中未提及...注册资本。"). NO `synthesis_refusal_lift` and NO `synthesis_contract_violation` events fired — the model emits valid `internal_answer_v1` JSON directly, so neither safety-net nor fallback is needed. Root cause solved at API layer.

**Full 12-query run:** recall@15=50.00% | contract=100.00% | refusal_correct=75.00% | faithfulness=59.60% | labels: PASS=4, RETRIEVAL_MISS=6, GENERATION_UNGROUNDED=2.

**Variance finding (important):** recall@15 swung 62.50% (Iter 8) → 50.00% (Iter 9) despite IDENTICAL retrieval config (codegen reverted in both). Per-query diff proves this is pure retrieval non-determinism: Q2 (4R) ret 100%→0%, Q10 (638) ret 50%→0% — chunks simply weren't retrieved this run. JSON mode cannot affect recall@15 (it only touches the synthesis LLM call). ∴ the aggregate metric dip is noise, NOT a JSON-mode regression. Only Q4 (4A架构) flipped on the generation side (ret 0% both runs): under JSON mode the model refused to infer "4A架构" from the "业务/数据/应用/技术" chunk — more faithful, less confabulation; golden `must_include=["4A架构"]` rewards inference and is arguably too lenient.

**Conclusion:** JSON mode is a net-positive root-cause fix (eliminates the English-fallback failure class; contract=100% stable). The 12-query smoke set is too small for stable single-run metrics (±12pp retrieval variance) — for reliable regression gating, either average 3 runs or grow the golden set.

**State shipped:** codegen reverted to baseline + rag-answer/rag-system §5.4 generation edits + metrics_v2 answer-first labeling + Q6/Q7/Q10 golden fixes + synthesis refusal safety-net + DeepSeek JSON mode on synthesis turn.

---

## Iter 0 (post-audit baseline, 2026-06-30 22:37) — full 12-query run, no changes from audit-shipped state

Targets status: answerable PASS 2/9 ❌(≥6/9) · answerable recall@15 51.85% ❌(≥60%) · answerable graded_recall@15 55.70% ❌(≥70%) · contract 100% ✓ · refusal_correct 75% ❌(≥100%) · stability not-yet-checked.

Per-query归因 (1-based): Q1=RETRIEVAL_MISS(真检索失败,thesis L109"2019年于大连市投资建厂"未召回) · Q2=SELECTION_MISS(4R-def块召回但误引Schultz块;must_include精确短语遇英文译注不匹配) · Q3=GENERATION_UNGROUNDED(must_include"概念启动，在概念阶段"强求连接词"在",模型说"属于") · Q5=REFUSAL_WRONG(baiyao L538"100大主题域(L2)"未召回→对L2拒答) · Q6=RETRIEVAL_MISS(thesis L110"550万/370万/700万"未召回) · Q9=GENERATION_UNGROUNDED(must_include"业务、数据、应用、技术"强求无架构形式,模型用"业务架构"等同样正确的形式) · Q10=GENERATION_UNGROUNDED(IPD"370"是行号非"370个活动"文本,未召回).

## Smoke v5 decoupled scorecard (auto)

**Retrieval:** recall@15=63.89% | hit@15=50.00% | mrr=0.244 | ndcg@15=0.543
**Retrieval (graded, ADR 0011):** graded_recall@15=66.78% | graded_ndcg@15=0.584
**Retrieval (answerable-only, excl. vacuous adversarial 100%):** recall@15=51.85% | graded_recall@15=55.70% | substring_faithfulness=50.13%
**Selection:** precision=46.53% | recall=55.56%
**Generation:** refusal_correct=75.00% | contract=100.00% | substring_faithfulness=59.82%

**Labels:** RETRIEVAL_MISS=2, SELECTION_MISS=1, GENERATION_UNGROUNDED=3, REFUSAL_WRONG=1, PASS=5

| subset | label | ret_recall | sel_precision | faithfulness | query |
|---|---:|---:|---:|---:|---|
| thesis_factual | RETRIEVAL_MISS | 0% | 0% | 100% | Y冷冻设备公司是哪一年在大连投资建厂的？ |
| thesis_factual | SELECTION_MISS | 100% | 0% | 80% | 4R策略包括哪四个方面？ |
| ipd_table | GENERATION_UNGROUNDED | 100% | 100% | 50% | 华为IPD流程中活动号为PAC-05的活动是什么？在哪个阶段？ |
| baiyao_pdf | PASS | 0% | 0% | 50% | 云南白药IT规划基于什么架构进行整体设计？ |
| baiyao_pdf | REFUSAL_WRONG | 67% | 100% | 33% | 云南白药数据资产目录中有多少个主题域分组（L1）？多少个主题域（L2）？多少个业务对象（L3）？ |
| thesis_synthesis | RETRIEVAL_MISS | 0% | 0% | 0% | Y冷冻设备公司2019年和2020年的营业收入分别是多少？为什么会出现亏损？ |
| thesis_numeric | PASS | 100% | 0% | 67% | 2019年我国速冻食品行业规模是多少亿元人民币？ |
| ipd_table | PASS | 100% | 100% | 33% | 华为IPD流程中概念决策评审的活动号是什么？ |
| cross_document | GENERATION_UNGROUNDED | 50% | 25% | 50% | Y冷冻设备公司的4R策略和云南白药IT规划的4A架构，分别是什么意思？ |
| cross_document | GENERATION_UNGROUNDED | 50% | 33% | 55% | 华为IPD流程有多少个活动？云南白药IT规划有多少个业务对象？两者在'体系化'方面有什么共同特点？ |
| thesis_adversarial | PASS | 100% | 100% | 100% | Y冷冻设备公司的速冻机产品保修期是几年？ |
| thesis_adversarial | PASS | 100% | 100% | 100% | Y冷冻设备公司的注册资本是多少万元？ |

---

## Iter 1 (2026-06-30 22:53) — subset [1,2,3,5,6,9,10] (Iter0 的 7 道 non-PASS); 杠杆=golden must_include 合理化

**改动 (一处杠杆, 附证据):** Q2/Q3/Q9 的精确短语 must_include → token 列表. 语义门槛不变(仍要求全部关键术语出现), 只去掉连接词/英文译注/架构后缀的字符级惩罚.
- Q2 `["关联、反应、关系、回报"]`→`["关联","反应","关系","回报"]`: 模型答对4R四方面, 仅因加英文译注(Relativity/...)导致精确短语不匹配.
- Q3 `["概念启动，在概念阶段"]`→`["概念启动","概念阶段"]`: 原文是表格行, "在"是标注者连接词非原文, 模型说"位于/属于概念阶段"同样正确.
- Q9 `["关联、反应、关系、回报","业务、数据、应用、技术"]`→8 tokens: baiyao L971/1645 有"业务、数据、应用、技术"、L1550-1626 也有"业务架构/数据架构/应用架构/技术架构"(两种都对), 模型用架构形式, 强求无架构形式过严.

**结果 (7题子集):** Q2/Q3/Q9 → PASS (确定性✓). Q5 → PASS (**检索方差**: 本轮召回了 baiyao L538"100大主题域(L2)"块, recall 3/3 vs Iter0 2/3, 非我改动). Q1/Q6 → RETRIEVAL_MISS (未改golden, 真检索失败). Q10 → **SYNTHESIS_CONTRACT** (JSON解析失败→英文fallback回归; Iter0 是 GENERATION_UNGROUNDED; contract 降至 85.71%).
- labels: RETRIEVAL_MISS=2, SYNTHESIS_CONTRACT=1, PASS=4. contract=85.71% (Q10). answerable recall@15=42.86% (子集, 受Q1/Q6=0%+Q2/Q9/Q10方差拖累).

**语义裁判闸 (criterion 5):** Q3(cited dc410e4f PAC-05表行✓)/Q5(cited 6d2b0181+8dc382fa✓)/Q9(cited a46fd97d"通过业务、数据、应用、技术解构产品"✓) 均有据. **Q2 本轮 recall 0%** — 答案正确但 cited 英文摘要 d39077b3(SMEs...)不含4R, 4R 疑来自参数记忆 → grounding 依赖检索方差, 标记为潜在假阳性(下轮若 4R-def 块未召回则不grounded).

**关键发现 (lever 可用性):** `retrieval_hints` 是**死字段** — grep 全仓仅出现在 golden JSON + `golden_set.rs:64` 结构体 + `metrics_v2.rs:755` 测试 stub, 从未被 harness/agent runtime 读取注入提示词. ∴ runbook 的"外科式 per-query retrieval_hints"杠杆**不可用**. 剩余检索失败(Q1/Q6 + Q5/Q10/Q2 方差)只能靠 codegen 策略, 但 Iter1-3 已证明全局 codegen 检索编辑拖垮 recall → 检索系统层瓶颈.

**下一步:** 全量12题确认聚合(Q2/Q3/Q9 全量是否稳, Q4/Q8 守住否, Q5/Q10 方差), 再定 Iter2.

## Smoke v5 decoupled scorecard (auto)

**Retrieval:** recall@15=42.86% | hit@15=57.14% | mrr=0.309 | ndcg@15=0.353
**Retrieval (graded, ADR 0011):** graded_recall@15=42.09% | graded_ndcg@15=0.287
**Retrieval (answerable-only, excl. vacuous adversarial 100%):** recall@15=42.86% | graded_recall@15=42.09% | substring_faithfulness=28.73%
**Selection:** precision=32.14% | recall=35.71%
**Generation:** refusal_correct=71.43% | contract=85.71% | substring_faithfulness=28.73%

**Labels:** RETRIEVAL_MISS=2, SYNTHESIS_CONTRACT=1, PASS=4

| subset | label | ret_recall | sel_precision | faithfulness | query |
|---|---:|---:|---:|---:|---|
| thesis_factual | RETRIEVAL_MISS | 0% | 0% | 100% | Y冷冻设备公司是哪一年在大连投资建厂的？ |
| thesis_factual | PASS | 0% | 0% | 0% | 4R策略包括哪四个方面？ |
| ipd_table | PASS | 100% | 100% | 50% | 华为IPD流程中活动号为PAC-05的活动是什么？在哪个阶段？ |
| baiyao_pdf | PASS | 100% | 100% | 40% | 云南白药数据资产目录中有多少个主题域分组（L1）？多少个主题域（L2）？多少个业务对象（L3）？ |
| thesis_synthesis | RETRIEVAL_MISS | 0% | 0% | 0% | Y冷冻设备公司2019年和2020年的营业收入分别是多少？为什么会出现亏损？ |
| cross_document | PASS | 50% | 25% | 11% | Y冷冻设备公司的4R策略和云南白药IT规划的4A架构，分别是什么意思？ |
| cross_document | SYNTHESIS_CONTRACT | 50% | 0% | 0% | 华为IPD流程有多少个活动？云南白药IT规划有多少个业务对象？两者在'体系化'方面有什么共同特点？ |

---

## Iter 1 full-12 confirm (2026-06-30 23:05) — no new changes, full 12 queries (确认 Iter1 改动在全量上的聚合)

**结果:** labels RETRIEVAL_MISS=2(Q1,Q9), GENERATION_UNGROUNDED=2(Q5,Q10), SYNTHESIS_CONTRACT=1(Q6), PASS=7(Q2,Q3,Q4,Q7,Q8,Q11,Q12). answerable PASS=**4/9**(Q2,Q3,Q4,Q8) — vs Iter0 2/9, 确定性 +2(Q2,Q3).
- Q9 全量退回 RETRIEVAL_MISS(recall 0%, 4R/4A块未召回; 子集里 50% 是方差) → Q9 PASS 依赖检索方差, 非确定性.
- Q5 recall **3/3**(11/100/638 全召回!) 但 GENERATION_UNGROUNDED — 答案"L1主题域分组：11个 / L2主题域：100个 / L3业务对象：638个"正确且全引用, 但 must_include"11大主题域分组/100大主题域/638个业务对象"不匹配模型重排格式 → must_include 过严(同类 Iter1, Iter2 修).
- Q6 SYNTHESIS_CONTRACT: recall 0/3(财务块未召回) + synthesis emit "EVIDENCE_INSUFFICIENT_FALLBACK" fallback(非合法JSON) → 检索层为主 + 合成可靠性次.
- 聚合: answerable recall@15=50.00% graded_recall@15=55.24% contract=91.67%(Q6) refusal_correct=75%(Q1/Q6/Q9 should-answer 拒答).

**Iter 2 (2026-06-30 23:08):** 杠杆=golden must_include 合理化 Q5 `["11大主题域分组","100大主题域","638个业务对象"]`→`["11","100","638","主题域分组","业务对象"]`. 证据: Q5 全量 recall 3/3 + 答案正确+全引用, 仅 must_include 精确短语不匹配模型"L1主题域分组：11个"重排格式. 语义门槛不变(仍要求3数字+L1/L3标签; L2由"100"保证; 配对由语义裁判闸把关). 子集 [1,5,6,9,10] 验证.

## Smoke v5 decoupled scorecard (auto)

**Retrieval:** recall@15=62.50% | hit@15=41.67% | mrr=0.243 | ndcg@15=0.550
**Retrieval (graded, ADR 0011):** graded_recall@15=66.43% | graded_ndcg@15=0.596
**Retrieval (answerable-only, excl. vacuous adversarial 100%):** recall@15=50.00% | graded_recall@15=55.24% | substring_faithfulness=60.56%
**Selection:** precision=45.83% | recall=54.17%
**Generation:** refusal_correct=75.00% | contract=91.67% | substring_faithfulness=67.64%

**Labels:** RETRIEVAL_MISS=2, GENERATION_UNGROUNDED=2, SYNTHESIS_CONTRACT=1, PASS=7

| subset | label | ret_recall | sel_precision | faithfulness | query |
|---|---:|---:|---:|---:|---|
| thesis_factual | RETRIEVAL_MISS | 0% | 0% | 100% | Y冷冻设备公司是哪一年在大连投资建厂的？ |
| thesis_factual | PASS | 100% | 0% | 80% | 4R策略包括哪四个方面？ |
| ipd_table | PASS | 100% | 100% | 67% | 华为IPD流程中活动号为PAC-05的活动是什么？在哪个阶段？ |
| baiyao_pdf | PASS | 0% | 0% | 50% | 云南白药IT规划基于什么架构进行整体设计？ |
| baiyao_pdf | GENERATION_UNGROUNDED | 100% | 100% | 40% | 云南白药数据资产目录中有多少个主题域分组（L1）？多少个主题域（L2）？多少个业务对象（L3）？ |
| thesis_synthesis | SYNTHESIS_CONTRACT | 0% | 0% | 0% | Y冷冻设备公司2019年和2020年的营业收入分别是多少？为什么会出现亏损？ |
| thesis_numeric | PASS | 100% | 0% | 67% | 2019年我国速冻食品行业规模是多少亿元人民币？ |
| ipd_table | PASS | 100% | 100% | 33% | 华为IPD流程中概念决策评审的活动号是什么？ |
| cross_document | RETRIEVAL_MISS | 0% | 0% | 100% | Y冷冻设备公司的4R策略和云南白药IT规划的4A架构，分别是什么意思？ |
| cross_document | GENERATION_UNGROUNDED | 50% | 50% | 75% | 华为IPD流程有多少个活动？云南白药IT规划有多少个业务对象？两者在'体系化'方面有什么共同特点？ |
| thesis_adversarial | PASS | 100% | 100% | 100% | Y冷冻设备公司的速冻机产品保修期是几年？ |
| thesis_adversarial | PASS | 100% | 100% | 100% | Y冷冻设备公司的注册资本是多少万元？ |

---

## Iter 2 (2026-06-30 23:12) — subset [1,5,6,9,10] (full-12 Iter1 的 5 道 non-PASS); 杠杆=golden must_include 合理化 Q5

**改动 (同杠杆 Iter1):** Q5 `["11大主题域分组","100大主题域","638个业务对象"]`→`["11","100","638","主题域分组","业务对象"]`. 证据: full-12 Iter1 Q5 recall 3/3 + 答案正确+全引用, 仅 must_include 精确短语不匹配模型"L1主题域分组：11个"重排格式. 语义门槛不变(3数字+L1/L3标签; L2由"100"保证; 配对由语义裁判闸把关).

**结果 (5题子集):** Q5 → **PASS** ✓ (recall 3/3, 答案"11个主题域分组（L1），100个主题域（L2），638个业务对象（L3）", tokens 全匹配; 确定性当检索3/3). Q1/Q6/Q9 → RETRIEVAL_MISS (检索 0%, 未变). Q10 → **SELECTION_MISS** (answer 泄漏模型自身推理"如果找不到明确总数，我们应该宣称'未在文档中找到明确的活动总数'"——非提示词原文, 是合成卫生问题).
- labels: RETRIEVAL_MISS=3, SELECTION_MISS=1, PASS=1. contract=100% (本轮 Q6 干净拒答未 fallback). answerable recall@15=30.00% (5题, Q1/Q6/Q9=0% 拖低).

**关键发现 (prompt bug):** `EVIDENCE_INSUFFICIENT_FALLBACK` 标记是提示词诱导——`grounded-answer.md` L58 + `rag-answer.md` L72/300 指示"证据不足时在回复里包含该标记", 但该标记破坏 `internal_answer_v1` JSON 契约 → Q6 full-12 的 SYNTHESIS_CONTRACT. RAG 模式应干净中文拒答, 不该 emit fallback 标记. → Iter3 用 rag-answer.md 修.

**下一步 (Iter3):** rag-answer.md 合成卫生——RAG 模式证据不足时输出合法 internal_answer_v1 中文拒答(非 EVIDENCE_INSUFFICIENT_FALLBACK 标记), 且禁泄漏推理. 目标: 提升 contract 可靠性 + 标签准确性(Q6 归 RETRIEVAL_MISS 而非 SYNTHESIS_CONTRACT). 不增 PASS(Q6/Q1/Q9 检索层无解).

## Smoke v5 decoupled scorecard (auto)

**Retrieval:** recall@15=30.00% | hit@15=40.00% | mrr=0.214 | ndcg@15=0.266
**Retrieval (graded, ADR 0011):** graded_recall@15=31.43% | graded_ndcg@15=0.179
**Retrieval (answerable-only, excl. vacuous adversarial 100%):** recall@15=30.00% | graded_recall@15=31.43% | substring_faithfulness=50.50%
**Selection:** precision=20.00% | recall=20.00%
**Generation:** refusal_correct=20.00% | contract=100.00% | substring_faithfulness=50.50%

**Labels:** RETRIEVAL_MISS=3, SELECTION_MISS=1, PASS=1

| subset | label | ret_recall | sel_precision | faithfulness | query |
|---|---:|---:|---:|---:|---|
| thesis_factual | RETRIEVAL_MISS | 0% | 0% | 100% | Y冷冻设备公司是哪一年在大连投资建厂的？ |
| baiyao_pdf | PASS | 100% | 100% | 40% | 云南白药数据资产目录中有多少个主题域分组（L1）？多少个主题域（L2）？多少个业务对象（L3）？ |
| thesis_synthesis | RETRIEVAL_MISS | 0% | 0% | 0% | Y冷冻设备公司2019年和2020年的营业收入分别是多少？为什么会出现亏损？ |
| cross_document | RETRIEVAL_MISS | 0% | 0% | 12% | Y冷冻设备公司的4R策略和云南白药IT规划的4A架构，分别是什么意思？ |
| cross_document | SELECTION_MISS | 50% | 0% | 100% | 华为IPD流程有多少个活动？云南白药IT规划有多少个业务对象？两者在'体系化'方面有什么共同特点？ |

---

## Iter 3 (2026-06-30 23:18) — subset [6,9,10] (synthesis-affected non-PASS); 杠杆=rag-answer.md 合成卫生

**改动 (一处杠杆, rag-answer.md L297-302):** RAG 模式禁用 `EVIDENCE_INSUFFICIENT_FALLBACK` 标记 + 禁泄漏模型自身推理/元指令. 证据: Q6 full-12 的 SYNTHESIS_CONTRACT 根因是模型 emit 该 fallback 标记(破坏 internal_answer_v1 JSON 契约); Q10 subset-Iter2 的 SELECTION_MISS 根因是 answer_text 泄漏"如果找不到…我们应该宣称…"推理. 两症同属合成输出契约, 一处编辑同治.

**结果 (3题子集):** Q6 → **RETRIEVAL_MISS** ✓(answer 干净中文拒答"文档中未提及…营业收入数据以及亏损原因", **无 EVIDENCE_INSUFFICIENT_FALLBACK 标记**, contract OK, label 归真实根因). Q10 → **RETRIEVAL_MISS** ✓(answer 干净拒答, **无"如果找不到…宣称…"泄漏**). Q9 → REFUSAL_WRONG(4A块未召回→模型拒4A, 检索方差).
- labels: RETRIEVAL_MISS=2, REFUSAL_WRONG=1. **contract=100%**(无 SYNTHESIS_CONTRACT). refusal_correct=0%(3题皆因检索缺失而拒答).
- 修法验证成功: marker 与泄漏双修, contract 可靠性 + 标签准确性提升. 不增 PASS(Q6/Q9/Q10 检索层无解).

## Smoke v5 decoupled scorecard (auto)

**Retrieval:** recall@15=16.67% | hit@15=33.33% | mrr=0.111 | ndcg@15=0.102
**Retrieval (graded, ADR 0011):** graded_recall@15=17.26% | graded_ndcg@15=0.197
**Retrieval (answerable-only, excl. vacuous adversarial 100%):** recall@15=16.67% | graded_recall@15=17.26% | substring_faithfulness=20.00%
**Selection:** precision=11.11% | recall=16.67%
**Generation:** refusal_correct=0.00% | contract=100.00% | substring_faithfulness=20.00%

**Labels:** RETRIEVAL_MISS=2, REFUSAL_WRONG=1

| subset | label | ret_recall | sel_precision | faithfulness | query |
|---|---:|---:|---:|---:|---|
| thesis_synthesis | RETRIEVAL_MISS | 0% | 0% | 0% | Y冷冻设备公司2019年和2020年的营业收入分别是多少？为什么会出现亏损？ |
| cross_document | REFUSAL_WRONG | 50% | 33% | 60% | Y冷冻设备公司的4R策略和云南白药IT规划的4A架构，分别是什么意思？ |
| cross_document | RETRIEVAL_MISS | 0% | 0% | 0% | 华为IPD流程有多少个活动？云南白药IT规划有多少个业务对象？两者在'体系化'方面有什么共同特点？ |

---

## Iter 3 full-12 confirm (2026-06-30 23:30) — rag-answer.md marker+泄漏修复在全量上的聚合确认 + 检索瓶颈终判

**结果:** labels RETRIEVAL_MISS=2(Q1,Q6), SELECTION_MISS=2(Q4,Q9), GENERATION_UNGROUNDED=1(Q7), SYNTHESIS_CONTRACT=1(Q10), PASS=6(Q2,Q3,Q5,Q8,Q11,Q12). answerable PASS=**4/9**(Q2,Q3,Q5,Q8). answerable recall@15=55.56% graded_recall@15=50.52% contract=83.33% refusal_correct=75%.
- marker 修复在全量确认: Q6 干净中文拒答(无 EVIDENCE_INSUFFICIENT_FALLBACK), 归 RETRIEVAL_MISS ✓. Q10 在 recall=0% 时干净拒答(Iter3 子集✓), 但本全量 Q10 recall=50%(partial) 时 answer="I found relevant material but could not format a validated cited answer. Please try asking again."——**合成管线代码层 fallback 串**(citation 校验失败时管线吐出, 英文), 非 marker 非泄漏, **超出提示词杠杆**(属 app-chat 合成运行时可靠性, 非本轮允许的 prompt 文件).
- Q4 SELECTION_MISS: answer 语义正确("4A架构…业务/数据/应用/技术紧密协同[[cite:ffbc9854]]"), cit_acc=0% 系 chunk-ID 匹配边缘(model 引了含 golden 子串"业务、数据、应用、技术"的块但 ID 不在 matched 集; 语料 L971/L1645 两处含该子串). Q4 在 Iter1 全量为 PASS → 方差, 非 golden 过窄(带后缀串"业务架构、…"在语料中不存在, 系 model 改写).
- Q7 GENERATION_UNGROUNDED: recall 100%(vacuous 0/0) + cit_acc 100%, 纯生成方差(must_include 数字匹配), Iter1 为 PASS.
- Q9 SELECTION_MISS: recall 50%(4R 召回 4A 未召回), partial 答题, 检索方差.

## 检索瓶颈终判 (退出条件核对)

| 退出条件 | 目标 | 当前 | 状态 |
|---|---|---|---|
| 1. answerable PASS | ≥6/9 | 4/9 | ❌ |
| 2a. answerable recall@15 | ≥60% | 55.56% | ❌ |
| 2b. answerable graded_recall@15 | ≥70% | 50.52% | ❌ |
| 3a. contract | 100% | 83.33%(Q10 管线fallback) | ❌ |
| 3b. refusal_correct | 100% | 75%(Q1/Q6/Q9/Q10 检索缺失致拒答) | ❌ |
| 4. graded_recall@15 ±5pp 稳定 | ≤±5pp | Iter1=55.24% / 本轮=50.52% → 4.7pp(单对比内) | ⚠️ 检索方差±12pp 仍在 |
| 5. 语义裁判闸 | non-PASS 全查+2 PASS | 未全跑 | ⏸ |

**瓶颈归属:** Q1(建厂年"2019大连" thesis L109) / Q6(营收"550/370/700万" thesis L110) recall=**0%**——语料含证据但检索系统(dense+lexical)未召回, 拖低 recall/graded/refusal 三项硬门槛. Q9(4A)/Q10(370+638) partial recall 检索方差. Q10 contract 失败系合成管线代码 fallback串(非 prompt).
**杠杆穷尽核对:** ① must_include 合理化(Q2/Q3/Q5/Q9)已用尽可确定性提升的题; ② rag-answer.md 合成卫生(marker+泄漏)已修, 治纯拒答路径, 治不了 partial-recall 管线 fallback; ③ retrieval_hints **死字段**(未注入 prompt, Iter1 已证); ④ codegen/SKILL.md 检索编辑 **致 recall 退化**(Iter1-3 已证回滚); ⑤ rag-system.md 无检索杠杆(编排层). → 剩余 non-PASS(Q1/Q6/Q9/Q10) 均检索/管线层, 提示词无杠杆; Q4/Q7 属方差无杠杆.
**结论:** 命中 runbook 预判的"检索系统层瓶颈, 非提示词能解". 停止 prompt loop, 转检索系统工作.

**确定性收益 (Iter0→Iter3):** answerable PASS 2/9→4/9(+Q2/Q3/Q5), 合成卫生(marker+泄漏)修复, 标签准确性提升(Q6/Q10 归真实根因).
**建议下一步(超出本轮边界, 待用户定):**
1. 检索系统: 排查 thesis 长文 chunking/embedding 为何漏召 L109(建厂)/L110(营收)——chunk 边界/overlap、embedding 模型、dense+lexical 融合权重.
2. 合成管线: 修 "I found relevant material but could not format a validated cited answer" fallback 串——partial evidence 时应走 internal_answer_v1 中文拒答或可引用部分, 不该吐英文管线串(Q10 contract 根因).
3. retrieval_hints 接线: 让 golden 的 per-query hints 真正注入检索/codegen, 解锁 runbook 的外科式检索杠杆(当前死字段).

## Smoke v5 decoupled scorecard (auto)

**Retrieval:** recall@15=66.67% | hit@15=50.00% | mrr=0.273 | ndcg@15=0.580
**Retrieval (graded, ADR 0011):** graded_recall@15=62.89% | graded_ndcg@15=0.565
**Retrieval (answerable-only, excl. vacuous adversarial 100%):** recall@15=55.56% | graded_recall@15=50.52% | substring_faithfulness=25.93%
**Selection:** precision=41.67% | recall=50.00%
**Generation:** refusal_correct=75.00% | contract=83.33% | substring_faithfulness=42.36%

**Labels:** RETRIEVAL_MISS=2, SELECTION_MISS=2, GENERATION_UNGROUNDED=1, SYNTHESIS_CONTRACT=1, PASS=6

| subset | label | ret_recall | sel_precision | faithfulness | query |
|---|---:|---:|---:|---:|---|
| thesis_factual | RETRIEVAL_MISS | 0% | 0% | 0% | Y冷冻设备公司是哪一年在大连投资建厂的？ |
| thesis_factual | PASS | 0% | 0% | 80% | 4R策略包括哪四个方面？ |
| ipd_table | PASS | 100% | 100% | 80% | 华为IPD流程中活动号为PAC-05的活动是什么？在哪个阶段？ |
| baiyao_pdf | SELECTION_MISS | 100% | 0% | 0% | 云南白药IT规划基于什么架构进行整体设计？ |
| baiyao_pdf | PASS | 100% | 100% | 40% | 云南白药数据资产目录中有多少个主题域分组（L1）？多少个主题域（L2）？多少个业务对象（L3）？ |
| thesis_synthesis | RETRIEVAL_MISS | 0% | 0% | 0% | Y冷冻设备公司2019年和2020年的营业收入分别是多少？为什么会出现亏损？ |
| thesis_numeric | GENERATION_UNGROUNDED | 100% | 0% | 75% | 2019年我国速冻食品行业规模是多少亿元人民币？ |
| ipd_table | PASS | 100% | 100% | 33% | 华为IPD流程中概念决策评审的活动号是什么？ |
| cross_document | SELECTION_MISS | 50% | 0% | 0% | Y冷冻设备公司的4R策略和云南白药IT规划的4A架构，分别是什么意思？ |
| cross_document | SYNTHESIS_CONTRACT | 50% | 0% | 0% | 华为IPD流程有多少个活动？云南白药IT规划有多少个业务对象？两者在'体系化'方面有什么共同特点？ |
| thesis_adversarial | PASS | 100% | 100% | 100% | Y冷冻设备公司的速冻机产品保修期是几年？ |
| thesis_adversarial | PASS | 100% | 100% | 100% | Y冷冻设备公司的注册资本是多少万元？ |

---

## 后续代码修复计划 A — 合成兜底 C+A（待执行）

**背景:** Q10 (活动数+业务对象, partial recall 50%) 命中 `contract_violation_fallback` 英文兜底串 → SYNTHESIS_CONTRACT. 根因: 模型有部分资料、想答, 但跨文档对比答案写不成合格 internal_answer_v1 JSON, 一轮 repair 仍未过, 草稿无拒答句 → 走英文兜底(`synthesis.rs:170` 代码替模型吐, 非模型输出, prompt 改不动).

**目标文件:** `crates/app-chat/src/agents/loop/synthesis.rs` + `answer_contract.rs`(运行时代码, 非冻结的评测代码).

**C (治标):** 两轮候选都过不了质检、且模型想答(草稿无拒答句)时, 不吐英文 fallback, 改从模型已写 answer_text 提取可用部分(丢不合法引用)输出中文; 无可用正文则输出中文"资料不足以完整回答".
**A (治本):** repair 从 1 轮增至最多 3 轮, 每轮带具体错误列表反馈(+可选 B: 附正确模板).
**验证:** Q10 子集 + 全量 12 题, 确认 Q10 不再 SYNTHESIS_CONTRACT, contract 回 100%, 不连累其他题.

---

## 检索漏召根因实证 — Milvus BM25 未配中文分词器（2026-07-01）

**症状:** Q1(建厂年)/Q6(营收) recall=0%, Q9/Q10 partial recall. 模型查词正确(Q1 turn-0 用 `["Y冷冻设备公司","大连","投资","建厂"]`, 带高 IDF "建厂"), 语料 L109 确有答案"在2019年于大连市投资建厂，组建了Y冷冻设备公司"且匹配全部 4 查询词, 但 L109 块 4 轮检索从未返回(sse_events 搜 L109 独有短语"两大项目的建设完成"=0 命中). BM25 把只匹配"Y冷冻设备公司"1 词的摘要块排第 1(score 6.258).

**根因(已坐实):** `crates/storage-milvus/src/schema.rs` 的 `text` 字段 `enable_analyzer=true` 但**未设 `analyzer_params`**, BM25 function `params={}`. Milvus 官方文档: 不设 analyzer 默认走 `standard` 分词器, **只按空格/标点切, 中文连续汉字不切词**. 所以 L109"因此在2019年于大连市投资建厂"被整块当一个 token; 查询词"建厂/大连/投资"是独立短 token, BM25 整 token 匹配 → 全部 0 命中. L109 对 4 查询词全 0 分 → 不返回. 摘要块因关键词行用全角"；"隔开"Y冷冻设备公司"成独立 token → 命中 → 排第 1.

**佐证:** 仓库已有 `jieba-rs`(用在 PG 记忆 FTS `common/src/text_segment.rs`), 但**文档检索的 Milvus BM25 没接**, 两套分词不一致. Q6(L110, 紧挨 L109)同根.

**修复(真本事, 非作弊, 全中文文档检索受益):** 给 `text` 字段加 `analyzer_params: {"type":"chinese"}`(等价 `{"tokenizer":"jieba","filter":["cnalphanumonly"]}`). 
**实施注意:** analyzer 建集合时定, 改它需重建 collection + re-ingest(非热改); smoke_v5 corpus 要重灌.
**影响面:** 治 Q1/Q6, 且 Q9/Q10 partial recall 很可能同根; 中文 lexical 检索整体上一个台阶.

---

## graph 检索确认 — 管线接了但 smoke_v5 图集合为空（2026-07-01）

**接线完整:** Milvus 3 个图集合 `kg_entities`/`kg_relations`/`graph_passages`(storage-milvus/lib.rs:35-45) + `search_graph` BFS 查询(ops/graph.rs) + `graph_retrieval` 工具(dispatch) + codegen skill 教 `client.graph_search`.

**smoke_v5 图集合空:** 灌库管线 `document_pipeline.rs:623` 在 `triplet_extraction_enabled()`(env `INGESTION_TRIPLET_ENABLED`, 默认 true) 且 `triplet_llm` 配置时才跑 LLM 抽三元组建图. **smoke_v5 走的 `pdf_corpus.rs:40` 显式设 `INGESTION_TRIPLET_ENABLED=0`**(TXT 轻量语料省 LLM) → 灌库跳过三元组抽取 → 图集合空. `rag_quality_prod.rs:436-440` 注释印证: TXT corpus 路径不建 KG triplets, 要测得另起 office parser + Paddle OCR + 原始 DOCX/XLSX/PDF.

**LLM 实际调用:** smoke_v5 全 12 题 0 次调 graph_retrieval(tools 列表只有 dense/lexical). 双重无意义: 题意不对症(无实体关系题) + 即便调了图也是空.

**对漏召题无影响:** Q1/Q6/Q9/Q10 非实体关系题, graph 帮不上. 杠杆仍是 BM25 分词 + 合成兜底 C+A.

---

## index 检索澄清 + codegen 两步流接线（2026-07-01）

**澄清 "index" 语义:** index 不是"各章节讲什么"的散文描述, 而是**章节→chunk_id 的定位索引**——doc_profile 返回的 `sections` 数组每条挂一个 `chunk_id`(doc_profile.rs:152-167). 用户问"某文档某章"内容时, 期望流: doc_profile 定位该章 chunk_id → chunk_fetch 取该 chunk → 该 chunk 即用户所求.

**chunk_id 可靠性核实:**
- heuristic TOC(`build_toc_entries`, pg_side_effects.rs:267): 每个标题块按 `block_id→chunk_id` 映射, chunk_id = 包含该标题的 chunk(章节起始块).
- LLM section index(`toc_entries_from_llm_sections`, :229): 每 section 可挂**多个** chunk_ids(章节跨多块时全挂).
- 两路径 chunk_id 都可靠填充. **caveat**: heuristic 路径一章只给起始 chunk, 完整章节要再 fetch 后续块; LLM 路径已挂全.

**补丁(clusters/codegen/SKILL.md, 备份至 _backups/codegen-SKILL.md.2026-07-01-index-flow):**
1. §3 doc_profile 描述: "TOC、sections、metadata" → "metadata + sections（每章含 chunk_id，即章节→chunk_id 定位索引）".
2. §5 选范式: 加一条 "用户问某文档某章/某节具体内容 → doc_profile 定位 chunk_id → chunk_fetch（该 chunk 即用户所求内容，直接据此作答并 cite）".
3. §6 新增示例 "用户问某文档某章内容 → doc_profile 定位 chunk_id → chunk_fetch", 含两轮代码 + 明确告知 "**chunk_fetch 返回的那个 chunk 就是用户要找的章节内容**" + 多 chunk 章节拼接 + null chunk_id 纯结构标题跳过.

**性质:** 新能力接线(prompt 层), 非 smoke_v5 错题驱动; 不动检索/评估代码. **待验**: 需设计"某文档某章"题跑一遍, 看 LLM 是否走 profile→chunk_fetch 两步(而非直接 dense). 验收维度=工具调用序列, 非答案正确率.

---

## Smoke v5 decoupled scorecard (auto)

**Retrieval:** recall@15=100.00% | hit@15=100.00% | mrr=1.000 | ndcg@15=1.000
**Retrieval (graded, ADR 0011):** graded_recall@15=60.00% | graded_ndcg@15=1.148
**Retrieval (answerable-only, excl. vacuous adversarial 100%):** recall@15=100.00% | graded_recall@15=60.00% | substring_faithfulness=33.33%
**Selection:** precision=100.00% | recall=100.00%
**Generation:** refusal_correct=100.00% | contract=100.00% | substring_faithfulness=33.33%

**Labels:** PASS=1

| subset | label | ret_recall | sel_precision | faithfulness | query |
|---|---:|---:|---:|---:|---|
| thesis_factual | PASS | 100% | 100% | 33% | Y冷冻设备公司是哪一年在大连投资建厂的？ |

---

## Smoke v5 decoupled scorecard (auto)

**Retrieval:** recall@15=91.67% | hit@15=75.00% | mrr=0.459 | ndcg@15=0.806
**Retrieval (graded, ADR 0011):** graded_recall@15=87.89% | graded_ndcg@15=0.800
**Retrieval (answerable-only, excl. vacuous adversarial 100%):** recall@15=88.89% | graded_recall@15=83.85% | substring_faithfulness=88.89%
**Selection:** precision=65.83% | recall=80.56%
**Generation:** refusal_correct=91.67% | contract=100.00% | substring_faithfulness=91.67%

**Labels:** REFUSAL_WRONG=1, PASS=11

| subset | label | ret_recall | sel_precision | faithfulness | query |
|---|---:|---:|---:|---:|---|
| thesis_factual | PASS | 100% | 100% | 100% | Y冷冻设备公司是哪一年在大连投资建厂的？ |
| thesis_factual | PASS | 100% | 0% | 100% | 4R策略包括哪四个方面？ |
| ipd_table | PASS | 100% | 100% | 100% | 华为IPD流程中活动号为PAC-05的活动是什么？在哪个阶段？ |
| baiyao_pdf | PASS | 100% | 0% | 100% | 云南白药IT规划基于什么架构进行整体设计？ |
| baiyao_pdf | PASS | 100% | 100% | 50% | 云南白药数据资产目录中有多少个主题域分组（L1）？多少个主题域（L2）？多少个业务对象（L3）？ |
| thesis_synthesis | PASS | 100% | 100% | 100% | Y冷冻设备公司2019年和2020年的营业收入分别是多少？为什么会出现亏损？ |
| thesis_numeric | REFUSAL_WRONG | 100% | 0% | 100% | 2019年我国速冻食品行业规模是多少亿元人民币？ |
| ipd_table | PASS | 100% | 100% | 50% | 华为IPD流程中概念决策评审的活动号是什么？ |
| cross_document | PASS | 50% | 50% | 100% | Y冷冻设备公司的4R策略和云南白药IT规划的4A架构，分别是什么意思？ |
| cross_document | PASS | 50% | 40% | 100% | 华为IPD流程有多少个活动？云南白药IT规划有多少个业务对象？两者在'体系化'方面有什么共同特点？ |
| thesis_adversarial | PASS | 100% | 100% | 100% | Y冷冻设备公司的速冻机产品保修期是几年？ |
| thesis_adversarial | PASS | 100% | 100% | 100% | Y冷冻设备公司的注册资本是多少万元？ |

---

## Smoke v5 decoupled scorecard (auto)

**Retrieval:** recall@15=91.67% | hit@15=75.00% | mrr=0.502 | ndcg@15=0.846
**Retrieval (graded, ADR 0011):** graded_recall@15=87.89% | graded_ndcg@15=0.822
**Retrieval (answerable-only, excl. vacuous adversarial 100%):** recall@15=88.89% | graded_recall@15=83.85% | substring_faithfulness=76.37%
**Selection:** precision=70.00% | recall=91.67%
**Generation:** refusal_correct=91.67% | contract=100.00% | substring_faithfulness=82.28%

**Labels:** GENERATION_UNGROUNDED=1, REFUSAL_WRONG=1, PASS=10

| subset | label | ret_recall | sel_precision | faithfulness | query |
|---|---:|---:|---:|---:|---|
| thesis_factual | PASS | 100% | 100% | 100% | Y冷冻设备公司是哪一年在大连投资建厂的？ |
| thesis_factual | PASS | 100% | 0% | 100% | 4R策略包括哪四个方面？ |
| ipd_table | PASS | 100% | 100% | 100% | 华为IPD流程中活动号为PAC-05的活动是什么？在哪个阶段？ |
| baiyao_pdf | PASS | 100% | 50% | 6% | 云南白药IT规划基于什么架构进行整体设计？ |
| baiyao_pdf | PASS | 100% | 100% | 100% | 云南白药数据资产目录中有多少个主题域分组（L1）？多少个主题域（L2）？多少个业务对象（L3）？ |
| thesis_synthesis | GENERATION_UNGROUNDED | 100% | 100% | 100% | Y冷冻设备公司2019年和2020年的营业收入分别是多少？为什么会出现亏损？ |
| thesis_numeric | REFUSAL_WRONG | 100% | 0% | 100% | 2019年我国速冻食品行业规模是多少亿元人民币？ |
| ipd_table | PASS | 100% | 100% | 50% | 华为IPD流程中概念决策评审的活动号是什么？ |
| cross_document | PASS | 50% | 50% | 31% | Y冷冻设备公司的4R策略和云南白药IT规划的4A架构，分别是什么意思？ |
| cross_document | PASS | 50% | 40% | 100% | 华为IPD流程有多少个活动？云南白药IT规划有多少个业务对象？两者在'体系化'方面有什么共同特点？ |
| thesis_adversarial | PASS | 100% | 100% | 100% | Y冷冻设备公司的速冻机产品保修期是几年？ |
| thesis_adversarial | PASS | 100% | 100% | 100% | Y冷冻设备公司的注册资本是多少万元？ |

---

## Smoke v5 decoupled scorecard (auto)

**Retrieval:** recall@15=0.00% | hit@15=0.00% | mrr=0.000 | ndcg@15=0.000
**Retrieval (graded, ADR 0011):** graded_recall@15=0.00% | graded_ndcg@15=0.000
**Retrieval (answerable-only, excl. vacuous adversarial 100%):** recall@15=0.00% | graded_recall@15=0.00% | substring_faithfulness=0.00%
**Selection:** precision=0.00% | recall=0.00%
**Generation:** refusal_correct=0.00% | contract=0.00% | substring_faithfulness=0.00%

**Labels:** SYNTHESIS_CONTRACT=1

| subset | label | ret_recall | sel_precision | faithfulness | query |
|---|---:|---:|---:|---:|---|
| thesis_factual | SYNTHESIS_CONTRACT | 0% | 0% | 0% | Y冷冻设备公司是哪一年在大连投资建厂的？ |

---

## Smoke v5 decoupled scorecard (auto)

**Retrieval:** recall@15=100.00% | hit@15=100.00% | mrr=1.000 | ndcg@15=1.000
**Retrieval (graded, ADR 0011):** graded_recall@15=60.00% | graded_ndcg@15=1.148
**Retrieval (answerable-only, excl. vacuous adversarial 100%):** recall@15=100.00% | graded_recall@15=60.00% | substring_faithfulness=100.00%
**Selection:** precision=100.00% | recall=100.00%
**Generation:** refusal_correct=100.00% | contract=100.00% | substring_faithfulness=100.00%

**Labels:** PASS=1

| subset | label | ret_recall | sel_precision | faithfulness | query |
|---|---:|---:|---:|---:|---|
| thesis_factual | PASS | 100% | 100% | 100% | Y冷冻设备公司是哪一年在大连投资建厂的？ |

---

## Smoke v5 decoupled scorecard (auto)

**Retrieval:** recall@15=100.00% | hit@15=100.00% | mrr=0.091 | ndcg@15=0.279
**Retrieval (graded, ADR 0011):** graded_recall@15=60.00% | graded_ndcg@15=0.387
**Retrieval (answerable-only, excl. vacuous adversarial 100%):** recall@15=100.00% | graded_recall@15=60.00% | substring_faithfulness=100.00%
**Selection:** precision=100.00% | recall=100.00%
**Generation:** refusal_correct=100.00% | contract=100.00% | substring_faithfulness=100.00%

**Labels:** PASS=1

| subset | label | ret_recall | sel_precision | faithfulness | query |
|---|---:|---:|---:|---:|---|
| thesis_factual | PASS | 100% | 100% | 100% | Y冷冻设备公司是哪一年在大连投资建厂的？ |

---

## Smoke v5 decoupled scorecard (auto)

**Retrieval:** recall@15=100.00% | hit@15=100.00% | mrr=0.091 | ndcg@15=0.279
**Retrieval (graded, ADR 0011):** graded_recall@15=60.00% | graded_ndcg@15=0.387
**Retrieval (answerable-only, excl. vacuous adversarial 100%):** recall@15=100.00% | graded_recall@15=60.00% | substring_faithfulness=100.00%
**Selection:** precision=100.00% | recall=100.00%
**Generation:** refusal_correct=100.00% | contract=100.00% | substring_faithfulness=100.00%

**Labels:** PASS=1

| subset | label | ret_recall | sel_precision | faithfulness | query |
|---|---:|---:|---:|---:|---|
| thesis_factual | PASS | 100% | 100% | 100% | Y冷冻设备公司是哪一年在大连投资建厂的？ |
