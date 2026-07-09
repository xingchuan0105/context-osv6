# ADR-0011: RAG 评测分轨记分卡

| 项目 | 内容 |
|---|---|
| 状态 | **已采纳** |
| 决策日期 | 2026-06-30 |
| 关联 | ADR-0008（Answer Contract）、ADR-0009（Codegen Sandbox Retrieval Bridge）、ADR-0010（LLM 负责指代消解） |

---

## 1. 背景

旧 RAG 质量评测使用三项指标：

1. `Recall@15`
2. `Citation Accuracy`
3. `Hallucination Rate`

但实现中 `Recall@15` 的输入来自 `ChatResponse.citations`，也就是合成器最终选择引用的 chunk，而不是检索器真实返回的 chunk。这把两个不同问题混在一起：

- 检索层是否找到了正确证据
- 合成层是否选择并引用了正确证据

当指标下降时，无法判断该调检索策略、提示词检索查询，还是合成/引用规则。

另一个问题是旧 `Hallucination Rate` 是英文词重叠启发式：`split_whitespace()`、`len()>5`、英文 stopword。中文答案通常没有空格，导致该指标对中文 RAG 基本不可信。

## 2. 决策

**采用分轨记分卡：检索层、选择层、生成层分别评分。**

数据来源：

1. **检索层**：`ChatResponse.tool_results`
   - 读取 `dense_retrieval` / `lexical_retrieval` / `graph_retrieval` / `index_lookup`
   - 支持 `data: [...]` 与 `data: {"chunks": [...]}` 两种形状
   - chunk 字段使用 `chunk_id`、`text` / `content`、`score`
   - 按 `chunk_id` 去重，保留 first-seen 顺序作为有效 rank
2. **选择层**：`ChatResponse.citations`
   - 这是合成器最终选择引用的 chunk
3. **生成层**：`ChatResponse.answer` + cited chunk 正文
   - 评估拒答行为、合约合规、确定性 substring faithfulness

新增模块：

- `tests/rag_quality/src/harness_extract.rs`
- `tests/rag_quality/src/metrics_v2.rs`
- `crates/e2e-analyzer/src/rag_diag.rs`

## 3. 指标

### 3.1 检索层

- `Recall@k`
- `Hit@k`
- `MRR`
- `nDCG@k`（Phase 0 使用二值相关性；Phase 2 引入 relevance grade 后升级）

### 3.2 选择层

- Citation Precision
- Citation Recall

这里的 citation 不再作为检索召回的替身，只衡量合成器是否把已找到的证据选进答案。

### 3.3 生成层

- Refusal Correctness
- Contract Compliance
- Substring Faithfulness

Substring Faithfulness 是确定性快速检查：从答案抽数字、日期、编号、英文/字母数字术语等硬 claim anchor，检查其是否出现在引用 chunk 正文中。

它只用于 smoke / 快速 loop，不替代语义级 LLM-as-Judge。

## 4. Release Gate

硬门槛：

1. 检索层 `Recall@15` 相对 baseline 下降不超过 3%
2. `Refusal Correct = 100%`
3. `Contract Compliance = 100%`

报告但暂不阻断：

1. Citation Precision / Recall
2. Substring Faithfulness
3. nDCG@15

原因：

- Citation Precision 需要更多稳定运行校准
- Substring Faithfulness 只能抓数字/日期/编号等硬幻觉
- nDCG 需要 relevance grade 标注后才适合升为硬门槛

## 5. 诊断标签

每题输出一个优先级诊断标签：

1. `RETRIEVAL_MISS`
2. `SELECTION_MISS`
3. `GENERATION_UNGROUNDED`
4. `SYNTHESIS_CONTRACT`
5. `REFUSAL_WRONG`
6. `PASS`

优先级用于 prompt loop：先修检索，再修选择，再修生成。

## 6. 风险与缓解

### 6.1 tool_results 缺失或数据形状变化

**风险**：如果未来检索工具不再写 `data` chunk 数组，检索层指标会变成 0。

**缓解**：

- `harness_extract.rs` 同时支持数组和 `{"chunks": [...]}` 两种形状
- product-e2e 编译测试覆盖该路径
- `e2e-analyzer rag-diag` 会暴露 `retrieved_count=0`

### 6.2 Substring Faithfulness 误判

**风险**：同义改写、中文数字转阿拉伯数字、OCR 变体会造成误判或漏判。

**缓解**：

- smoke 层只把它作为报告项
- regression 层引入 `FaithfulnessJudge` + `LlmNliJudge`
- Judge 上硬门槛前必须先用人工校准集跑 Cohen's κ

### 6.3 golden schema 逐步升级

**风险**：一次性重写 110+ 条 golden 会拖慢落地。

**缓解**：

- 新字段全部 `#[serde(default)]`
- Phase 0 只引入 `expected_should_answer` 与 `refusal_keywords`
- Phase 2 再补 `must_include`、`must_not_include`、`retrieval_hints`、`difficulty`、`relevance_grades`

