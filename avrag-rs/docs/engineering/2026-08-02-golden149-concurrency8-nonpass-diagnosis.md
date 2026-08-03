# 全量 149 并发 8 首跑 · 12 题非 PASS 诊断（2026-08-02）

> 数据：`crates/app/tests/e2e_output/rag_eval_v2/v2_20260802-143621`（E2E_CONCURRENCY=8，938s，judge=deepseek-v4-flash）。
> 结果：**PASS 137/149（91.9%，历史最高）**；非 PASS 12 题 = PARTIAL×6 + SELECTION_MISS×2 + RETRIEVAL_MISS×2 + UNGROUNDED×1 + INCORRECT×1。
> 对照：07-30 串行基线 PASS 135；08-01 双轮 PASS 122–123。

---

## 0. 数据陷阱先修（读表前必看）

**`per_query.tsv` 的 `n` 列是完成顺序索引（`i+1`），不是黄金集题号。** 并发跑时每题按完成顺序写入 scores，TSV 的 `n` 因此与真实题号错位（如 TSV 第 48 行实为 q050）。**真实题号以 `qNNN.artifact.json` 文件命名为准**。本文全部按 artifact 题号。建议后续修 TSV（用 `score_v2` 内的真实序号或 golden idx）。

---

## 1. 12 题清单（真实题号）

| # | 题号 | subset | label | corr | 一句话 |
|---|---|---|---|---|---|
| 1 | q050 | adr_factual | SELECTION_MISS | 0.0 | 空答：「找到了资料但未能生成符合引用格式的完整答案」，cite=0 |
| 2 | q053 | adr_factual | PARTIAL | 0.7 | ADR-0009 检索方法漏列 rerank/chunk_fetch/doc_summary |
| 3 | q068 | consulting_factual | PARTIAL | 0.8 | 80%/70% 双口径并陈，未给唯一 80% |
| 4 | q078 | ipd_table | SELECTION_MISS | 0.0 | 概念阶段活动数：按名去重 57，gold 逐行 81 |
| 5 | q083 | ipd_table | PARTIAL | 0.7 | 生命终止评审活动号答 PAC-100 但并列 PRO-26 混淆 |
| 6 | q088 | ipd_table | UNGROUNDED | 0.0 | 验证/发布活动数：去重 45/24，gold 逐行 59/30 |
| 7 | q096 | baiyao_pdf | RETRIEVAL_MISS | 0.0 | A 级项目定义：答「战略型」定性，gold 要金额标准；recall=0，act_retr=1 |
| 8 | q105 | cross_document | PARTIAL | 0.8 | 读法①核心命中但类比过度引申（生态锁定同构） |
| 9 | q106 | cross_document | RETRIEVAL_MISS | 0.4 | IPD 活动数 25/348 与业务对象 28 均非 gold（370 / 638）；recall=0 |
| 10 | q123 | chat_builtin_tools | INCORRECT | 0.0 | 计算题 (1587+2933)×1.13：模型 4607.6，gold 5107.6（心算错，未用计算器） |
| 11 | q132 | rag_codegen_channels | PARTIAL | 0.7 | 文档类型字段缺失，未给「咨询/评论类」 |
| 12 | q139 | new_corpus_factual | PARTIAL | 0.8 | 宋向前质疑：乐旋案例三点当主答，模式层面三点当补充，与 gold 组织不符 |

---

## 2. 归因聚类

### A. IPD 表类「行数 vs 去重」口径（q078 / q088 / q106-半）—— 3 题

- **症状**：gold 要求**逐行计数**（q078=81 行、q088=验证 59/发布 30），模型按「活动名去重」（q078=57、q088=45/24）输出。
- **judge 判 UNGROUNDED/SELECTION_MISS 的根**：`faithfulness.unsupported_claims` 指出 **context 里没有「去重后 57/45/24」这个数**——模型自己去重出的数字无支撑；而 gold 的逐行数在 rubric_notes 明确（「306–312 为 7 行同名同描述活动行，需逐行计数」）。
- **执行证据**：q078 budget_exhausted=1、act_retr=183（doc_grep 反复轰、未做 doc_ids 收窄）；q088 act_retr=83。
- **结论**：不是检索不到（recall=1.0），是**计数口径**——模型把「行数」当「活动数」去重，且 SKILL 的 `total_hits 是命中行数` 教学没拦住二次去重。**q088/q079（08-01 已修 total_hits 载体）同类复发**。

### B. 检索缺失 / gold 数字未进上下文（q096 / q106）—— 2 题

- **q096**：act_retr=**1**（几乎没检索就答），recall=0.0。答「战略型/与战略目标直接相关」定性，gold 要**金额标准**（300 万≤投入<500 万或跨一级流程且 100 万≤投入<300 万）。
- **q106**：act_retr=150 但 gold 数字（IPD 370、云南白药 638）未进 citation，recall=0.0。模型用了总览表的 25 / 明细行 348，业务对象答 28（context 里明示 638 也没用）。
- **结论**：这两题是**真检索缺口**（唯一 recall=0 的 2 题）——要么没搜到位，要么搜到了 gold 数字所在行却没选中进 cite。

### C. 双口径并陈 / 多答案混淆（q068 / q083）—— 2 题

- q068：context 里 80%（公众号）与 70%（另一文档）并存，gold 明确要 80%，模型并陈未收敛唯一答案。
- q083：主要答案 PAC-100 正确，但并列引入另一文档的 PRO-26 造成歧义。
- **结论**：多源口径题，模型**并列诚实**但没按 gold 口径收敛到「唯一正确」。

### D. 枚举漏列 / 字段缺失（q053 / q132）—— 2 题

- q053：ADR-0009 检索方法，漏 rerank/chunk_fetch/doc_summary 三个（gold 六个）。
- q132：文档类型字段为空，未给「咨询/评论类」（体裁=文章、语言=中文已对）。
- **结论**：枚举/属性完整性缺失，非检索问题。

### E. 计算错误 + 未用工具（q123）—— 1 题

- (1587+2933)×1.13：模型 4607.6，gold 5107.6。**纯计算题心算错**；`chat_builtin_tools` 本应走 `client.calculator`（G-17 工具闸），模型未用。与 08-01 报告「三件套不用（D11 无提示词教导）」同源。

### F. 空答 / 引用格式失败（q050）—— 1 题

- 答「未能生成符合引用格式要求的完整答案，请尝试重新提问」，cite=0。模型放弃式空答，非检索缺失（recall=1.0）。

### G. 读法/类比过度引申（q105 / q139）—— 2 题

- q105：读法①核心（CRM 23% 份额锁定 + Y 专业化市场）命中，但把「生态锁定」与「服务关系锁定」类比为同构，超出 context 支持。
- q139：gold 要「模式层面三点」（RBF 非股非债/资金成本高/P2P 转嫁），模型把乐旋案例三点当主答、模式三点当补充——要点全含但**组织优先级反了**。

---

## 3. 总判

- **12 题中 9 题 recall=1.0**，只有 q096/q106 是真检索缺口（recall=0）；q050 是放弃式空答。
- **hard fail（corr=0）6 题**：q050（空答）、q078/q088（去重口径）、q096/q106（检索缺口）、q123（心算错）。
- **PARTIAL（corr≥0.7）6 题**：内容基本对，缺**枚举完整性 / 口径收敛 / 字段 / 组织优先级**。
- **共性根因**：多为主观/行为层——①表格题「行数 vs 去重」口径；②多源口径不收敛；③计算题不用工具；④枚举漏项。与 08-01 报告（q079 total_hits、q086 表序、q121 取舍）同族，无新的系统回归。

## 4. 建议（未实施，供决策）

| 优先级 | 动作 | 目标 |
|---|---|---|
| P0 | 表格计数口径：SKILL 强化「行数即计数、勿按名去重」，或 harness 对 `grep.total_hits` 强制信任 | q078/q088/q106 |
| P0 | 计算题强制走 `client.calculator` 提示词教导（D11 三件套缺口） | q123 |
| P1 | 多源口径题：提示「同义词并存时给唯一口径并注明来源」 | q068/q083 |
| P1 | 枚举题：终答前对照文档方法/字段清单查漏 | q053/q132 |
| P2 | 空答护栏：引用格式失败时重试而非放弃 | q050 |
| P2 | per_query.tsv `n` 列改真实题号 | 数据可信度 |

## 5. 速查

- 全量日志：`/tmp/full149_e2ec8_20260802.log`
- v2 产物：`crates/app/tests/e2e_output/rag_eval_v2/v2_20260802-143621/`
- 题级 artifact（真实题号）：`v2_20260802-143621/qNNN.artifact.json`
- label 派生：`tests/rag_quality/src/eval_v2/aggregate.rs::label_for`
- 计数口径教训：`tests/rag_quality/GOTCHAS.md`
