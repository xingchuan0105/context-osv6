# 全量 149 并发 8 · LLM 行为全面诊断（2026-08-02）

> 与《2026-08-02-golden149-concurrency8-nonpass-diagnosis.md》互补：那份覆盖 12 题非 PASS；本文覆盖**全部 149 题**的准确度、检索质量、效率、成功率与行为模式，含「带瑕通过」「隐性检索缺口」等非 PASS 清单之外的隐患。
> 数据源：`crates/app/tests/e2e_output/rag_eval_v2/v2_20260802-143621/`（artifact + judge，judge=deepseek-v4-flash，ok=149/error=0）× `e2e_output/realistic_corpus_full_eval/qNNN.json`（tool_trace / citations / mode_debug，时间戳 08-02 22:36，即本跑）。
> 注意：`per_query.tsv` 的 `n` 列是完成顺序非题号（见 nonpass 诊断 §0），本文一律按 artifact 文件名题号 join。

---

## 1. 总体成绩与趋势

| 指标 | 值 |
|---|---|
| label | **PASS 137/149（91.9%）**，PARTIAL 6，SELECTION_MISS 2，RETRIEVAL_MISS 2，UNGROUNDED 1，INCORRECT 1 |
| correctness 均值 | 0.948（中位 1.0） |
| faithfulness 均值 | 0.966（适用 n=139） |
| relevancy 均值 | 0.989 |
| retrieval recall（full） | 0.961；recall@15 = 0.927 |
| 工具调用 | 均值 9.7 次/题，p50=7，p90=23，max=36 |
| 工具错误 | 6 次 / 149 题（全部自愈，见 §5） |
| 耗时 | 938s（并发 8） |

**趋势**：07-30 串行基线 PASS 135 → 08-01 双轮 PASS 122–123 → 本跑 **137（历史最高）**。
**08-01 行为报告五类问题的收敛对照**：

| 08-01 问题类 | 当时 | 本跑 | 结论 |
|---|---|---|---|
| A. 零检索直答 / 意图叙述泄漏 | 4 题 | **1 题**（q096） | 大幅收敛，未根除 |
| B. 代码块即终答 + 方法名幻觉 | 3 题 | **0** | 已消除 |
| C. 表类检索过载 + 选错 | 6 题 | 3 题（q078/q088/q106） | 减半，仍是最大单族 |
| D. chat 工具三件套不用 | 5 闸 | 1 题（q123） | 基本收敛，余一致性问题 |
| E. JUDGE_ERROR / INFRA | 6 题 | **0** | 已消除 |

---

## 2. 准确度（answer 质量）

### 2.1 correctness 分布（n=149）

| 分段 | 题数 | 说明 |
|---|---|---|
| 1.0（满分） | 130 | |
| 0.7–0.99 | 13 | 6 题 PARTIAL + **7 题带瑕通过** |
| 0.4–0.69 | 1 | q106（0.4） |
| <0.4 | 5 | q050/q078/q088/q096/q123，全部 0.0 |

**带瑕通过（PASS 但 corr<1.0）7 题**：q020(0.9)、q102(0.9)、q104(0.9)、q107(0.9)、q121(0.9)、q126(0.9)、q131(0.95)。judge 口径偏严（漏次要要点即扣分），这些题主答正确；不构成故障，但说明 PASS≠满分，真实满分率是 130/149（87.2%）。

### 2.2 faithfulness（接地性）

- 全量仅 **4 题**出现 `unsupported_claims`：q078、q088（各 2 条，均为「去重后 57/45/24」无 context 支撑）、q096（5 条）、q106（1 条）。其余 145 题全部 grounded。
- 即：**幻觉不是普遍问题**，唯四的失控全部集中在「模型自行统计/推导数字」场景——与表格计数口径族（nonpass 诊断 §2-A）同源。

### 2.3 relevancy

均值 0.989，无一题失焦。答非所问不是本跑的问题。

### 2.4 context_sufficiency<1 的 12 题要拆开看

- **结构性 N/A 5 题**：q039/q041/q042/q045（thesis_adversarial，对抗题 context 本不该含答案）、q134（memory_coreference）。不是缺口。
- **真缺口 7 题**：q068(0.8)、q078、q088、q096、q106(0.7)、q112、q123——与非 PASS 清单高度重合。

---

## 3. 检索质量

### 3.1 recall 缺口：8 题没检全 gold chunk，只有 2 题因此失败

| 题号 | subset | recall | label | 性质 |
|---|---|---|---|---|
| q096 | baiyao_pdf | 0.0 | RETRIEVAL_MISS | 真缺口（且零检索，见 §6-A） |
| q106 | cross_document | 0.0 | RETRIEVAL_MISS | 真缺口（gold 数字行未进 cite） |
| q091 | baiyao_pdf | 0.0 | PASS | **隐性缺口**：gold chunk 全缺，靠其它 chunk 答对 |
| q099 | baiyao_pdf | 0.0 | PASS | 同上 |
| q055 | adr_factual | 0.5 | PASS | 隐性缺口 |
| q077 | ipd_table | 0.67 | PASS | 隐性缺口 |
| q107 | cross_document | 0.5 | PASS | 隐性缺口 |
| q114 | orchestrator_paradigm | 0.67 | PASS | 隐性缺口 |

**baiyao_pdf recall 仅 0.727（21 个 subset 垫底）**：11 题中 3 题 gold chunk 完全未检索到（q091/q096/q099），2/3 靠冗余信息蒙对。这是当前**唯一成建制的检索通道短板**（PDF 类语料的索引/通道问题），建议优先排查该语料的 ingest 与通道覆盖，而不是只看 q096 一题。

### 3.2 排序质量

- ndcg 均值 0.882；recall@15 = 0.927（full recall 0.961 的 3.4pt 差 = 检到了但排在 15 名外）。
- mrr 均值 0.594 **含结构性失真**：30 题 mrr=0 中 13 题是 `golden_count=0`（thesis_adversarial×8、orchestrator_paradigm、chat_builtin_tools 等无 gold chunk 标注的 subset，mrr 对这些题无意义）、17 题 `retrieved_count=0`（纯 chat / 拒答题）。有效题中首位命中（hit@1）73 题。读 mrr 前必须先按 golden_count 过滤，否则结论会被 N/A 稀释。

---

## 4. 效率

### 4.1 工具调用成本

- 分布：p50=7，p90=23，p95=28，max=36（q107，cross_document，PASS）。
- **轻量题（≤3 次调用）26 题**，23 题 PASS——简单事实题一次命中是常态且可靠。
- **重载题（≥15 次）32 题（21.5%）**，29 PASS / 2 PARTIAL / 1 MISS——**过载能换到分，但烧钱**；且同族题成本差异巨大：同为 IPD 计数题，q079 用 28 次调用过了，q078 用 12 次却死在口径上。问题不在调用次数本身，在计数语义。

### 4.2 工具组合（全量 1447 次）

`doc_grep` 611（42%）、`dense_retrieval` 313、`lexical_retrieval` 249、`graph_retrieval` 81、`struct_catalog` 53、`web_search` 46、`doc_profile` 41、`doc_summary` 21、`calculator` 9，其余（session_fs/weather/user_context 等）个位数。

- doc_grep 是最重武器也是最易被抡空的：top 消费者 q113(24 次)、q043(20)、q061(18)——均 PASS，但属「拿次数换覆盖」。
- 分 subset 平均成本最高：cross_adr 21.0、thesis_adversarial 19.2、cross_document 18.8；最低：thesis_factual 5.0、thesis_numeric 5.1。成本结构与题目难度匹配，无异常膨胀的 subset。

### 4.3 上下文经济（retrieved → cited）

- retrieved：p50=9，p90=42，max=169（q043）。cited：p50=1，p90=6，max=17。
- 检索进终答的转化率很低（中位 1/9）。大量 retrieved chunk 只起背景作用——这是现状不是故障，但说明**压缩/早停仍有空间**。
- cited=0 共 20 题，其中 16 题合理（orchestrator 澄清、chat 工具题、纯 chat），**4 题异常且全部失败**：q048(PASS 例外)、q050、q078、q096。「rag 题零引用」仍是一个有效的失败先兆信号。

### 4.4 PASS vs 非 PASS 的成本画像

| 组 | tools 均值 | retrieved 均值 | 答案长度均值 |
|---|---|---|---|
| PASS (137) | 9.6 | 16.9 | 414 字符 |
| 非 PASS (12) | 9.9 | 21.9 | **619 字符** |

失败题不多花钱（调用数几乎相同），但**答案显著更长**——失败模式是「并陈/过度解释/不收敛」（q068 双口径、q105 类比引申、q139 组织颠倒都是长答案），而非「检索不够」。

---

## 5. 成功率与健壮性

### 5.1 分 subset（垫底 8 个）

| subset | PASS | 失分题 |
|---|---|---|
| ipd_table | 9/12 | q078、q083、q088 |
| cross_document | 6/8 | q105、q106 |
| chat_builtin_tools | 4/5 | q123 |
| adr_factual | 10/12 | q050、q053 |
| new_corpus_factual | 5/6 | q139 |
| rag_codegen_channels | 6/7 | q132 |
| baiyao_pdf | 10/11 | q096（另有 q091/q099 隐性缺口） |
| consulting_factual | 13/14 | q068 |

其余 13 个 subset 全 PASS；correctness 满分 subset 14/21。

### 5.2 工具层错误 6 次，5 次自愈

- `doc_summary` Error ×5：q048、q061、q099、q115、q139。**同一工具反复出错，值得单独查因**（5 题中 4 题 PASS 自愈，q139 PARTIAL）。
- `web_fetch` Error ×2：q118（PASS，web_search 冗余兜底）。
- 无 HTTP_500、无 JUDGE_ERROR、无 INFRA_ERROR——链路健康度是三次全量跑里最好的。

### 5.3 特殊行为面

- **expect_no_retrieval 4 题（q132–q135）全部 `refusal.correct=true`**，无一误拒答/误检索。
- **零工具调用 8 题**：q108–q111（orchestrator 澄清，设计行为）、q135（memory 直读）、q144（纯 chat）均正确；异常 2 题——**q096**（rag 已挂载却零检索直答，RETRIEVAL_MISS）与 **q123**（计算题不用 calculator 心算错）。
- **计算器一致性**：q122 与 q148 是同一道题（128×46+357），两次都用 `calculator` 且 PASS；q123（(1587+2933)×1.13）同等条件下**没走工具**心算错。不是能力缺口，是**同类题行为不稳定**——需要确定性手段（G-17 闸或提示词教导）把「算术必走 calculator」钉死。
- **放弃式空答**仅 q050 一例（「未能生成符合引用格式的完整答案」）。

---

## 6. 总判

1. **本跑质量是历史最好，且改善主要来自行为层修复**（08-01 的 A/B/D/E 四类基本收敛），不是 judge 放水或运气。
2. **剩余失败高度集中**：表格计数口径（q078/q088/q106 一族）+ baiyao_pdf 检索通道（q096 明、q091/q099 暗）两块就占了非 PASS 的一半和全部真检索缺口。
3. **幻觉面很窄**：unsupported_claims 只出现在「模型自行统计数字」场景，修计数口径即同时修掉 4 题中全部的 grounding 失控。
4. **效率无系统性浪费**：工具成本与题目难度匹配；真正的问题是**行为一致性**（同族题不同命、同题不同工具决策）和**长答案不收敛**。
5. **建议优先级**（在 nonpass 诊断 §4 基础上，补全量视角证据）：
   - P0 表格计数口径（3 题 + 全部 grounding 失控）；
   - P0 baiyao_pdf ingest/通道排查（recall 0.727，3 题 gold chunk 全缺）——nonpass 诊断只提了 q096，全量数据显示这是**成建制问题**；
   - P0 算术强制 calculator 确定性化（q123）；
   - P1 `doc_summary` Error×5 查因；
   - P1 多源口径收敛 / 枚举查漏（q068/q083/q053/q132，见 nonpass 诊断）；
   - P2 per_query.tsv `n` 列改真实题号；mrr 报表按 golden_count 过滤 N/A。

## 7. 速查

- v2 产物：`crates/app/tests/e2e_output/rag_eval_v2/v2_20260802-143621/`（summary.md / summary.json / qNNN.artifact.json / qNNN.judge.json）
- 行为产物：`crates/app/tests/e2e_output/realistic_corpus_full_eval/qNNN.json`（tool_trace / citations / mode_debug）
- 12 题归因：`docs/engineering/2026-08-02-golden149-concurrency8-nonpass-diagnosis.md`
- 分析中间表（149 行 join 结果）：`/tmp/full149_rows.json`（临时，重跑脚本可再生）
