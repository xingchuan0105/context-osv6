# ADR 0002: 文件解析路由与多模态检索召回优化

## 背景

当前 `context-osv6` 的文档处理主链路是：

1. `ParserFactory` 按扩展名选择本地解析器
2. 解析结果统一落成 `ParsedDocument { pages[] }`
3. `build_chunk_items()` 只为文本生成 `ParsedPreviewItem`
4. Worker 将文本块写入 PostgreSQL，并把同一批文本向量写入 Qdrant
5. RAG runtime 只运行文本 BM25 + 文本 Dense + 文本 rerank

这条链路已经满足纯文本、代码、基础 Office/PDF 的 MVP 需求，但对复杂版面文档存在三个结构性缺口：

- 缺少“本地解析 / MinerU 精准解析”的路由层，复杂 PDF、PPT、扫描件无法按成本和质量分流
- 缺少统一的图文中间表示，MinerU 即使返回图片与 Markdown，也无法进入当前 chunk / embedding / retrieval 主链路
- 缺少多模态召回与重排，图表、架构图、截图中的核心证据即使被解析出来，也无法稳定参与检索与答案生成

因此，本 ADR 的目标不是替换现有 Rust 本地解析链路，而是在保留低成本主路径的前提下，引入“复杂文档走 MinerU、下游统一图文结构、多路召回汇合后统一重排”的架构。

## 决策

### 1. 文档摄取拆为两条并行子路径

摄取阶段拆为两个正交子路径，而不是继续把“解析、切块、向量化”绑死在一个文本管线里：

1. 解析路由路径：决定当前文件应走本地解析还是 MinerU 精准解析
2. 内容归一化路径：无论上游来源是什么，都输出统一的“文本块 + 图片上下文块”集合

这样可以保证：

- 简单文档继续走本地库，维持毫秒级到秒级处理和低成本
- 复杂版面只在必要时调用 MinerU，避免全量外部解析
- 下游 chunking / embedding / retrieval 不再依赖“解析来源”，只依赖统一中间模型

### 2. 引入统一的图文中间模型

当前 `ParsedDocument -> ParsedPreviewItem` 只覆盖文本预览，不足以承载 MinerU 图片产物。新增统一中间表示，作为 parser 与 chunker/embedding 之间的稳定边界。

建议引入两层结构：

```rust
#[derive(Debug, Clone)]
pub enum ParsedUnitKind {
    Text,
    ImageWithContext,
}

#[derive(Debug, Clone)]
pub struct ParsedUnit {
    pub unit_id: String,
    pub page: u32,
    pub kind: ParsedUnitKind,
    pub text: String,
    pub image_path: Option<String>,
    pub caption: Option<String>,
    pub context: Option<String>,
    pub parser_backend: String,
    pub metadata: std::collections::BTreeMap<String, String>,
}
```

约束如下：

- `Text`：只承载纯文本正文、标题、列表、代码等可直接切块的内容
- `ImageWithContext`：承载图片路径、图片附近上下文、可选 caption，以及必要的页码/区域信息
- `text` 字段始终保留可检索主文本；对图文块而言，`text` 应为 `caption + context` 的标准化结果，便于 BM25 / fallback 文本召回
- `parser_backend` 明确记录 `local_pdf`、`local_office`、`mineru_precise` 等来源，便于质量观测和重跑

现有 `ParsedDocument` 可以继续保留，用于 parser 内部；但 `build_chunk_items()` 的输入应逐步从 `ParsedDocument.pages` 升级为 `Vec<ParsedUnit>`。

### 3. 解析路由层升级为 Router，而不是仅靠扩展名

当前 `ParserFactory::create_parser()` 只有“按扩展名选解析器”的职责。新架构中，它应升级为两段式：

1. 轻量探针：快速判断文档复杂度与可解析性
2. 路由决策：选择本地解析或 MinerU 精准解析

建议新增：

```rust
pub enum ParseRoute {
    Local,
    MineruPrecise,
}

pub struct ParseProbeResult {
    pub mime_type: String,
    pub extension: String,
    pub extracted_text_chars: usize,
    pub page_count: Option<u32>,
    pub image_hint_count: usize,
    pub table_hint_count: usize,
    pub likely_scanned: bool,
    pub likely_presentation: bool,
}
```

路由规则建议如下：

1. `.txt/.md/.csv/.json/.rs/.py/...` 等纯文本与代码文件直接走 `Local`
2. `.png/.jpg/.jpeg/.webp` 等纯图片文件直接走 `MineruPrecise`
3. `.ppt/.pptx` 直接走 `MineruPrecise`
4. `.pdf` 先走探针：
   - 前 1-3 页可稳定提取高密度文本，且复杂图表/表格提示较低，走 `Local`
   - 文本极少、疑似扫描件、图像密集、表格密集时，走 `MineruPrecise`
5. `.doc/.docx/.xls/.xlsx` 初期默认走本地解析；若后续发现图文损失明显，再为 Office 增加复杂度探针和 MinerU 分流

这一定义的核心原则是：先用低成本探针判定，再决定是否调用高成本外部解析。

### 4. 解析执行层保留“双引擎”，但统一出图文单位

#### 分支 A：本地轻量解析

本地分支继续复用现有 parser：

- `PdfParser`
- `OfficeParser`
- `HtmlParser`
- `TextParser`
- `CodeParser`

但输出阶段新增归一化步骤：

1. 将正文、标题、代码块等转成 `ParsedUnitKind::Text`
2. 若文档内可识别图片占位、图片链接、HTML `<img>`、Office/PDF 内嵌图片锚点，则提取邻近上下文，生成 `ParsedUnitKind::ImageWithContext`
3. 若当前本地 parser 无法抽出结构化图片信息，也必须保留将来补充图片 extractor 的接口，而不是把数据结构锁死为纯文本

#### 分支 B：MinerU 精准解析

MinerU 精准解析路径应遵循以下约束：

1. 上传文件到 MinerU，获取任务与结果地址
2. 拉取高精度 Markdown 与裁剪图片目录
3. 将 Markdown 解析成文本单位
4. 将图片及其前后段落、标题、列表项等组装成 `ImageWithContext`
5. 将图片资产回写到对象存储，统一由本系统托管访问路径

关键要求：MinerU 输出不能只消费纯文字，必须把图片与其上下文一起归一化，否则会直接损失图表问答能力。

### 5. Chunking 与 Embedding 分为“文本主路”和“图文辅路”

当前 `text-splitter` 适合文本，不适合直接切图片。因此下游不再试图“所有内容都变成同一种 chunk”，而是分路处理：

#### 文本主路

- 输入：`ParsedUnitKind::Text`
- 切块：继续使用 `text-splitter`
- 预算：沿用当前 `512 token` 目标预算
- overlap：维持低重叠策略，默认 32-64 token 对应字符窗口即可
- 检索：PostgreSQL BM25 + 文本 embedding 向量召回

#### 图文辅路

- 输入：`ParsedUnitKind::ImageWithContext`
- 切块对象：只切 `context`，不切图片本身
- 每个图文块保留 `(image_path, caption, trimmed_context)` 作为最小检索单元
- 若上下文过长，只截取图前/图后最相关片段，不生成多张图片的跨图混合块
- 向量化：使用多模态 embedding 模型为 `(query/image/context)` 兼容场景生成向量

这样可以避免两类错误：

- 用文本切块器错误地“切图片”
- 为了统一管线，强行让多模态块退化成纯文本块

### 6. 存储层采用“文本块 / 图文块”双实体，而不是单表硬塞

为了降低实施风险，建议先使用逻辑双实体，而不是一开始把所有字段塞进当前文本 chunks 模型。

建议的持久化结构：

1. `document_chunks`
   - 存文本块
   - 保留当前 BM25 / text dense 主路兼容性
2. `document_assets`
   - 存图片资产、页码、对象存储路径、caption、parser backend
3. `document_multimodal_chunks`
   - 存图片上下文块
   - 通过 `asset_id` 关联到图片资产
   - 持有 `context_text`、`caption`、`page`、`parser_backend`

如果希望降低初期迁移复杂度，也可以在第一阶段先保留 `chunks` 表不动，仅新增 `document_assets` 与 `document_multimodal_chunks`，等多模态链路稳定后再考虑统一抽象。

Qdrant 存储建议分两步：

1. MVP 阶段使用两个 collection：
   - `chunks_text`
   - `chunks_multimodal`
2. 后续若验证命中率与维护复杂度可控，再评估迁移到单 collection + named vectors

选择双 collection 的原因是：它与当前文本检索主链兼容，改动边界清晰，便于逐步接入多模态而不破坏已有文本 RAG。

### 7. 检索召回升级为“双主路并发 + 汇合重排”

当前 runtime 只覆盖文本检索。新方案中，召回分为文本主路与视觉辅路，再在 rerank 阶段汇合。

#### 7.1 查询理解层

Planner 需要新增一个轻量判断：当前问题是否需要视觉证据。

建议输出新增字段：

- `needs_visual_evidence: bool`
- `visual_queries: Vec<String>`
- `text_queries: Vec<String>`

判断标准可包括：

- 问题显式提到“图、图表、截图、结构图、曲线、页面、界面、PPT、哪张图”
- 实体类型天然偏视觉，如 slide、dashboard、chart、architecture diagram
- 上一轮会话已指向图片或演示材料

#### 7.2 文本主路

保持现有思路，但扩大为稳定的 hybrid baseline：

1. BM25 Top 50
2. 文本 Dense Top 50
3. 用 RRF 融合，得到文本候选 Top 30

该路径继续复用当前 PostgreSQL + Qdrant + reranker 基础设施。

#### 7.3 视觉辅路

当 `needs_visual_evidence = true` 时启用：

1. 对 query 生成多模态兼容 embedding
2. 在 `chunks_multimodal` collection 中召回 Top 20 图文块
3. 返回的不只是图片路径，还必须附带 `caption/context/page/asset_id`

视觉辅路不承担全文召回职责，只承担“视觉证据补充”职责，因此预算可以比文本主路更小，但必须保真。

#### 7.4 汇合与重排

文本候选与图文候选汇合后，进入统一重排阶段：

1. 文本候选 Top 30
2. 图文候选 Top 20
3. 合并成最多 50 个候选证据
4. 第一层重排：
   - 纯文本可先走轻量文本 reranker
   - 图文候选保留原始多模态得分
5. 第二层终裁：
   - 使用多模态 VLM 或统一的 evidence judge，对文本块与图文块共同排序
   - 产出最终进入 answer context 的 Top N 证据

核心要求是：最终 answer 阶段看到的是“同一问题下的混合证据池”，而不是两个完全分裂的上下文。

### 8. Answer 上下文与引用协议也要升级

为了让 answer agent 正确理解图文证据，上下文拼装不能再只输出纯文本 chunk。

建议 evidence index 扩展为：

- `chunk_id`
- `doc_id`
- `chunk_type` (`text` / `image_with_context`)
- `page`
- `retrieval_channel` (`bm25` / `text_dense` / `multimodal_dense` / `rrf` / `vl_rerank`)
- `caption`
- `image_path`（若有）
- `context_excerpt`

这样 answer agent 才能：

- 明确知道某条证据来自图文块，而不是普通段落
- 在回复中做更准确的保守性判断
- 在后续扩展前端图片预览与 citation 卡片时复用同一引用协议

### 9. 对现有代码的模块化改造建议

建议在当前代码基础上按以下方式演进：

#### 摄取层

- `crates/ingestion/src/parser/mod.rs`
  - 从 `ParserFactory` 升级为 `ParseRouter + ParserExecutor`
- `crates/ingestion/src/parser/`
  - 新增 `mineru.rs`
  - 新增 `probe.rs`，负责 PDF/Office 复杂度探针
- `crates/ingestion/src/chunker.rs`
  - 从“只输入文本页”升级为“按 `ParsedUnitKind` 分路输出文本块/图文块”
- `bins/worker/src/main.rs`
  - 在解析后新增归一化与双路入库流程

#### 存储层

- `crates/storage-pg`
  - 新增 `document_assets` / `document_multimodal_chunks` 的 repository API
  - 文本 chunks API 保持兼容
- `crates/storage-qdrant`
  - 支持文本 collection 与多模态 collection 的独立 upsert/search

#### RAG 层

- `crates/rag-core/src/retrieval.rs`
  - 新增 multimodal retrieval primitives
- `crates/rag-core/src/runtime.rs`
  - 将当前文本-only plan item execution 扩展为 text / visual / hybrid 三类
- `crates/rag-core/src/context.rs`
  - 支持拼装图文 evidence index，而不是只拼文本块

### 10. 分阶段实施顺序

#### Phase 1：解析路由与统一图文模型

目标：不引入多模态检索，先把上游结构打通。

- 增加 `ParseProbeResult`
- 引入 `ParseRoute`
- 接入 MinerU precise parse client
- 统一输出 `ParsedUnit`
- 新增 `document_assets` / `document_multimodal_chunks` 入库

验收标准：

- PPT、扫描 PDF、图片文档能够成功走 MinerU
- MinerU 输出图片不会被丢弃
- PostgreSQL 能看到文本块与图文块两类记录

#### Phase 2：多模态向量化与独立召回

目标：让图文块能被单独召回。

- 新增多模态 embedding client
- 新增 `chunks_multimodal` collection
- worker 在文本块之外，为图文块写入多模态向量
- runtime 新增 visual retrieval path

验收标准：

- 针对“图表/截图/PPT 页面”类问题，能够召回图文块
- 文本主路行为不回退

#### Phase 3：混合重排与答案上下文统一

目标：让 text + image 证据共同参与最终回答。

- planner 产出 `needs_visual_evidence`
- runtime 汇合文本候选与图文候选
- 引入统一 evidence judge / multimodal rerank
- answer context 注入统一 evidence index

验收标准：

- 图文类问题的最终答案不再只依赖文本描述
- citation 能区分文本证据与图文证据

#### Phase 4：观测、成本控制与回灌优化

目标：把这条链路稳定成可运营能力。

- 记录路由命中率：本地 / MinerU
- 记录 MinerU 平均耗时与成本
- 记录图文召回命中率、图文被 answer 采用率
- 对低收益文档类型回调阈值，避免过度调用 MinerU

### 11. 关键工程约束

- 不把 MinerU 作为默认全量解析器，只作为复杂版面兜底
- 不把图片压平成纯文字后再假装支持图文检索
- 不把文本 embedding 和多模态 embedding 强行混成同一种索引单元
- 不让 answer 阶段只看到“文本摘录”，却不知道某证据来自图片

## 结果

采用该方案后，`context-osv6` 的文档处理与检索架构将形成以下稳定分层：

1. 低成本本地解析作为默认主路
2. MinerU 精准解析作为复杂文档兜底
3. 文本块与图文块使用统一中间模型承接
4. 文本召回与视觉召回并发执行
5. 重排阶段汇合多源证据，统一注入 answer context

这条路线兼顾了 MVP 阶段的成本、时延与工程复杂度，也为后续图表问答、PPT 问答、截图问答和更强的多模态 citation 留出了明确扩展点。

## 不在本 ADR 范围内

- 前端图片预览与图文 citation UI
- OCR/VLM 模型供应商的最终选型
- 多模态 rerank prompt 的最终产品化文案
- 全量历史文档回灌与重建计划