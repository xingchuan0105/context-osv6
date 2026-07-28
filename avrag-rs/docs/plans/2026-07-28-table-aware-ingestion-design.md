# 表格感知灌库解析设计（Table-aware Ingestion）

| 项目 | 内容 |
|---|---|
| 状态 | **设计待评审** |
| 日期 | 2026-07-28 |
| 关联 | 诊断：`v2_20260727-201553` 全量（q088 计数失败、q097/q098 PDF 扁平化混淆）；最佳实践核对见 §2 |
| 范围 | 灌库解析→切块→入库，**仅 txt/md/csv/xlsx 等天然结构化文档**；PDF 不在本期范围（维持现状 liteparse 扁平化路径）；不改运行时检索/评测；不重灌存量（仅新增/更新文档受益，见 §7 迁移） |

---

## 0. 一句话

表格信息丢失发生在**灌库解析的两个环节**：解析器把表格压成扁平文本（PDF liteparse 合并 glyph、office 剥离 XML），切块器再按 512 token 从行中间砍断。本设计在 IR 层引入**结构化表格块**，按来源分派解析器，切块对表格**原子化/按行组**处理——让"表格结构"成为数据的一等属性，而不是运行时再去猜文本。

---

## 1. 证据（为什么必须在灌库层做）

| 证据 | 出处 |
|---|---|
| q088 计数题三次临时代码三个错数（9/42/56），模型编造"缺 309/302"自洽 | `v2_20260728-030557` q088 dump |
| chunk 接缝恰在表格行 309 中间（`f033308e`/`507441cf`），跨块读行必然塌缩 | 任务C 诊断 |
| 白药 PDF 的 banner 数字（"638个业务对象（L3）"）与组名在扁平文本里相邻 → q098 张冠李戴 | 任务B 诊断；`parser/liteparse_ir.rs:54` coalescer 合并 glyph |
| chunk 无顺序元数据（UUIDv4、page=1、source_locator 全空），运行时拼 chunk 还原表格**不可行**（已实测否决） | `rag_text_chunks` 实测 |
| xlsx 入库即被剥成一块扁平文本（office-parser-jvm `main.rs:305-349`，伪 row_range） | `bins/office-parser-jvm` |

## 2. 最佳实践核对（2025–2026）

| 工具 | 表格能力 | 形态 | 适配判断 |
|---|---|---|---|
| **Docling**（IBM） | TableFormer 模型，独立 benchmark 表格单元格准确率 ~97.9%（[procycons](https://procycons.com/en/blogs/pdf-data-extraction-benchmark/)），复杂版面开源最强 | Python，本地 | PDF 表格首选；需 sidecar 服务化 |
| **Marker** | 同级开源，表格略弱于 Docling | Python，本地 | 备选 |
| **LlamaParse** | 综合最强（[firecrawl 2026](https://www.firecrawl.dev/blog/best-pdf-parsers)） | **云 API** | 数据出域，不合本地/私有部署纪律，**不采用** |
| 灌库侧共识 | "First Mile"决定 RAG 质量（[murdio](https://murdio.com/insights/unstructured-data-problems/)）：版面/表格必须在摄入时保结构，下游无法补救 | — | 支持本设计方向 |
| 表格切块共识 | 表格原子化；大表按行组切并重复表头；带表名/章节上下文（LlamaIndex/Docling chunk 实践） | — | §5 采用 |

## 3. 现状缺口（已核实的管道地图）

管道：`worker pipeline → parse.rs（ParseRouter）→ DocumentIr → chunker.rs → materialize → index`

| 层 | 现状 | 缺口 |
|---|---|---|
| IR | `BlockType` 已有 `Table/SheetTable/SheetCellRange`（`ingestion/src/ir.rs:258`）；`SourceLocator` 已有 `table_index/row_range/col_range/sheet_name`（:382） | **没有任何解析器真正产出结构化表格内容** |
| txt/md/csv | TextParser 整文一页；CSV 当纯文本（`parser/text.rs`） | 编号行表/markdown 表无解析 |
| xlsx | office-parser-jvm 剥 XML 标签成一块扁平文本 | 网格被丢弃；**calamine 可直接读网格** |
| PDF | liteparse 出 bbox 文本run，coalesce 成扁平块；"table" 只是标签 | bbox 坐标被丢弃，可重建行列 |
| 切块 | `chunker.rs:385` 一律 512-token text-splitter，overlap ~16 token | 不感知块类型 → 行中间砍断（309 接缝） |
| 入库 | `chunk_type` 列空闲可用；`section_path` 全局为空 | 表格 chunk 无任何标识/上下文 |

## 4. 设计：结构化表格块（TableIr）

### 4.1 IR 表示

```rust
// ingestion/src/ir.rs 新增
pub struct TableIr {
    pub caption: Option<String>,        // 表名/标题（邻近文本提取）
    pub headers: Vec<String>,           // 列头（可为位置列名 col_1..n）
    pub rows: Vec<Vec<String>>,         // 网格单元格
    pub parse_confidence: TableConfidence, // High（确定性源）| Medium（PDF 重建）| Low
    pub notes: Vec<String>,             // 解析诊断（缺口/合并单元格/不规则行）
}
```

- `BlockType::Table` 的 block 内容从"扁平文本"升级为 `TableIr` + 其**markdown 序列化文本**（供 embedding/检索使用，保留文本面）。
- `SourceLocator` 填 `table_index/row_range/col_range/sheet_name/bbox`（schema 已就位，零迁移）。
- `parse_confidence` 入库到 block/chunk metadata——**低置信表格在检索/评测侧可被识别**（对应此前的"保守边界"共识：绝不输出貌似可靠的结构）。

### 4.2 分源解析器

| 来源 | 解析器 | 置信度 | 说明 |
|---|---|---|---|
| txt/md/rst | `TextTableParser`（Rust 新写）：编号行表锚点（行首数字+有限词表/列位规律）+ markdown 管道表（`\| a \| b \|`） | High | 格式白名单制；自证不通过 → 降级为普通文本块，**不输出垃圾结构** |
| csv/tsv | `csv` crate（成熟库，不用 calamine 也可以） | High | 现在被当纯文本压平，改为逐行网格 |
| xlsx/xls | **calamine**（Rust 原生，无 JVM 依赖）直接读网格 | High | 替换 office-parser-jvm 的 xlsx 路径（docx/pptx 仍走原服务，xlsx 从此不吃标签剥离） |
| PDF（数字文本层） | **bbox 表格重建器**（Rust）：liteparse 已输出 glyph 坐标——按 y 聚类行、x 聚类列，重建网格；有框线表另加线框探测 | Medium | 不引入新依赖；覆盖白药类无框线表的大部分 |
| PDF（扫描/重建失败） | 现状 Paddle OCR 扁平文本 + `parse_confidence=Low` 标注 | Low | 诚实降级 |
| PDF（高精度需求，Phase 2 可选） | **Docling sidecar 服务**（Python，沿用 office-parser-jvm 的 service 模式，`OFFICE_PARSER_BASE_URL` 同构） | High | TableFormer 97.9% 单元格准确率；本地部署；**Phase 2 才做**，见 §8 |

### 4.3 PDF 边界（诚实声明）

- 数字文本层 PDF：bbox 重建覆盖"对齐良好的表格"；合并单元格/跨页表按 Medium 标注。
- 扫描件：仍需 OCR，结构恢复有限——Phase 2 的 Docling（带表格模型）才是正解。
- 无框线且对齐差的：可能重建失败 → Low 标注 + 保留扁平文本。**不承诺恢复不存在的结构**。

## 5. 表格感知切块

`chunker.rs` 对 `BlockType::Table` 块新增专用臂：

1. **原子化**：表格 ≤ token 预算（512）→ 整块一个 chunk，**绝不再从行中间砍断**（消灭 309 接缝）。
2. **行组切分**：超预算 → 按行组切（每组含 **重复表头行** + N 行数据），组与组之间不 overlap 文本行。
3. **上下文注入**：chunk 文本 = `表名/章节路径（section_path）` + markdown 表格片段。section_path 由块序的最近 Heading 链生成（顺带填补全局 section_path 为空的空白）。
4. **元数据**：`chunk_type="table"`、`table_index`、`row_range` 落入 `rag_text_chunks.chunk_type` 与 metadata（列已存在）。
5. 检索面：embedding 作用于"表头+行组"的序列化文本（行级语义可被 dense/lexical 命中）；`pg_bigm` 对表格行文本同样有效。

## 6. 端到端数据流

```
文件 → ParseRouter
  txt/md/csv ──→ TextTableParser ──┐
  xlsx ────────→ calamine ─────────┤
  pdf ─→ liteparse → bbox重建 ─────┤（失败→扁平+Low）
                    ↓
        BlockType::Table { TableIr, markdown_text }
                    ↓
        chunker: 原子 / 行组+表头重复 + section_path
                    ↓
        rag_text_chunks（chunk_type="table", table_index, row_range）
```

## 7. 迁移与兼容

- **零 schema 迁移**：`document_blocks`、`rag_text_chunks.chunk_type`、SourceLocator 字段全部已存在。
- **存量数据**：不重灌。`parser_backend` 加版本标记（如 `text-table-v1`），新旧数据共存；评测语料如需受益，用 `E2E_FORCE_INGEST=1` 重灌 e2e 库（一次性）。
- **评测配套**：灌库解析改变后，golden `source_chunks` 子串匹配可能受表格行序列化格式影响——验收跑后核对 thesis/ipd/baiyao 子集的检索轨命中率。

## 8. 切片

| 切片 | 内容 | 验证 |
|---|---|---|
| T1 | `TableIr` + TextTableParser（编号行表+markdown 表，白名单+自证降级） | fixture：IPD 表 370 行/六阶段计数断言（验证 59/发布 30/309 存在）；不规则输入降级断言 |
| T2 | chunker 表格臂（原子/行组+表头重复+section_path） | 单测：大表行组切分无行断裂、表头重复、chunk_type 落库 |
| T3 | xlsx calamine 路径（office-parser-jvm 替换或旁路）+ CSV 接 csv crate | fixture xlsx → 网格断言 |

**本期明确不做**：PDF（bbox 重建、Docling sidecar 均另议，维持 liteparse 现状）、行级实体抽取、运行时检索排序变更、重灌存量。

## 9. 非目标

- 不做行级实体/关系抽取（表格行进知识图谱是另一个课题）。
- 不改变运行时检索排序（表格 chunk 只是更好的文本面；`table_count` 类运行时原语**不再需要**——结构已在数据里）。
- 不接云解析 API（LlamaParse 出域）。
- 不重灌存量生产数据（除非用户另行决定）。

## 10. 风险

| 风险 | 缓解 |
|---|---|
| TextTableParser 规则脆弱（用户已指出缺行/空格不齐场景） | 白名单 + 自证 + 显式降级（§4.2）；绝不输出貌似可靠的结构 |
| bbox 重建对无框线表不准 | Medium 置信度标注 + 保留扁平原文；Phase 2 Docling 兜底 |
| 表格序列化改变影响 golden 检索匹配 | T5 验收核对；必要时修 golden source_chunks |
| calamine 大文件性能 | xlsx 通常小；超限降级 office 服务 |

---

**下一步**：评审通过后按 T1→T2→T3→T4→T5 顺序开工（T1/T2 可并行，T4 独立）。
