# ingestion worker 改造清单（双 collection / 双模型版）

## 1. 目标

在不推翻现有 API、队列和 worker 骨架的前提下，把 ingestion 主链路升级为：

1. parser backend 按格式和 PDF 页级 probe 路由
2. 所有 parser 输出统一 `DocumentIR`
3. 文本块与图文块分实体持久化
4. 文本向量与多模态向量分别写入不同 collection
5. 删除伪解析 fallback，失败必须显式化

本设计默认保留：

- `BM25 + text dense` 文本主路
- `multimodal dense` 图文辅路
- 两套 embedding 模型
- 两个 Qdrant collection

---

## 2. 现状与问题

当前 worker 主链路位于：

- [main.rs](/home/chuan/context-osv6/avrag-rs/bins/worker/src/main.rs#L250)

现状是：

1. object store 取 bytes
2. route
3. parse
4. `NormalizedDocument`
5. `build_chunk_plan(...)`
6. `store_document_body_items(...)`
7. text embeddings
8. optional multimodal embeddings

主要问题：

- parser 输出结构过薄
- `docx/xlsx` 还在错误共用 `OfficeParser`
- PDF 还是文档级二选一路由
- mm collection 通过第一条向量长度隐式建表，缺少明确边界
- `fallback_normalized_doc(...)` 会把二进制伪装成文本

---

## 3. 目标模块拆分

### 3.1 新的 ingest pipeline

```text
load bytes
  -> build parse plan
  -> execute backend parse
  -> normalize to DocumentIR
  -> validate + sanitize IR
  -> persist parse run / blocks / assets
  -> build text chunk plan
  -> build multimodal chunk plan
  -> persist text chunks
  -> persist multimodal chunks
  -> write text embeddings
  -> write multimodal embeddings
  -> mark document completed
```

### 3.2 新增内部边界

建议新增模块：

- `parse_planner.rs`
- `backend_clients/edgeparse.rs`
- `backend_clients/office_service.rs`
- `backend_clients/mineru_ocr.rs`
- `ir.rs`
- `ir_validator.rs`
- `ir_chunker.rs`

---

## 4. Phase 1: 路由与 parser 边界改造

### 4.1 任务

1. 引入 `ParsePlan`
2. 替换当前 `ParseRoute::Local | MineruPrecise`
3. DOCX/XLSX/PPT/PPTX 不再进入本地 `OfficeParser`
4. PDF 改成页级 backend 归属

### 4.2 代码改造点

#### `crates/ingestion/src/parser/router.rs`

从当前：

- `Local`
- `MineruPrecise`

升级为：

```rust
pub enum ParseBackendChoice {
    EdgeParsePdf,
    MineruPdfOcr,
    OfficeParserService,
    HtmlLocal,
    TextLocal,
    CodeLocal,
}
```

#### `bins/worker/src/main.rs`

替换当前：

- route -> parse -> normalize

为：

- route -> parse_plan -> backend execution -> `DocumentIR`

### 4.3 验收

- DOCX/XLSX/PPT/PPTX 不再走旧 `OfficeParser`
- PDF 的 route 日志能打印每页 backend 决策
- 旧 `fallback_normalized_doc(...)` 仍保留但禁止主链使用

---

## 5. Phase 2: IR 校验与持久化

### 5.1 任务

1. 在 PG 中新增 parse audit 表
2. 持久化 `DocumentIR.blocks`
3. 持久化 `DocumentIR.assets`

### 5.2 数据库建议

新增：

#### `document_parse_runs`

- `run_id`
- `document_id`
- `backend_summary`
- `status`
- `duration_ms`
- `warnings_json`
- `error_json`
- `artifact_path`

#### `document_blocks`

- `block_id`
- `document_id`
- `page`
- `block_type`
- `modality`
- `text`
- `summary_text`
- `source_locator_json`
- `parser_backend`
- `metadata_json`

保留并继续使用：

- `document_assets`
- `document_multimodal_chunks`
- `document_chunks`

### 5.3 校验逻辑

在写库前执行：

1. 去除 `\0`
2. 归一空白
3. 检查 block / asset 唯一性
4. 检查图文块必须绑定 asset

### 5.4 验收

- 失败文档能看到 parse run 记录
- 块与资产可以独立审计
- 不再出现 `invalid byte sequence for encoding "UTF8": 0x00`

---

## 6. Phase 3: chunking 改造

### 6.1 任务

1. 废弃“直接从弱 `ParsedUnit` 切块”的长期路径
2. 引入 `ir_chunker`
3. 文本块与图文块分别产出 chunk 计划

### 6.2 文本 chunk 规则

输入：

- `Heading`
- `Paragraph`
- `ListItem`
- `Table`
- `Quote`
- `Code`
- `SlideText`
- `SlideNotes`
- `SheetTable`

规则：

1. 优先按 section 合并
2. 小段落合并到上一个 chunk
3. `Table` 单独成块，不和大段正文混合
4. `SheetTable` 按逻辑区域切，不按整 sheet 切

### 6.3 图文 chunk 规则

输入：

- `Figure`
- `SlideImage`

规则：

1. 每个图文块对应一个 multimodal chunk
2. `summary_text = caption + local heading + local context`
3. 不跨图拼接
4. 不把图文块切成多段文本 chunk

### 6.4 验收

- 文本 chunk 与 multimodal chunk 数量可预测
- PPT/PPTX 同时产出 text chunk 和 multimodal chunk
- XLSX 不会退化成超长文本 chunk

---

## 7. Phase 4: 双 collection 写入改造

### 7.1 任务

1. 文本 collection 与 multimodal collection 显式配置
2. 不再依赖“第一条 multimodal 向量长度”做隐式 schema 决策
3. collection 在启动或首写前明确 ensure

### 7.2 配置建议

- `QDRANT_TEXT_COLLECTION`
- `QDRANT_MULTIMODAL_COLLECTION`
- `TEXT_EMBEDDING_DIM`
- `MM_EMBEDDING_DIM`

### 7.3 写入规则

#### 文本主路

- 输入：`document_chunks`
- 模型：text embedding
- collection：text collection

#### 图文辅路

- 输入：`document_multimodal_chunks`
- 模型：multimodal embedding
- collection：multimodal collection

### 7.4 验收

- 两个 collection 的向量维度各自稳定
- 维度不匹配时报错可诊断
- 不再依赖运行时第一条向量推断 multimodal collection schema

---

## 8. Phase 5: retrieval 与 answer 上下文的后续对接

本阶段不是 worker 内部实现，但必须为下游留出明确接口。

### 8.1 文本检索

保留：

- BM25
- text dense
- text rerank

### 8.2 图文检索

保留：

- multimodal dense
- 可选 multimodal rerank

### 8.3 汇合方式

沿用双 collection 设计，继续 late fusion：

1. text pool
2. multimodal pool
3. merge
4. rerank
5. answer context

但 answer context 必须能区分：

- `chunk_type`
- `asset_id`
- `page`
- `parser_backend`

---

## 9. 删除项

以下能力必须被明确删除或降级：

1. `docx/xlsx` 共用旧 `OfficeParser`
2. 二进制 `lossy bytes -> text` fallback
3. PDF 整份二选一路由

### 9.1 允许保留的过渡代码

仅允许保留：

- 旧 parser 适配器
- 迁移期测试辅助代码

不允许长期保留“双主路径都在生产里同时跑”的隐藏废案。

---

## 10. 测试与验收样本

### 10.1 样本集

至少准备：

1. 数字文本 PDF
2. 扫描 PDF
3. 图文混排 PDF
4. 复杂 DOCX
5. 大表 XLSX
6. 文本密集 PPTX
7. 图表密集 PPTX

### 10.2 必须补的测试

#### 单元测试

- `ParsePlan` 路由测试
- `DocumentIR` 校验测试
- `ir_chunker` 切块测试
- `source_locator` 映射测试

#### 集成测试

- office parser service contract test
- PDF OCR fallback test
- text / multimodal dual collection write test

#### 回归测试

- 旧纯文本文档吞吐不显著回退
- 历史文本 RAG 基本能力不退化

---

## 11. 实施顺序

1. `DocumentIR` 与 validator
   - verify: 各格式都能产出统一 IR
2. `office-parser-jvm` 协议与 adapter
   - verify: docx/xlsx/ppt/pptx 可稳定返回 IR
3. `EdgeParse + MinerU OCR` 的 PDF page-level route
   - verify: digital/scanned PDF 都有明确 backend 归属
4. `document_blocks / parse_runs` 持久化
   - verify: parse 问题可审计
5. `ir_chunker` 替换旧 chunk 入口
   - verify: text/mm chunk 数量与结构稳定
6. 双 collection ensure 与写入
   - verify: text/mm 各自稳定入库
7. retrieval 对接
   - verify: answer context 能消费双路 evidence

---

## 12. 成功标准

1. parser 不再直接把脏文本塞进 PG
2. DOCX/XLSX/PPT/PPTX 解析链路从旧 `OfficeParser` 完全迁出
3. PDF 数字页和 OCR 页都能稳定落到正确 backend
4. text collection 与 multimodal collection 长期稳定运行
5. 新链路出错时可以从 `parse_run -> blocks -> chunks -> vectors` 全链审计

