# 文档解析 backend 协议设计（EdgeParse + Office Parser + MinerU）

## 1. 背景与目标

本设计定义 parser backend 边界，解决三个问题：

1. Rust worker 不应直接承载 JVM 依赖
2. Office 文档需要稳定、可观测、可限流的独立解析服务
3. `EdgeParse` 与 `MinerU` 在 PDF 链路里的职责必须清晰，不再重复争夺主路径

本设计明确保留：

- 文本向量与多模态向量使用不同模型
- 文本 collection 与 multimodal collection 分离
- parser 统一输出 `DocumentIR`

---

## 2. backend 拓扑

### 2.1 in-process backend

- `EdgeParsePdfAdapter`
  - 运行位置：`avrag-worker` 进程内
  - 职责：数字 PDF 主解析

### 2.2 service backend

- `office-parser-jvm`
  - 运行位置：独立 JVM 进程或容器
  - 内含：
    - `docx4j` adapter
    - `poi-xlsx` adapter
    - `poi-ppt` adapter

### 2.3 external backend

- `MinerU API`
  - 运行位置：外部 HTTP 服务
  - 职责：PDF OCR fallback

---

## 3. backend 职责边界

### 3.1 EdgeParse

只负责：

- `PDF` 数字文本页
- `PDF` 多栏/表格/figure 的主解析
- 结构化文本块与 bbox 输出

不负责：

- OCR
- Office 文档
- PPT/PPTX

### 3.2 office-parser-jvm

负责：

- `DOCX -> docx4j`
- `XLSX -> POI XSSF`
- `PPTX -> POI XSLF`
- `PPT -> POI HSLF`

不负责：

- PDF
- OCR
- embedding
- 存储

### 3.3 MinerU

只负责：

- PDF OCR fallback
- 扫描页救援
- `EdgeParse` 失败页或低文本异常页救援

不负责：

- 替代 EdgeParse 成为 PDF 全量主路径
- Office 文档主解析

---

## 4. 路由协议

### 4.1 ParsePlan

worker 在进入具体 backend 前，先形成显式 `ParsePlan`。

```rust
pub enum ParsePlan {
    Pdf(PdfParsePlan),
    Office(OfficeParsePlan),
    Local(LocalParsePlan),
}
```

```rust
pub struct PdfParsePlan {
    pub document_id: String,
    pub filename: String,
    pub pages: Vec<PdfPagePlan>,
}
```

```rust
pub struct PdfPagePlan {
    pub page_number: u32,
    pub backend: PdfPageBackend,
    pub reason: String,
}
```

```rust
pub enum PdfPageBackend {
    EdgeParse,
    MineruOcr,
}
```

```rust
pub struct OfficeParsePlan {
    pub document_id: String,
    pub filename: String,
    pub doc_type: OfficeDocType,
}
```

```rust
pub enum OfficeDocType {
    Docx,
    Xlsx,
    Ppt,
    Pptx,
}
```

### 4.2 路由规则

1. `PDF`
   - 先 probe
   - page-level route
   - 每页只选一个 backend
2. `DOCX/XLSX/PPT/PPTX`
   - 直接进入 `office-parser-jvm`
3. `TXT/MD/HTML/Code`
   - 保留本地 parser

---

## 5. office-parser-jvm API

### 5.1 设计原则

- 同机/同 VPC 内部调用
- 同步请求，同步返回 `DocumentIR`
- worker 负责从对象存储取 bytes，parser service 不持有对象存储权限
- service 不做持久化

### 5.2 公共头

- `X-Request-Id`
- `X-Trace-Id`
- `X-Document-Id`

### 5.3 公共 endpoint

#### `GET /v1/healthz`

返回：

```json
{
  "ok": true,
  "service": "office-parser-jvm"
}
```

#### `GET /v1/capabilities`

返回：

```json
{
  "formats": ["docx", "xlsx", "ppt", "pptx"],
  "backend_versions": {
    "docx4j": "11.x",
    "poi": "5.x"
  }
}
```

### 5.4 解析接口

#### `POST /v1/parse/docx`
#### `POST /v1/parse/xlsx`
#### `POST /v1/parse/ppt`
#### `POST /v1/parse/pptx`

请求类型：`multipart/form-data`

字段：

- `file`: 二进制文件
- `filename`: 原始文件名
- `document_id`: 文档 id
- `parse_profile`: 固定值 `default`

响应：

```json
{
  "document_ir": { "...": "..." },
  "warnings": [],
  "stats": {
    "duration_ms": 1320,
    "block_count": 48,
    "asset_count": 3
  }
}
```

错误响应：

```json
{
  "error": {
    "code": "DOCX_PARSE_FAILED",
    "message": "failed to parse structured paragraphs",
    "retryable": false
  }
}
```

### 5.5 限流与超时

服务必须支持：

- 文档大小上限
- 并发上限
- 请求超时
- 每类格式单独 metrics

建议默认值：

- `DOCX/XLSX`: 30s
- `PPT/PPTX`: 60s

---

## 6. EdgeParse in-process 协议

`EdgeParse` 不走 HTTP，不做 sidecar。Rust worker 内部只保留稳定 adapter trait。

```rust
pub trait PdfDigitalParser {
    async fn parse_pdf(
        &self,
        document_id: &str,
        filename: &str,
        bytes: &[u8],
        page_filter: Option<&[u32]>,
    ) -> anyhow::Result<DocumentIR>;
}
```

约束：

- 必须支持 page filter
- 必须输出 `SourceLocator.page + bbox`
- 必须在 adapter 层做字符清洗，禁止脏文本直接下传

---

## 7. MinerU OCR fallback 协议

### 7.1 外部调用抽象

worker 内部保留独立 client trait：

```rust
pub trait PdfOcrFallbackClient {
    async fn parse_pdf_pages(
        &self,
        document_id: &str,
        filename: &str,
        bytes: &[u8],
        page_numbers: &[u32],
    ) -> anyhow::Result<DocumentIR>;
}
```

### 7.2 使用边界

MinerU 只在以下场景被触发：

1. 页级 probe 判定为扫描页
2. `EdgeParse` 对该页输出为空或明显异常
3. 文本层页出现严重乱码

### 7.3 不允许的使用方式

以下行为在 V1 中禁止：

- 把整份 PDF 无条件送到 MinerU
- 同一页同时接受 `EdgeParse` 与 `MinerU` 块级结果，再做复杂 merge

---

## 8. PPT/PPTX 的解析协议约束

`office-parser-jvm` 中的 PPT/PPTX adapter 必须同时产出：

1. `SlideText` / `SlideNotes` block
2. `SlideImage` block 与 render 资产

render 资产要求：

- `slide_index` 稳定
- 产物写回对象存储前，可先落临时文件
- 返回 IR 时必须带 `AssetIR(storage_path=temporary://...)` 或最终对象路径

worker 接手后负责：

- 镜像资产到对象存储
- 更新最终 `storage_path`

---

## 9. 失败语义

### 9.1 parser 失败

parser 失败应返回明确错误，不允许退化成二进制 `lossy bytes -> text`。

### 9.2 可重试错误

仅以下情况允许标记 `retryable=true`：

- service timeout
- 临时 IO 错误
- 外部 MinerU 5xx

以下情况必须是不可重试：

- 文档损坏
- 不支持格式
- schema 校验失败

---

## 10. 验收标准

### 10.1 service 层

- `office-parser-jvm` 能稳定响应 `docx/xlsx/ppt/pptx`
- 错误码语义清晰
- 并发和超时可配置

### 10.2 PDF 层

- `EdgeParse` 仍是 PDF 主路径
- `MinerU` 只在 OCR fallback 触发
- 页级 backend 归属可追踪

### 10.3 资产层

- PPT/PPTX render 资产可稳定写回对象存储
- `Figure/SlideImage` 都能关联到唯一 asset

