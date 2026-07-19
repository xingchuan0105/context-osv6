# Realistic RAG Golden Set v2.0 — 设计说明

## 1. 动机

原 `fixtures_golden.json` 的语料是两篇人工精简的概念笔记（`antifragile.txt` 261 词 + `lindy.txt` 246 词，合计 507 词），存在以下问题：

1. **不是真实文档** — 高度压缩的 Markdown 大纲，无版式噪声、表格、长段落。
2. **规模太小** — 全部内容通常只切成一个 chunk，无法测试多 chunk 综合、跨文档推理、长文档定位。
3. **LLM 污染** — 《反脆弱》是知名著作，LLM 训练时见过，无法区分"检索到"还是"背出来的"。
4. **题量不足** — 有效 RAG 题仅 13 条，远低于 PRD 100~500 条目标。

本 golden set 用 7 份真实私有文档解决以上全部问题。

## 2. 语料组成

| 文件 | 类型 | 大小 | 字数 | 用途 |
|---|---|---|---|---|
| `thesis_y_refrigeration.docx` | MBA 学位论文 | 484 KB | 37,473 中文字 + 1,438 英文词 | 主语料，LLM 训练集未收录 |
| `adr-0004-rag-agent-loop.md` | 架构决策记录 | 4.8 KB | 541 英文词 | 技术文档检索 |
| `adr-0009-codegen-sandbox-bridge.md` | 架构决策记录 | 13.6 KB | 1,029 中文字 + 英文 | 技术文档检索 |
| `consulting_platform_network_effects.docx` | 咨询文章 | 45 KB | 15,567 中文字 | 商业/管理文档检索 |
| `consulting_compensation_design.docx` | 咨询文章 | 91 KB | 2,705 中文字 | 薪酬管理案例检索 |
| `huawei_ipd_370_activities.xlsx` | 表格 | 90 KB | 372 行 × 6 列 | 结构化表格数据检索 |
| `baiyao_it_planning.pdf` | PDF 方案书 | 1.9 MB | ~3,175+ 中文字 | PDF 解析+复杂版式检索 |

所有文件已复制到 `crates/app/tests/product_e2e/fixtures/`。

### 2.1 为什么选这篇论文

- **私有性**：作者本人的 MBA 论文，LLM 训练集未收录，检索信号干净。
- **真实性**：含版式、表格、引用、多层级标题，接近生产环境文档。
- **规模适中**：3.7 万字，会被切成数十个 chunk，能真实测试多 chunk 检索与综合。
- **内容丰富**：有具体数字（营收、市场份额、增长率）、人名、地名、理论框架，适合构造多种题型。
- **中英混合**：正文中文，摘要/图表标题英文，测试中英混合检索。

### 2.2 为什么加 ADR

- 技术文档与论文风格完全不同（代码引用、结构化表格、设计决策），覆盖更多文档类型。
- ADR 是项目自有文档，LLM 未见过具体内容。
- 两篇 ADR 有关联（ADR-0009 引用 ADR-0007，与 ADR-0004 同属 RAG 架构），可构造跨文档题。

### 2.3 为什么加咨询文档

- **主题多样性**：网络效应/平台经济、薪酬管理、华为IPD流程、云南白药IT规划，4 个完全不同领域，避免"同主题检索捷径"。
- **格式多样性**：DOCX（叙述体）、XLSX（372行表格）、PDF（1.9MB复杂版式），覆盖生产环境主要文档格式。
- **干扰文档**：7 份文档可作为彼此的干扰项，测试 doc_scope 过滤——问薪酬问题时不应返回论文内容。
- **表格检索**：华为IPD的370个活动是纯表格数据，测试RAG对结构化数据的解析和检索能力。
- **PDF压力测试**：云南白药方案书含架构图、多列表格、多列排版，测试PDF解析的鲁棒性。

## 3. Golden Set 结构

共 **107 题**，10 个子集：

| 子集 | 题数 | 考察目标 |
|---|---|---|
| `thesis_factual` | 15 | 单事实查找（人名、日期、数字、产品名） |
| `thesis_synthesis` | 10 | 跨章节综合（多 chunk 检索 + 信息整合） |
| `thesis_numeric` | 12 | 精确数字提取（营收、份额、增长率、计算结果） |
| `thesis_adversarial` | 8 | 对抗题（语料中无答案，考察拒答能力） |
| `adr_factual` | 12 | 技术文档事实查找（组件名、设计决策、方法名） |
| `cross_adr` | 5 | 跨文档推理（同时需要两篇 ADR 的信息） |
| `consulting_factual` | 14 | 咨询文章事实查找（网络效应、薪酬、票房数据） |
| `ipd_table` | 12 | 表格数据检索（IPD阶段、活动号、角色） |
| `baiyao_pdf` | 11 | PDF文档检索（4A架构、项目分级、通过率指标） |
| `cross_document` | 8 | 跨多文档推理（主题关联、概念消歧、策略对比） |

### 3.1 题型设计原则

1. **细节优先**：问"2019年营收550万"而非"公司是做什么的"，确保必须检索才能答对。
2. **source_chunks 用精确子串**：所有 substring 已验证存在于语料中。
3. **对抗题用" plausible but absent"事实**：问注册资本、韩方投资者姓名、竞争对手营收等文中未提及的细节，但听起来像论文应该有的内容。
4. **跨文档题需要多篇文档的信息**：如"4R策略和4A架构分别是什么"，测试多文档检索和消歧能力。
5. **数字题分散在不同章节和文档**：论文市场分析、IPD活动数、白药业务对象数，测试跨文档定位精度。
6. **表格题按行/列检索**：IPD表格问"活动号PAC-05是什么活动"，测试结构化数据解析。

### 3.2 对比原 golden set

| 维度 | 原 `fixtures_golden.json` | 新 `golden_set_realistic.json` |
|---|---|---|
| 语料规模 | 507 词 / 3.3 KB | ~63,000 字 / ~2.6 MB |
| 文档类型 | 2 篇概念笔记 | 1 论文 + 2 ADR + 2 咨询DOCX + 1 XLSX + 1 PDF |
| 格式覆盖 | 仅 TXT | DOCX + MD + XLSX + PDF |
| LLM 污染 | 高（知名著作） | 无（全部私有文档） |
| 有效 RAG 题数 | 13 | 107 |
| 题型覆盖 | 单事实 + 对抗 | 单事实 + 综合 + 数字 + 对抗 + 技术文档 + 跨文档 + 表格 + PDF + 跨多文档 |
| chunk 数量 | ~2 | ~数百个（7份真实文档 chunking） |
| PRD §13.2 目标 | 100~500 条 | 107 条 ✅ 达标 |

## 4. 使用方法

### 4.1 加载

```rust
let path = Path::new(env!("CARGO_MANIFEST_DIR"))
    .join("../../tests/rag_quality/golden_set_realistic.json");
let dataset = GoldenDataset::load(&path).expect("load realistic golden set");
```

### 4.2 语料准备

测试前需将七份 fixture 文件上传到测试环境的文档系统：

```rust
let ctx = TestContext::new_with_real_llm().await;
// 上传论文（大文件，需较长 ingestion 时间）
let thesis = ctx.upload_document("thesis_y_refrigeration.docx").await?;
ctx.wait_for_ingestion(&thesis.document_id, Duration::from_secs(600)).await?;
// 上传 ADR
let adr4 = ctx.upload_document("adr-0004-rag-agent-loop.md").await?;
ctx.wait_for_ingestion(&adr4.document_id, Duration::from_secs(120)).await?;
let adr9 = ctx.upload_document("adr-0009-codegen-sandbox-bridge.md").await?;
ctx.wait_for_ingestion(&adr9.document_id, Duration::from_secs(120)).await?;
// 上传咨询文档
let consulting = ctx.upload_document("consulting_platform_network_effects.docx").await?;
ctx.wait_for_ingestion(&consulting.document_id, Duration::from_secs(300)).await?;
let comp = ctx.upload_document("consulting_compensation_design.docx").await?;
ctx.wait_for_ingestion(&comp.document_id, Duration::from_secs(120)).await?;
// 上传表格
let ipd = ctx.upload_document("huawei_ipd_370_activities.xlsx").await?;
ctx.wait_for_ingestion(&ipd.document_id, Duration::from_secs(120)).await?;
// 上传 PDF
let baiyao = ctx.upload_document("baiyao_it_planning.pdf").await?;
ctx.wait_for_ingestion(&baiyao.document_id, Duration::from_secs(600)).await?;
```

### 4.3 跑评估

```bash
# 需要真实 LLM + embedding API + Milvus + PG
E2E_MODE=nightly cargo test -p app --test product_e2e rag_quality_prod \
  --features product-e2e -- --ignored --test-threads=1 --nocapture
```

## 5. 已知限制与后续工作

1. **幻觉指标仍是词重叠启发式**：需替换为 NLI/LLM-as-judge（见 `GOTCHAS.md`）。
2. **DOCX 解析依赖**：论文/咨询文档需经 LiteParse/Office parser 正确解析，表格内容可能丢失。
3. **PDF 解析依赖**：云南白药 PDF 含架构图和多列表格，pdftotext 提取的文本可能有乱序，需验证 LiteParse/视觉PDF路径的解析质量。
4. **XLSX 解析依赖**：华为IPD表格需确认 Office parser 能正确提取行/列结构，否则表格题 recall 会很低。
5. **对抗题 expected_answer 是中文拒答**：当前 `hallucination_check` 的 refusal pattern 需包含中文模式（如"未提及""未说明"）。
6. **规模已达标**：107 条满足 PRD §13.2 的 100~500 条下限，后续可根据运行结果继续扩充到 200+。
7. **生产测试的 5 条限制**：`rag_quality_prod.rs` 的 `take(5)` 需在本 golden set 验证稳定后移除，改为跑全部 107 条。
8. **跨文档题的 doc_scope**：`cross_document` 子集需要同时检索多份文档，测试时 doc_scope 需包含所有 7 份文档的 ID。

---

## 6. v3.0.0-orchestrator（2026-07-19，对齐 Orchestrator 新范式）

### 6.1 为什么升级

产品面已从「三 agent 显式切换」改为 **能力标签（RAG / Search 多选，不选=纯聊天）+ Orchestrator（ReAct）→ Worker 取证 → Chat exit 唯一出口**。v2 的 `mode:"rag"` 只能经 `agent_type` 兼容路径单通道，表达不了新范式的关键行为。v3 在**不动 107 道旧题语义**的前提下补齐新范式覆盖。

### 6.2 新字段（runner 可选消费，旧 golden 文件语义不变）

| 字段 | 默认 | 含义 |
|---|---|---|
| `capabilities` | `[]` | 能力标签：`["rag"]` / `["search"]` / `["rag","search"]` / `[]`（纯聊天）。空时按旧 `mode` 映射（rag/search→单能力，其余→纯聊天），见 `GoldenExample::resolved_capabilities` |
| `doc_scope_hint` | `"all"` | 该题的文档范围：`all`（全语料，旧 runner 行为）/ `empty`（空选择规则）/ `thesis` / `adr_pair` / `consulting_platform` / `consulting_compensation` / `ipd` / `baiyao` |
| `expect_citations` | `null` | 新引用协议的类型化下限 `{min_doc, min_web}`（doc=`[[cite:chunk]]`，web=`[[web:…]]`，单计数器）。`null` 时只用旧 `expected_citations` 存在性语义 |
| `requires_network` | `false` | 需要 Brave 外网（联合题）。`E2E_SKIP_NETWORK_CASES=1` 时 runner 跳过 |

### 6.3 新增子集 `orchestrator_paradigm`（12 题，总计 107→**119**）

| 组 | 题数 | 验证点 |
|---|---|---|
| 空选择规则 | 2 | `capabilities:["rag"]` + 空 scope → 引导选择文档；`must_not_include` 语料专有名词兜底「全库注入」 |
| 纯聊天边界 | 2 | `capabilities:[]` → 不触发检索，答案零引用 |
| RAG+Search 联合 | 4 | 语料内框架 vs 外部最佳实践（4R/4C、IPD/Stage-Gate、4A/TOGAF、网络效应 70%），要求 `min_doc≥1 且 min_web≥1` |
| 跨文档张冠李戴 | 2 | A 文档事实安到 B 文档头上；scope 限定 B → 必须拒答（证据库不越界） |
| 多 chunk 覆盖 | 2 | 四维营销策略（产品/价格/渠道/宣传）、白药三阶段时间表，要求多 chunk 证据 |

### 6.4 runner 变更（`realistic_corpus_full_eval`）

- 按 `doc_scope_hint` 逐题解析范围（`resolve_doc_scope`），不再一律全语料；
- 请求带 `capabilities`（`TestContext::chat_with_capabilities`，`post_rag_chat` 增加可选参数）；
- `expect_citations` 按 citations 数组统计 doc/web 计数，不达标记入 failures（calibration 语义，不硬门）；
- `requires_network` + `E2E_SKIP_NETWORK_CASES=1` → 跳过联网题。

### 6.5 语料原文档地址

| 文档 | 项目内地址（canonical） | 库外原件 |
|---|---|---|
| `thesis_y_refrigeration.docx` | `crates/app/tests/product_e2e/fixtures/`（另有同名 `.txt` 抽取对照） | 未在本机检出（作者私有 MBA 论文，OneDrive 知境笔记目录无） |
| `adr-0004-rag-agent-loop.md` | 同上 | `avrag-rs/docs/adr/0004-rag-agent-loop-native-tools.md`（逐字节一致） |
| `adr-0009-codegen-sandbox-bridge.md` | 同上 | `avrag-rs/docs/adr/0009-codegen-sandbox-retrieval-bridge.md`（逐字节一致） |
| `consulting_platform_network_effects.docx` | 同上（+`.txt`） | 未在本机检出 |
| `consulting_compensation_design.docx` | 同上（+`.txt`） | 未在本机检出 |
| `huawei_ipd_370_activities.xlsx` | 同上（+`.txt`） | 未在本机检出 |
| `baiyao_it_planning.pdf` | 同上（+`.txt`） | 未在本机检出 |

> 注意：runner 当前上传的是 `.txt` 抽取版（office parser 离线时可跑）；要测完整多模态管线，起 office parser 后把 `corpus_files` 切回 DOCX/XLSX/PDF 原件。
