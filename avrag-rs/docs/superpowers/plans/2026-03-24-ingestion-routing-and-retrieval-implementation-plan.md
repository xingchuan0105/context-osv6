# 文件解析路由与检索召回优化实施计划

> **状态**: Draft for Review
> **2026-04-26 文档状态**: Historical plan. Parser routing and text/multimodal evidence ideas remain useful; Qdrant/Tantivy/PG BM25 storage targets are superseded by [Current Product Architecture](/home/chuan/context-osv6/avrag-rs/docs/superpowers/specs/2026-04-26-current-product-rag-architecture.md).
> **关联 ADR**: `docs/adr/0002-ingestion-routing-and-retrieval.md`
> **执行原则**: 先审阅计划，再按 Phase 顺序实施；在未批准前，不进入代码落地。

**Goal:** 将当前仅支持文本主链路的 ingestion / retrieval 架构，升级为“本地轻量解析主路 + MinerU 精准解析兜底 + 统一图文中间模型 + 文本/图文双路召回 + 汇合重排”的可生产化方案。

**Architecture:** 上游引入 `ParseRouter + ParseProbe + MinerU client + ParsedUnit`，中游引入文本块/图文块双实体与双向量写入，下游在 `planner -> retrieval -> rerank -> answer context` 中显式区分文本证据与图文证据，但最终仍汇合成统一的 evidence pool 提供给回答阶段。

**Tech Stack:** Rust (`ingestion`, `storage-pg`, `storage-qdrant`, `rag-core`, `llm`), PostgreSQL, Qdrant, S3/Object Storage, MinerU Precise Parse API, text-splitter, 多模态 embedding / rerank model.

---

## 0. 审阅重点

在执行前，希望你重点审下面 6 个点：

1. 是否接受“**双实体 + 双 collection**”作为 MVP 结构，而不是一步到位做单表/单 collection 抽象。
2. 是否接受“**PPT 与纯图片直接走 MinerU**、PDF 走探针后再决策”的路由策略。
3. 是否接受 `ParsedUnit` 作为新的 ingestion 中间模型，并逐步替换 `ParsedDocument.pages -> ParsedPreviewItem` 的直连方式。
4. 是否接受“**文本主路继续维持现有 text embedding + BM25**，图文辅路新增多模态 embedding”，而不是把两者强行统一成单一 embedding。
5. 是否接受“**planner 明确输出视觉检索意图**”，让视觉召回只在需要时触发。
6. 是否接受“**阶段上线**”，即先打通解析与入库，再接多模态召回，最后接统一重排与 answer context，而不是一次性全改。

---

## 1. 成功标准

### 1.1 功能标准

- `.ppt/.pptx`、扫描 PDF、纯图片文档能正确走 MinerU 精准解析。
- MinerU 返回的图片不会被丢弃，能够以图文块形式落库。
- 文本块继续支持 BM25 + dense 检索，不回退当前文本 RAG 能力。
- 图文类问题能够召回图片上下文块，而不是只能命中文本描述。
- answer 阶段能区分文本证据与图文证据，并保留 `chunk_id / asset_id / page / retrieval_channel`。

### 1.2 工程标准

- 不保留“新的主链路已经上线、旧实现继续并行长期存活”的废案代码。
- 每个阶段结束时都有单元测试、集成测试和一组固定样本文档验收。
- 多模态能力是增量接入，不影响现有纯文本文档吞吐。
- 对 MinerU 调用、对象存储资产写入、多模态向量写入建立可观测日志和失败诊断。

### 1.3 运维标准

- 能统计本地解析命中率、MinerU 命中率、MinerU 平均耗时、图文召回命中率。
- 当 MinerU 不可用时，系统不会崩；但也不会伪装成“已完成高质量解析”。文档状态与审计必须反映真实退化情况。

---

## 2. 范围与非目标

### 2.1 本计划范围内

- ingestion 路由升级
- MinerU Precise Parse API 集成
- 统一图文中间模型
- 文本块 / 图文块双入库
- 文本向量 / 多模态向量双写入
- planner / retrieval / rerank / answer context 的多模态升级
- 观测指标、样本集、E2E 验收脚本

### 2.2 本计划范围外

- 前端图片证据卡片与图像预览 UI
- 模型供应商长期选型策略
- 全量历史文档重建与回灌批处理系统
- 图片 OCR/VLM prompt 的产品化微调

---

## 3. 当前现状映射

### 3.1 当前 ingestion 主链

- `crates/ingestion/src/parser/mod.rs`
  - `ParserFactory` 仅按扩展名选择本地 parser
- `crates/ingestion/src/parser/*.rs`
  - 已有 `pdf / office / html / text / code` 解析器
- `crates/ingestion/src/chunker.rs`
  - 只为文本构造 `ParsedPreviewItem`
  - 已使用 `text-splitter`
  - 文本预算为 `512 token`
- `bins/worker/src/main.rs`
  - 负责 parser -> chunk -> PostgreSQL -> Qdrant 文本向量写入

### 3.2 当前 retrieval 主链

- `crates/rag-core/src/retrieval.rs`
  - 已有 sparse / dense 文本检索封装
- `crates/rag-core/src/runtime.rs`
  - 已有 hybrid 文本检索、RRF、rerank、answer synthesizer 主流程
- `crates/rag-core/src/context.rs`
  - 当前 context 拼装仍是文本证据中心

### 3.3 当前核心缺口

- 无复杂文档路由探针
- 无 MinerU 集成
- 无图文统一中间模型
- 无图片资产入库与多模态 chunk 入库
- 无多模态 embedding / retrieval / rerank
- 无图文 evidence index

---

## 4. 实施策略

采用 **四阶段逐层推进**，每阶段都只解决一个清晰边界的问题，并在阶段结束时收口：

1. **Phase 1: 解析路由与统一图文模型**
2. **Phase 2: 存储与向量化双路打通**
3. **Phase 3: 检索、重排与回答上下文升级**
4. **Phase 4: 验收、观测与收尾清理**

每个阶段结束后，先验证、再合并、再进入下一阶段，不跨阶段并行铺摊子。

---

## 5. Phase 1: 解析路由与统一图文模型

**目标:** 在不改 retrieval 的前提下，先把上游“文档 -> 统一图文单位”的路径打通。

### 5.1 任务 1: 引入 ParseProbe 与 ParseRouter

**Files:**
- Create: `crates/ingestion/src/parser/probe.rs`
- Create: `crates/ingestion/src/parser/router.rs`
- Modify: `crates/ingestion/src/parser/mod.rs`
- Modify: `bins/worker/src/main.rs`

- [ ] **Step 1: 定义探针结果与路由决策结构**

新增：
- `ParseProbeResult`
- `ParseRoute`
- `RouteReason`

要求：
- 探针结果必须能表达 `likely_scanned / likely_presentation / image_hint_count / table_hint_count / extracted_text_chars`。
- 路由决策必须保留“为什么走本地 / 为什么走 MinerU”的解释，便于日志与调试。

- [ ] **Step 2: 实现 PDF / 图片 / PPT 基础路由规则**

规则：
- 纯文本/代码: `Local`
- 图片: `MineruPrecise`
- PPT/PPTX: `MineruPrecise`
- PDF: 先 probe 再决策
- Office: 第一版默认 `Local`

- [ ] **Step 3: Worker 接入 router，而不是直接按扩展名选 parser**

`worker` 中的解析入口改为：
1. 构造 route request
2. route -> `Local` / `MineruPrecise`
3. 调用对应 executor
4. 统一输出归一化结果

### 5.2 任务 2: 引入统一图文中间模型 ParsedUnit

**Files:**
- Modify: `crates/ingestion/src/parser/mod.rs`
- Modify: `crates/ingestion/src/chunker.rs`
- Modify: `bins/worker/src/main.rs`

- [ ] **Step 1: 定义 `ParsedUnitKind` / `ParsedUnit` / `NormalizedDocument`**

建议结构：
- `ParsedUnitKind::{Text, ImageWithContext}`
- `ParsedUnit`
- `NormalizedDocument { title, units, metadata }`

- [ ] **Step 2: 为本地 parser 增加归一化层**

原则：
- parser 仍可输出 `ParsedDocument`
- 新增 normalizer 将 `ParsedDocument.pages` 转为 `ParsedUnit`
- 当前本地 parser 如果拿不到图片结构，也要保留 hook，先只生成 `Text` 单元

- [ ] **Step 3: 明确图文块的文本规范**

`ImageWithContext.text` 必须是：
- `caption`
- `context`
- 必要时附加局部标题

目标不是给人读，而是给 BM25 fallback 与 answer context 提供稳定检索文本。

### 5.3 任务 3: MinerU client 与结果归一化

**Files:**
- Create: `crates/ingestion/src/parser/mineru.rs`
- Modify: `crates/ingestion/src/parser/mod.rs`
- Modify: `bins/worker/src/main.rs`
- Modify: `Cargo.toml` / `crates/ingestion/Cargo.toml`

- [ ] **Step 1: 定义 MinerU 配置结构**

环境变量建议：
- `MINERU_BASE_URL`
- `MINERU_API_KEY`
- `MINERU_TIMEOUT_MS`
- `MINERU_CALLBACK_URL`（如后续需要）

- [ ] **Step 2: 实现 Precise Parse API client**

能力要求：
- 上传文件
- 查询任务状态
- 拉取 Markdown 结果
- 拉取图片列表/图片目录元数据

- [ ] **Step 3: 实现 MinerU 结果归一化**

归一化输出必须包括：
- Markdown 切分出的文本单位
- 图片及其附近段落构造出的 `ImageWithContext`
- `parser_backend = mineru_precise`

- [ ] **Step 4: 对象存储回写图片资产**

要求：
- MinerU 返回的图片资产不能只留外部 URL
- 必须统一回写到系统对象存储或受控路径
- 每个资产需要稳定的 `asset_id`

### 5.4 Phase 1 验收标准

- 能用至少 4 种样本跑通：
  - 纯文本 PDF
  - 扫描 PDF
  - PPTX
  - PNG/JPG 图片文档
- Worker 日志能看到明确 route 决策和 route reason
- NormalizedDocument 中出现 `Text` 和 `ImageWithContext` 两类单元
- 失败路径会明确报“解析失败 / 外部服务失败 / 结果归一化失败”，不静默吞掉

### 5.5 Phase 1 Review Gate

请你审：
- `ParsedUnit` 字段是否足够
- PDF 探针阈值是否要更保守或更激进
- 图片/PPT 是否直接 MinerU，有无例外想保留

---

## 6. Phase 2: 存储与向量化双路打通

**目标:** 让文本块和图文块都能被可靠持久化，并写入各自向量索引。

### 6.1 任务 1: PostgreSQL schema 扩展

**Files:**
- Create: `migrations/*_document_assets.up.sql`
- Create: `migrations/*_document_assets.down.sql`
- Create: `migrations/*_document_multimodal_chunks.up.sql`
- Create: `migrations/*_document_multimodal_chunks.down.sql`
- Modify: `crates/storage-pg/src/lib.rs`

- [ ] **Step 1: 新增 `document_assets` 表**

字段建议：
- `asset_id`
- `org_id`
- `workspace_id`
- `document_id`
- `page`
- `asset_kind`
- `storage_path`
- `mime_type`
- `width`
- `height`
- `caption`
- `parser_backend`
- `created_at`

- [ ] **Step 2: 新增 `document_multimodal_chunks` 表**

字段建议：
- `chunk_id`
- `org_id`
- `workspace_id`
- `document_id`
- `asset_id`
- `page`
- `context_text`
- `caption`
- `normalized_text`
- `parser_backend`
- `metadata`
- `created_at`

- [ ] **Step 3: Repository API 扩展**

新增：
- `store_document_assets(...)`
- `store_document_multimodal_chunks(...)`
- `get_multimodal_chunk_by_id(...)`
- `search_multimodal_chunks_*`（如后续需要 PG fallback）

### 6.2 任务 2: chunker 分路输出

**Files:**
- Modify: `crates/ingestion/src/chunker.rs`
- Modify: `crates/ingestion/src/lib.rs`
- Modify: `bins/worker/src/main.rs`

- [ ] **Step 1: 文本块与图文块分开构造**

建议将当前 `build_chunk_items()` 拆成：
- `build_text_chunk_items()`
- `build_multimodal_chunk_items()`
- 或 `build_chunk_plan()` 返回结构化结果

- [ ] **Step 2: 文本块继续走 `text-splitter`**

保持：
- `512 token`
- markdown/code 分支继续可用
- overlap 保持低重叠

- [ ] **Step 3: 图文块只切 context，不切图片**

规则：
- 以单张图片为单位
- context 过长则局部裁剪
- 保留 `caption + context` 组合文本
- 不把多张图片混成一个多模态 chunk

### 6.3 任务 3: 双向量写入

**Files:**
- Modify: `bins/worker/src/main.rs`
- Modify: `crates/storage-qdrant/src/*`
- Modify: `crates/app/src/lib.rs`（配置透传）

- [ ] **Step 1: 文本向量保留现状**

- collection: `chunks_text`
- 模型: 现有 text embedding model

- [ ] **Step 2: 新增多模态 embedding client 配置**

环境变量建议：
- `AVRAG_MM_EMBEDDING_BASE_URL`
- `AVRAG_MM_EMBEDDING_API_KEY`
- `AVRAG_MM_EMBEDDING_MODEL`
- `AVRAG_MM_EMBEDDING_DIM`

- [ ] **Step 3: 新增 `chunks_multimodal` collection 写入逻辑**

写入 payload 至少包含：
- `chunk_id`
- `document_id`
- `asset_id`
- `page`
- `caption`
- `parser_backend`

### 6.4 Phase 2 验收标准

- 同一文档可同时产生文本块和图文块
- PostgreSQL 与 Qdrant 中都能看到对应双路数据
- 任何一路写入失败时，worker 报错能指向具体阶段
- 纯文本文档仍只写文本块，不会被强行走多模态链路

### 6.5 Phase 2 Review Gate

请你审：
- PG 新表字段是否够用
- 是否坚持双 collection 方案
- 多模态 chunk 的 `normalized_text` 是否还需额外字段

---

## 7. Phase 3: 检索、重排与回答上下文升级

**目标:** 让图文证据真正参与回答，而不是只存在于存储层。

### 7.1 任务 1: Planner 增加视觉检索意图

**Files:**
- Modify: `crates/llm/src/*planner*`
- Modify: `crates/rag-core/src/runtime.rs`
- Modify: `crates/common/src/lib.rs`

- [ ] **Step 1: 扩展 planner 输出结构**

新增字段：
- `needs_visual_evidence: bool`
- `text_queries: Vec<String>`
- `visual_queries: Vec<String>`

- [ ] **Step 2: 明确触发条件**

视觉召回触发信号：
- 查询显式提到图/图表/截图/PPT/页面/界面/结构图
- 会话上下文已聚焦某张图或某份演示文稿
- 某些文档类型本身以图形证据为主

### 7.2 任务 2: retrieval 扩展为 text + multimodal

**Files:**
- Modify: `crates/rag-core/src/retrieval.rs`
- Modify: `crates/rag-core/src/runtime.rs`
- Modify: `crates/storage-qdrant/src/*`

- [ ] **Step 1: 保留文本 hybrid 主路**

文本主路：
- BM25 Top 50
- dense Top 50
- RRF -> Top 30

- [ ] **Step 2: 新增 multimodal retrieval primitives**

新增：
- `run_multimodal_retrieval(...)`
- 多模态 hit 结构
- 多模态 payload -> multimodal scored chunk 映射

- [ ] **Step 3: runtime 汇合双路候选**

候选池建议：
- 文本 Top 30
- 图文 Top 20
- 汇总后最多 50 个候选

### 7.3 任务 3: rerank 升级为两层

**Files:**
- Modify: `crates/rag-core/src/runtime.rs`
- Modify: `crates/llm/src/*rerank*` 或新增 client

- [ ] **Step 1: 文本候选保留轻量 rerank**
- [ ] **Step 2: 图文候选保留原始多模态相似度与元数据**
- [ ] **Step 3: 汇总后接统一 evidence judge / multimodal rerank**

注意：
- 第一版可以先不上全功能 VLM rerank，但必须为该层预留接口。
- 若第一版只做“文本 rerank + 图文原始分数并入排序”，也要在计划里明确这是临时过渡，不是最终终态。

### 7.4 任务 4: answer context 与 citation 协议升级

**Files:**
- Modify: `crates/rag-core/src/context.rs`
- Modify: `crates/llm/src/synthesizer.rs`
- Modify: `crates/common/src/lib.rs`

- [ ] **Step 1: evidence index 增加图文字段**

建议字段：
- `chunk_id`
- `doc_id`
- `chunk_type`
- `page`
- `retrieval_channel`
- `asset_id`
- `caption`
- `image_path`
- `context_excerpt`

- [ ] **Step 2: answer prompt 能识别图文证据来源**

要求：
- 模型知道哪些证据来自文本块
- 哪些证据来自图片上下文块
- 回答时可据此判断置信度和引用粒度

- [ ] **Step 3: citation data contract 扩展**

即使前端暂时不用，也要在后端 contract 中保留图文 citation 能力。

### 7.5 Phase 3 验收标准

- 图表类问题能命中图文块
- 文本问题不受图文链路干扰
- answer stage 可以输出带图文来源的 citation 元数据
- runtime trace 能区分 text retrieval 与 multimodal retrieval

### 7.6 Phase 3 Review Gate

请你审：
- visual retrieval 是否只在 planner 标记下触发
- 第一版是否允许“无 VLM rerank 的过渡方案”
- citation contract 是否还要加更多图像位置信息

---

## 8. Phase 4: 验收、观测与收尾清理

**目标:** 把功能闭环、测试和可观测性补齐，并清掉被新架构替代的废案代码。

### 8.1 任务 1: 测试矩阵

**Files:**
- Create: `tests/fixtures/docs/*`
- Create/Modify: `tests/*` / `crates/*/tests`
- Modify: `docs/runbooks/worker-dev.md`

- [ ] **Step 1: 单元测试**

覆盖：
- parse probe
- route 决策
- ParsedUnit normalizer
- text chunk builder
- multimodal chunk builder
- multimodal payload mapping

- [ ] **Step 2: 集成测试**

覆盖：
- worker 文档摄取全链路
- PG 双表写入
- Qdrant 双 collection 写入
- retrieval 汇合逻辑

- [ ] **Step 3: 样本验收集**

准备至少 6 类样本：
- 纯文本 PDF
- 扫描 PDF
- PPTX
- 图片文档
- HTML 混排文档
- 代码/Markdown 文档

### 8.2 任务 2: 观测与指标

**Files:**
- Modify: `bins/worker/src/main.rs`
- Modify: `crates/rag-core/src/runtime.rs`
- Modify: `docs/runbooks/worker-dev.md`

- [ ] **Step 1: 摄取指标**

记录：
- route = local / mineru_precise
- parse latency
- normalize latency
- asset count
- multimodal chunk count

- [ ] **Step 2: 检索指标**

记录：
- text recall count
- multimodal recall count
- multimodal selected count
- answer citation by type

### 8.3 任务 3: 清理废案代码

- [ ] **Step 1: 删除被新链路替代、且不会再被启用的旧辅助路径**
- [ ] **Step 2: 删除误导性的 fallback 注释与无效配置**
- [ ] **Step 3: 更新 runbook / env example / 配置说明**

原则：
- 不把“已经废弃的并行旧实现”留在主干中长期共存。
- 只保留真正用于降级保护的可验证逻辑，不保留废案。

### 8.4 Phase 4 验收标准

- 测试矩阵跑通
- Runbook 更新完成
- 旧废案代码清理完成
- 日志和 trace 足够支持线上排查

---

## 9. 风险与应对

### 风险 1: MinerU 成本和耗时不可控

应对：
- 用 probe 严格限制 MinerU 触发面
- 保留 route metrics
- 先只对图片/PPT/复杂 PDF 启用

### 风险 2: 多模态 chunk 质量不稳定

应对：
- 先以 `caption + local context` 的最小结构落地
- 减少过度复杂的图文拼装逻辑
- 用固定样本做回归

### 风险 3: retrieval 复杂度突然上升

应对：
- 先分 collection，避免单索引复杂化
- visual retrieval 只在明确需要时启用
- 第一版先做可用，再做高级 rerank

### 风险 4: answer prompt 无法稳定消费图文证据

应对：
- evidence index 中显式标明 `chunk_type` 和 `retrieval_channel`
- 先做 trace 可观测，再做 prompt 微调

---

## 10. 推荐执行顺序

建议按下面顺序推进，不建议打乱：

1. `Phase 1 / Task 1-2`
   先把 route、probe、ParsedUnit 打通
2. `Phase 1 / Task 3`
   再接 MinerU client 与结果归一化
3. `Phase 2 / Task 1-2`
   再扩 PG schema 与 chunk builder
4. `Phase 2 / Task 3`
   再接多模态 embedding 与 Qdrant 双写
5. `Phase 3 / Task 1-2`
   再升级 planner 与 retrieval
6. `Phase 3 / Task 3-4`
   最后升级 rerank、answer context、citation
7. `Phase 4`
   最后补测试、指标、清理

---

## 11. 批准后第一批落地范围建议

如果你准备批准执行，我建议**第一批只做到 Phase 1 完成**，原因是：

- 它能最快验证 MinerU 路由与统一图文模型是否合适
- 一旦 `ParsedUnit` 定型，后面的存储与 retrieval 改造会顺很多
- 它避免一开始就同时动 parser、数据库、Qdrant、runtime 四层，风险最小

第一批完成后的产物应是：
- route + probe 生效
- MinerU precise parse 可调用
- NormalizedDocument / ParsedUnit 定型
- worker 能输出文本/图文归一化结果
- 尚未进入 retrieval 改造

---

## 12. 本计划不覆盖的并行问题

以下问题与本计划并行，但不在本计划实施范围内：

- `frontend_rust/crates/web-sdk` 的 envelope DTO 兼容修复
- 当前 summary prompt / summary generator 的独立修复
- 现有 chat graphflow 主链的其他优化项

这些问题不应与本计划混做一个大提交。
