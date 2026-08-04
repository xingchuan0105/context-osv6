# 解析管线：anydoc 广覆盖（除 PDF）+ liteparse PDF + 文本/代码兜底

| 项目 | 内容 |
|---|---|
| 类型 | 设计决策（解析管线路由重构） |
| 日期 | 2026-08-05 |
| 范围 | 用 [firecrawl/anydoc](https://github.com/firecrawl/anydoc) 替换 office-direct，并尽量把 anydoc 支持的非 PDF 格式纳入产品路由；PDF 仍 liteparse；文本/代码长尾仍 markitdown |
| 分支 | 本地 `master`（solo trunk） |
| 前序 | [`2026-08-02-parser-pipeline-direct-readers.md`](2026-08-02-parser-pipeline-direct-readers.md)（**本文取代其 Office 直读决策**）、[`../engineering/2026-07-29-markitdown-hard-gate-handover.md`](../engineering/2026-07-29-markitdown-hard-gate-handover.md) |
| 状态 | **核心切片已落地（S1–S4）**；S5 基线重灌/e2e 待跑 |
| 实测 | 2026-08-05 本机 WSL2，`/tmp/pdf_spike` 同语料；产物 `/tmp/anydoc_bench/` |

---

## 0. 一句话结论

**最大化 anydoc 覆盖面，但 PDF 永不走 anydoc：**

| 层 | 后端 | 范围 |
|---|---|---|
| **A. anydoc** | `firecrawl-anydoc` / anydoc CLI → GFM markdown | 凡 anydoc 声明支持、且 **非 PDF** 的格式（见 §2） |
| **B. liteparse** | `lit parse --format markdown --no-ocr` | **仅 PDF**（扫描件仍走现有 PaddleOCR 分流） |
| **C. markitdown** | 现有子进程 | anydoc **不支持** 的纯文本/代码/配置长尾（txt/md/json/yaml/html/code…） |
| **D. PaddleOCR** | 现有 | 独立图片（png/jpg/…） |

- **删除** `office-direct` 全路径（Python 包、soffice 回环、`LocalParseKind::OfficeDirect`、`ParseBackend::OfficeDirect` 现役发射）。无兼容层、无双后端。
- **pptx 后处理**：anydoc 输出后仍做 **pptx-only** hex strip（与 08-02 决策一致）。
- 表格输出为 **GFM 管道表**（实测确认），可继续走 `blocks_from_markdown` → IR / chunk / struct-query。

> 本方案**取代** 2026-08-02 的「Office → office-direct（mammoth/openpyxl/python-pptx + soffice）」决策；**保留**「PDF → liteparse」「图片 → PaddleOCR」；**收窄** markitdown 为「anydoc 不支持的文本/代码」。

---

## 1. 为何换（证据）

### 1.1 速度（2026-08-05，`/tmp/pdf_spike`，anydoc 0.1.2）

| 文件 | anydoc | office-direct | markitdown | liteparse |
|---|---|---|---|---|
| 万科2024年报.pdf 9.9MB | 9.7s（**质量否决**，见 §1.2） | — | 39s | **4.6s** |
| thesis.docx 0.5MB | **24ms** | 616ms | 1.6s | 4.0s |
| huawei.xlsx 0.1MB | **6ms** | 406ms | 346ms | 2.2s（丢行） |
| MTL.pptx 23.7MB | **30ms** | 247ms | 278ms | 15s |
| binary .doc/.ppt/.xls | **0–2ms** | 1.4–2.1s（含 soffice） | — | — |

Office 类：**1～2 个数量级**；旧二进制 **去掉 LibreOffice 冷启动与并发串行瓶颈**。

### 1.2 质量（同语料探针）

| 维度 | anydoc | 对照 |
|---|---|---|
| xlsx 行完整性 | **370/370** | office-direct 370；liteparse 108（LO→PDF 丢行） |
| 表格格式 | **GFM 管道表**（`\| --- \|`），无 HTML table | office-direct 同为管道表；单元格内换行 anydoc 压空格，office-direct 用 `<br>` |
| docx 结构/CJK | 与 markitdown / office-direct 同量级 | — |
| pptx 源 hex 残渣 | **保留**（与 markitdown 相同，6 段） | office-direct / liteparse 已清 → **需 pptx-only strip** |
| PDF 中文财报 | **字体乱码**（`以下简称"本报告"`→`措色疆化北）`）；标题 36 vs liteparse 679 | **PDF 不得用 anydoc**（底层 pdf-inspector，与 08-02 否决一致） |
| 旧 xls 非空格 | 与 office-direct 同为 1192 nonempty cells | markdown 体积因空单元格填充风格不同（无丢 sheet） |

### 1.3 系统收益（非仅单文档耗时）

| 收益 | 说明 |
|---|---|
| 去掉 LibreOffice | 部署不再强制 writer+calc+impress；无 `OFFICE_SOFFICE_MAX_CONCURRENT` 串行 |
| 去掉 mammoth/openpyxl/python-pptx 专用链 | worker 依赖面收敛为 anydoc wheel/CLI |
| 格式面扩大 | anydoc 原生支持 docm/pptm/xlsm/xlsb、ODF、RTF、EPUB、CSV 等（产品当前未开的可本期打开） |
| 统一 GFM 序列化 | 各格式共用同一 serializer；表格转义等修一处全格式受益 |

---

## 2. 目标路由表

### 2.1 anydoc 支持面 vs 产品面（**尽量多**）

anydoc 官方格式（**排除 PDF** 后全部目标纳入 anydoc 路径）：

| 族 | 扩展名（产品应接受） | 现路由 | 目标 |
|---|---|---|---|
| Word | `doc`, `docx`, `docm` | doc/docx→office-direct；**docm 未开** | **anydoc**；**新增 docm** |
| PowerPoint | `ppt`, `pps`, `pot`, `pptx`, `pptm`, `ppsx`, `ppsm` | ppt/pptx→office-direct；其余未开 | **anydoc**；**新增** 未开扩展（至少 `pptm`/`ppsx`/`ppsm`；`pps`/`pot` 按需） |
| Excel | `xls`, `xlsx`, `xlsm`, `xlsb` | xls/xlsx→office-direct；其余未开 | **anydoc**；**新增 xlsm/xlsb** |
| OpenDocument | `odt`, `ods`, `odp` | **未开** | **anydoc**；**本期新增** |
| RTF | `rtf` | **未开** | **anydoc**；**本期新增** |
| EPUB | `epub` | **未开** | **anydoc**；**本期新增** |
| CSV | `csv` | markitdown | **anydoc**（signature-less，须显式 format 或扩展名） |
| PDF | `pdf` | liteparse | **仍 liteparse**；**禁止 anydoc** |

### 2.2 明确不归 anydoc（产品仍支持）

| 格式 | 后端 | 理由 |
|---|---|---|
| `pdf` | liteparse（+ 扫描件 PaddleOCR） | 质量否决 anydoc/pdf-inspector |
| `txt` `md` `rst` `tsv` `json` `toml` `yaml` `yml` `html` `htm` | markitdown | anydoc 不宣称支持；继续现路径 |
| 代码扩展（`rs`/`py`/…） | markitdown | 同上 |
| `png`/`jpg`/… | PaddleOCR | 不变 |

> **TSV**：anydoc 仅列 CSV。TSV **不**冒充 CSV 塞 anydoc（分隔语义不同）；仍 markitdown，除非后续实测通过再改。

### 2.3 路由伪代码

```text
ext = normalize_extension(filename)
match ext:
  image_*          → PaddleOcrImage
  pdf              → LiteparseV2Pdf
  anydoc_office_*  → Anydoc          # §2.1 非 PDF 全集
  text_or_code_*   → Markitdown      # §2.2
  _                → unsupported_file_type
```

`ensure_supported_file_type` / `is_supported_extension` / `mime_matches_extension` **同步扩大**到 §2.1 新增扩展；mime 表缺省的用 IANA / 常见 office mime，无法严格匹配时与现网「宽松 office mime」策略对齐（实施时对照 `router/mime.rs` 现有 docx/xlsx 分支补齐，不为未映射扩展开「任意 mime」洞）。

---

## 3. 实现形态（最小且完整）

### 3.1 原则

- **No backward compatibility tax**：删除 office-direct，不留 feature flag 双路径。
- **Simplest that fully works**：子进程调用 anydoc（与 markitdown/liteparse 同构：temp in/out + timeout + kill_on_drop），产出 markdown → 已有 `blocks_from_markdown`。
- **不做** 进程内 Rust crate 直链 anydoc（绑定 worker 编译链、版本耦合重）；若后续要零 spawn，另开设计。本期 **CLI / console-script 子进程**。

### 3.2 Rust 侧

1. **`LocalParseKind`**

```rust
pub enum LocalParseKind {
    Markitdown,     // 文本/代码/tsv/html… 长尾
    LiteparseV2Pdf, // 仅 PDF
    Anydoc,         // §2.1 非 PDF 广覆盖（取代 OfficeDirect）
}
```

- **删除** `OfficeDirect` 变体（路由测试、`parse_route.rs` 分支一并改）。

2. **`ParseBackend`**

- **新增** `Anydoc`（wire `anydoc`）。
- **`OfficeDirect` 标为 historical IR only**（与 `CalamineExcel` 等同）：`is_historical_ir_only()` 纳入；新 ingest **不再 emit**。不保留「写时兼容」双写。
- 注释更新：`Markitdown` 范围改为「anydoc 不支持的文本/代码」。

3. **新模块** `crates/ingestion/src/parser/anydoc.rs`（骨架对齐 `markitdown.rs` / `office_direct.rs`）：

| 项 | 约定 |
|---|---|
| 二进制 | `ANYDOC_BIN`，默认 `anydoc-extract`（见 §3.3） |
| 超时 | `ANYDOC_TIMEOUT_MS`，默认 `120_000` |
| 输入 | bytes → temp file（保留扩展名，供 format 检测 / CSV 扩展名） |
| 输出 | stdout markdown **或** temp `.md`（二选一，实施取 CLI 最简单形态） |
| 失败 | hard-fail，**不**降级 markitdown / office-direct |
| IR | `primary_backend = Anydoc`，blocks 经 `blocks_from_markdown` |
| 后处理 | 若 `ext ∈ {pptx,pptm,ppsx,ppsm,ppt,pps,pot}` → `strip_pptx_hex_runs(md)` |

4. **`execute_local_parse`**（`bins/worker/.../parse_route.rs`）：`Anydoc` 分支调 `parse_anydoc_document_ir`；删除 `OfficeDirect` 分支。

5. **hex strip（Rust，pptx 族 only）**

```text
// 与 08-02 一致：仅演示/幻灯片路径；阈值偏保守
re: [0-9A-Fa-f]{100,}  → 删除
```

- **不得**用于 docx/xlsx/csv/rtf/epub/文本/代码。
- 治本不在 anydoc 内（源文件粘贴残渣）；strip 为产品侧防御，与旧 office-direct 形状跳过 + 正则同目标。

### 3.3 调用形态（worker 部署）

**推荐（与现网 console-script 一致）**：薄包装

```text
# avrag-rs/scripts/anydoc-extract/ 或并入现有 scripts 布局
anydoc-extract <input-path> <output.md>
```

实现可选其一（实施时取更简单且可装进 Docker 的）：

| 选项 | 说明 | 倾向 |
|---|---|---|
| **A. Python 包装** `firecrawl-anydoc` | `anydoc.to_markdown(path)` → write out；CSV 用 path 扩展名 | **首选**（装 wheel 即得，与现 Python 工具链一致） |
| **B. 官方 example CLI** | `cargo install` / 拷 release binary | 若 worker 镜像已偏 Rust 可考虑 |
| **C. 裸 `python -c`** | 不推荐：参数转义与超时难控 | 否 |

失败语义：非零退出 + stderr；Rust 透传为 parse 错误。  
**无** soffice 信号量。

Docker / `avrag-runtime.Dockerfile` / deploy 脚本：

- **加** `pip install firecrawl-anydoc`（钉版本）+ 安装 `anydoc-extract` entrypoint。
- **删** office-direct 包、LibreOffice **仅若** 全仓无其它强制依赖（struct 探针等需核对；若仅 office 旧路径使用 LO，可从 worker 运行时依赖移除或标 optional）。
- `OFFICE_DIRECT_*` / `OFFICE_SOFFICE_*` env **删除**（文档与 `.env.example` 同步）。

### 3.4 支持扩展与 MIME 表

`router/mime.rs`：

- `is_supported_extension`：并入 §2.1 新增扩展。
- `mime_matches_extension`：为 `docm`/`xlsm`/`xlsb`/`pptm`/`odt`/`ods`/`odp`/`rtf`/`epub` 等补常见 mime；未知但合法 office mime 时策略与现 `docx` 分支同级严格度（避免「扩展名任意、mime 任意」）。
- 路由测试：每个 **新增族** 至少一个 `assert_local_kind(..., Anydoc)`；旧 OfficeDirect 测试全部改为 Anydoc。

### 3.5 删除清单（实施完成定义）

| 删除 | 路径 |
|---|---|
| office-direct Python 包 | `avrag-rs/scripts/office-direct/` |
| Rust 模块现役调用 | `parser/office_direct.rs` **删除** 或缩成 historical 注释 + 零引用（优先 **整文件删除**，historical backend 枚举值保留） |
| 路由/worker 分支 | `OfficeDirect` |
| 部署 | Dockerfile / deploy 中 office-direct、相关 LO 注释 |
| 文档 | runbook、README 解析段改指向本文；08-02 文首 SUPERSEDED |

---

## 4. 决策记录

| 决策 | 结论 | 依据 |
|---|---|---|
| PDF | **永远 liteparse**，禁止 anydoc | 万科年报乱码 + 弱标题；08-02 否决 pdf-inspector |
| Office / ODF / RTF / EPUB / CSV | **anydoc** | 速度与覆盖面；GFM 表实测可用 |
| 旧 doc/ppt/xls | anydoc **原生**，不再 soffice | 实测 0–2ms；去掉 LO |
| pptx hex | **Rust 侧 pptx 族 only strip** | anydoc 忠实保留源残渣 |
| 文本/代码/tsv/html/json… | **仍 markitdown** | anydoc 不支持；避免硬塞 |
| CSV | **anydoc**（从 markitdown 迁出） | anydoc 官方支持；GFM/表更一致 |
| TSV | **仍 markitdown** | anydoc 未列 TSV |
| 双后端 / feature flag | **不做** | design principles：无兼容税 |
| 接入形态 | **子进程 + temp 文件** | 与 markitdown/liteparse 同构，最小风险 |
| `ParseBackend::OfficeDirect` | historical only，新路径 emit `Anydoc` | 存量 IR 可反序列化 |
| 进程内 link anydoc crate | **本期不做** | 编译/发布耦合；可二期 |

### 4.1 表格与下游契约

- anydoc 表 = **GFM pipe table**（分隔行 `| --- |`），**无** `<table>` HTML（本语料）。
- `blocks_from_markdown` 可直接消费；struct-query 表格阶段依赖 markdown 管道表时 **应可工作**。
- **差异**：长单元格 anydoc **不**用 `<br>`、多行压成单行空格分隔。若某 skill/管道依赖 `<br>` 分行，属已知差异；实施时用 huawei xlsx 跑一遍 struct-query smoke。
- xlsx 大表仍可能被 `TextSplitter` 中段切开（08-02 §5.4 遗留，**本期不改 chunker**）。

### 4.2 图片 / 附件

- anydoc：内嵌图以 alt 文本进 markdown，原始字节在 document model（子进程只取 markdown 时 **不**进 asset 管线）——与 office-direct「不抽 docx 图实体」同级缺口。
- **本期不**做 anydoc assets 旁路提取。

---

## 5. 实施切片（有序，可验证）

| 步 | 内容 | 验证门 |
|---|---|---|
| **S0** | 本文 + 08-02 SUPERSEDED 横幅 + `docs/README.md` 解析段 | 文档可读 |
| **S1** | `anydoc-extract` 包装 + 本地 `/tmp/pdf_spike` 四件套 + binary 回环计时/行数探针脚本（可复用 `/tmp/anydoc_bench`） | xlsx 370 行；docx/pptx 非空；doc/ppt/xls Ok |
| **S2** | `parser/anydoc.rs` + `ParseBackend::Anydoc` + `LocalParseKind::Anydoc` + router 映射（含新增扩展）+ pptx hex strip | `cargo test -p ingestion --lib` 路由/单元 |
| **S3** | worker `parse_route` 接线；删 office-direct 调用 | worker 编译；smoke 上传 xlsx/docx |
| **S4** | 删除 `scripts/office-direct`、env、Dockerfile 依赖；runbook 更新 | 镜像构建；无 `office-direct-extract` 引用 |
| **S5** | 基线语料抽样重灌 + 相关 e2e / staging ingest（按 `e2e-gates`；**先估时并征得同意**） | 无解析回归；struct-query 抽样 |

每步通过再进下一步；失败不堆兼容分支。

---

## 6. 风险与边界

| 风险 | 缓解 |
|---|---|
| 中文复杂 PDF 被误路由到 anydoc | 路由 **硬编码** pdf→liteparse；单测锁死 |
| anydoc 版本回归 | 钉 `firecrawl-anydoc==x.y.z`；S1 探针进脚本可重复跑 |
| 新增格式 mime 拒收 | S2 补 mime 表 + 上传 API 单测 |
| CSV 无 magic、扩展名错误 | 与 anydoc 一致：扩展名 / 显式 format；错误扩展 → hard-fail |
| 大 xlsx 内存 | 超时 + worker 现有内存策略；异常样本记入评测 |
| LO 其它用途 | 删除前 `rg soffice|libreoffice` 全仓；若仅 office-direct 依赖则移出镜像 |
| 官方 bench 语料不可用 | 以产品 `/tmp/pdf_spike` + 基线语料为准，不依赖 anydoc 私有 100 篇 |

**明确不做（本期）**

- PDF 走 anydoc / 双 PDF 后端
- anydoc OCR 扫描件
- markitdown 完全删除（文本/代码仍要）
- chunker 表格感知重写
- anydoc 内嵌图 asset 提取

---

## 7. 配置一览（目标态）

| 变量 | 默认 | 含义 |
|---|---|---|
| `ANYDOC_BIN` | `anydoc-extract` | 子进程可执行文件 |
| `ANYDOC_TIMEOUT_MS` | `120000` | 单文档超时 |
| `LITEPARSE_BIN` / 现有 liteparse 扫描阈值 | 不变 | PDF |
| `MARKITDOWN_*` | 不变 | 文本/代码 |
| ~~`OFFICE_DIRECT_*`~~ | **删除** | — |
| ~~`OFFICE_SOFFICE_*`~~ | **删除** | — |

---

## 8. 文档与索引义务（与实施同 PR/同会话）

1. 本文为 **解析层现行设计真相**（取代 08-02 Office 段）。
2. `docs/plans/2026-08-02-parser-pipeline-direct-readers.md` 文首加 SUPERSEDED 横幅，指向本文。
3. `docs/README.md` 解析/入库 bullet 更新为：PDF→liteparse；**广覆盖 Office/ODF/RTF/EPUB/CSV→anydoc**；文本/代码→markitdown；图片→PaddleOCR。
4. `docs/runbooks/worker-dev.md` 解析段同步（实施 S4 时改，避免文档超前代码过久）。
5. `docs/engineering/2026-07-29-markitdown-hard-gate-handover.md` 横幅可再注「Office 段已被 2026-08-05 anydoc 方案取代」。

---

## 9. 附录：anydoc 能力备忘

- 输出：**GitHub-Flavored Markdown**（统一 document model + GFM serializer）。
- 表格：管道表；支持合并单元格与表头行（库能力；产品侧靠 GFM 消费）。
- PDF：内部 pdf-inspector，**产品禁用**。
- 绑定：Rust / Node / Python（`firecrawl-anydoc`）；本期用 Python 包装或独立 binary 子进程。
- License：MIT。

### 附录 B：实测命令备忘（非 CI）

```bash
# 环境（示例）
python3 -m venv /tmp/anydoc_bench/venv
/tmp/anydoc_bench/venv/bin/pip install firecrawl-anydoc
# 探针脚本曾用：/tmp/anydoc_bench/run_bench.py
# 报告：/tmp/anydoc_bench/report.md
```

---

*文档写于 2026-08-05；依据 anydoc 0.1.2 本机实测 + 官方 README 格式表。实施以 S0–S5 顺序推进。*
