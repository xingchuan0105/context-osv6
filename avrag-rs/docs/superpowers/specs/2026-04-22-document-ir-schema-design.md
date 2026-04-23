# 文档解析统一 IR 设计（双向量空间版）

## 1. 背景与决策边界

### 1.1 背景

当前 ingestion 主链路的问题不在 worker 编排，而在 parser 边界太弱：

- parser 输出结构过薄，无法承载复杂 PDF / Office / PPT 的结构信息
- 下游 chunking 只能围绕纯文本展开，图表、截图、页面图像很难稳定进入检索
- 现有 `ParsedDocument.pages -> text chunk` 直连方式，不适合作为长期稳定边界

### 1.2 本设计明确写死的决策

本设计不再讨论以下事项，直接作为约束：

1. 保留双向量空间：
   - 文本块进入 `text` collection
   - 图文块进入 `multimodal` collection
2. 保留双模型：
   - 文本 embedding 模型只服务文本块
   - 多模态 embedding 模型只服务图文块
3. 统一的是解析中间表示 `DocumentIR`，不是向量空间
4. PDF 主路径使用 `EdgeParse`
5. `MinerU API` 只负责 PDF OCR fallback，不与 `EdgeParse` 争主解析
6. `DOCX` 使用 `docx4j`
7. `XLSX` 使用 `Apache POI XSSF`
8. `PPT/PPTX` 使用 `Apache POI XSLF/HSLF`

### 1.3 目标

`DocumentIR` 作为 parser 与 chunk/storage/retrieval 之间的稳定契约，必须满足：

- 能覆盖 PDF / DOCX / XLSX / PPTX / HTML / 文本文件
- 能稳定表达文本块、表格块、图片块、slide 文本块、sheet 表格区域
- 能为 citations 提供统一 `source_locator`
- 能让后续 chunking 只依赖 IR，而不依赖 parser 实现细节

---

## 2. 设计原则

### 2.1 统一抽象边界，不统一原始来源

不同格式应保留各自最佳 parser：

- `PDF -> EdgeParse`
- `DOCX -> docx4j`
- `XLSX -> POI XSSF`
- `PPT/PPTX -> POI XSLF/HSLF`

但 parser 最终都必须输出同一套 `DocumentIR`。

### 2.2 PDF 第一版按页归属 backend

PDF 第一版不做“同一页内多个 backend 块级混合拼装”。约束如下：

- 每一页最终只归属于一个 backend
- `EdgeParse` 产出的页面，不再用 `MinerU` 块级补丁覆盖
- `MinerU` 接管的页面，整页结果来自 OCR fallback

这样可以避免 citations、页内顺序、块去重失控。

### 2.3 文本主路与图文辅路在 IR 层分流

IR 需要显式区分：

- 可直接进入文本 chunking 的块
- 需要产出图片资产与图文块的块

不能再依赖“是否存在 image_path”这种弱推断。

---

## 3. 核心 Schema

### 3.1 顶层结构

```rust
pub struct DocumentIR {
    pub document_id: String,
    pub title: String,
    pub doc_type: DocumentType,
    pub primary_backend: ParseBackend,
    pub backend_version: Option<String>,
    pub language: Option<String>,
    pub metadata: std::collections::BTreeMap<String, String>,
    pub pages: Vec<PageIR>,
    pub blocks: Vec<BlockIR>,
    pub assets: Vec<AssetIR>,
    pub warnings: Vec<ParseWarning>,
}
```

### 3.2 文档类型

```rust
pub enum DocumentType {
    Pdf,
    Docx,
    Xlsx,
    Ppt,
    Pptx,
    Html,
    Text,
    Code,
    Image,
    Unknown,
}
```

### 3.3 解析 backend

```rust
pub enum ParseBackend {
    EdgeParsePdf,
    MineruPdfOcr,
    Docx4jDocx,
    PoiXlsx,
    PoiPptx,
    PoiPpt,
    HtmlLocal,
    TextLocal,
    CodeLocal,
}
```

### 3.4 页面结构

```rust
pub struct PageIR {
    pub page_number: u32,
    pub width: Option<f32>,
    pub height: Option<f32>,
    pub backend: ParseBackend,
    pub text_char_count: usize,
    pub image_count: usize,
    pub metadata: std::collections::BTreeMap<String, String>,
}
```

约束：

- 非分页文档也允许虚拟页
- `DOCX/XLSX/HTML` 可只生成单页或逻辑页
- `PDF/PPT/PPTX` 必须有稳定页码或 slide 编号

### 3.5 块结构

```rust
pub struct BlockIR {
    pub block_id: String,
    pub page: Option<u32>,
    pub block_type: BlockType,
    pub modality: BlockModality,
    pub text: String,
    pub summary_text: Option<String>,
    pub asset_refs: Vec<String>,
    pub caption: Option<String>,
    pub section_path: Vec<String>,
    pub source_locator: SourceLocator,
    pub parser_backend: ParseBackend,
    pub metadata: std::collections::BTreeMap<String, String>,
}
```

```rust
pub enum BlockType {
    Heading,
    Paragraph,
    ListItem,
    Table,
    Quote,
    Code,
    Figure,
    Caption,
    SlideText,
    SlideNotes,
    SlideImage,
    SheetTable,
    SheetCellRange,
}
```

```rust
pub enum BlockModality {
    TextOnly,
    ImageWithContext,
}
```

### 3.6 资产结构

```rust
pub struct AssetIR {
    pub asset_id: String,
    pub page: Option<u32>,
    pub asset_kind: AssetKind,
    pub storage_path: String,
    pub mime_type: Option<String>,
    pub width: Option<u32>,
    pub height: Option<u32>,
    pub parser_backend: ParseBackend,
    pub metadata: std::collections::BTreeMap<String, String>,
}
```

```rust
pub enum AssetKind {
    Image,
    SlideRender,
}
```

### 3.7 引用定位结构

```rust
pub struct SourceLocator {
    pub page: Option<u32>,
    pub bbox: Option<[f32; 4]>,
    pub paragraph_index: Option<usize>,
    pub table_index: Option<usize>,
    pub sheet_name: Option<String>,
    pub row_range: Option<(u32, u32)>,
    pub col_range: Option<(u32, u32)>,
    pub slide_index: Option<u32>,
    pub shape_name: Option<String>,
}
```

### 3.8 警告结构

```rust
pub struct ParseWarning {
    pub code: String,
    pub message: String,
    pub page: Option<u32>,
    pub backend: ParseBackend,
}
```

---

## 4. 各格式映射规则

### 4.1 PDF

#### Digital PDF

- backend: `EdgeParsePdf`
- heading / paragraph / table 直接映射为 `TextOnly`
- figure 映射为：
  - `AssetIR(kind=Image)`
  - `BlockIR(block_type=Figure, modality=ImageWithContext)`
- `bbox` 必须保留到 `SourceLocator`

#### OCR fallback page

- backend: `MineruPdfOcr`
- 页级替换，不与 `EdgeParse` 块级混合
- `MinerU` 返回的图片页内容转成：
  - OCR 文本块
  - 图文块

### 4.2 DOCX

- heading / paragraph / list / table 进入文本块
- embedded image 进入：
  - `AssetIR(kind=Image)`
  - 与邻近段落组合成 `Figure`
- 第一版不要求完整支持：
  - comments
  - revisions
  - text box

### 4.3 XLSX

- 以逻辑表格区域为中心建块
- 一个工作表可有多个 `SheetTable`
- 不能把整张 sheet 拼成一大段文本
- `SourceLocator.sheet_name + row_range + col_range` 必须可回溯

### 4.4 PPT/PPTX

- 同一 slide 必须产出两类结果：
  - `SlideText` / `SlideNotes`
  - `SlideImage`
- `SlideImage` 必须关联 slide render 产物
- `SlideImage.summary_text` 由该页标题、正文摘要、notes 共同构造

这条约束是为了避免“只有整页图片向量、没有细文本命中”。

---

## 5. 从 IR 到存储与检索单元的投影规则

### 5.1 文本块投影

以下 block 进入文本主路：

- `Heading`
- `Paragraph`
- `ListItem`
- `Table`
- `Quote`
- `Code`
- `SlideText`
- `SlideNotes`
- `SheetTable`
- `SheetCellRange`

投影后产物：

- `document_blocks`
- `document_chunks`
- `text collection`

### 5.2 图文块投影

以下 block 进入图文辅路：

- `Figure`
- `SlideImage`

投影要求：

- 必须有 `asset_refs`
- `summary_text` 必须非空
- 进入：
  - `document_assets`
  - `document_multimodal_chunks`
  - `multimodal collection`

### 5.3 文本 fallback 规则

图文块的 `summary_text` 必须用于：

- answer context 的文本展示
- 可选的 BM25 fallback 文本拼接

但图文块默认不进入文本 dense collection。

---

## 6. 校验规则

`DocumentIR` 入库前必须跑统一校验。

### 6.1 硬校验

以下情况直接失败：

1. `block_id` 重复
2. `asset_id` 重复
3. `ImageWithContext` 块没有 `asset_refs`
4. `SlideImage` 没有 render 资产
5. `PDF` 块缺页码
6. `text` 或 `summary_text` 含 `\\0`

### 6.2 软校验

以下情况允许通过，但必须记 warning：

1. 块缺少 `section_path`
2. `bbox` 缺失
3. PPT notes 缺失
4. XLSX 无法识别逻辑表格区域，只退化为 `SheetCellRange`

---

## 7. 与当前代码的对接边界

当前仓库已有：

- parser route:
  [router.rs](/home/chuan/context-osv6/avrag-rs/crates/ingestion/src/parser/router.rs)
- worker parse -> chunk -> store:
  [main.rs](/home/chuan/context-osv6/avrag-rs/bins/worker/src/main.rs)
- chunker:
  [chunker.rs](/home/chuan/context-osv6/avrag-rs/crates/ingestion/src/chunker.rs)

本设计要求：

1. `NormalizedDocument { units }` 逐步升级为 `DocumentIR`
2. `build_chunk_plan(...)` 的输入从“弱文本单位”改为 `DocumentIR.blocks`
3. 现有 `ParsedUnitKind::Text / ImageWithContext` 可以作为过渡层，但不能作为最终稳定 schema

---

## 8. 验收标准

### 8.1 结构标准

- PDF / DOCX / XLSX / PPTX 都能输出 `DocumentIR`
- `source_locator` 能稳定回溯
- 图文块和文本块在 IR 层显式分离

### 8.2 检索标准

- 文本块能稳定进入文本 dense/BM25 主路
- 图文块能稳定进入 multimodal collection
- PPT/PPTX 不会退化成“只有页图，没有文本块”

### 8.3 质量标准

- 不再允许 `String::from_utf8_lossy(bytes)` 这种二进制降级伪解析进入 IR
- 不再允许 parser 直接把脏文本写入 chunks

