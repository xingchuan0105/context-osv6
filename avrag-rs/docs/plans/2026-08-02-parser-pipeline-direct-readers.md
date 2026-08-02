# 解析管线重构：Office 直读 + PDF liteparse + markitdown 兜底

| 项目 | 内容 |
|---|---|
| 类型 | 设计决策（解析管线路由重构） |
| 日期 | 2026-08-02（v2，吸收 code review 修订） |
| 范围 | PDF / docx / pptx / xlsx / doc / ppt / xls / 文本代码类 的解析路由；四方案实测对比 |
| 分支 | 本地 `master`（solo trunk；未 commit） |
| 前序 | `docs/engineering/2026-07-29-markitdown-hard-gate-handover.md`（本文部分取代其「markitdown 唯一解析器」决策）、`docs/liteparse-paddle-ingestion-architecture-2026-06-13.md`（已 supersede） |
| 状态 | 方案已定，待实施 |
| 审查 | 2026-08-02 code review 已吸收：撞名、调用形态、soffice 失败语义、chunker 表格切分、hex 误杀面等逐条修订 |

---

## 0. 一句话结论

**按格式分工，不做全类单一解析器：**
- **PDF → liteparse**（PDFium 原生，质量+速度实测最优）
- **docx/xlsx/pptx → 直读专用库**（mammoth / openpyxl / python-pptx）+ 敲净
- **旧二进制 doc/ppt/xls → soffice 无损转 OOXML → 直读**（已实测验证）
- **文本/代码类（txt/md/rst/csv/html/json/yaml/code…）→ markitdown 兜底**
- **图片 → PaddleOCR**（现状不变）

> ⚠️ 本方案**部分取代** 2026-07-29/31 的「markitdown 唯一解析器」决策。实施时必须同步清理代码内旧决策注释（见 §6.4），并在 `docs/engineering/2026-07-29-markitdown-hard-gate-handover.md` 顶部加「部分被 2026-08-02 取代」横幅。

---

## 1. 实测证据（2026-08-02，本机 WSL2）

### 1.1 文件与计时

| 文件 | 大小 | liteparse | markitdown | pdf-inspector | 直读 |
|---|---|---|---|---|---|
| 万科2024年报.pdf | 9.9MB | **3.76s** | 44.66s | 2.62s | — |
| thesis_y_refrigeration.docx | 484KB | 3.06s¹ | 3.06s¹ | — | **0.82s** |
| huawei_ipd_370_activities.xlsx | 90KB | 1.83s | 1.19s | — | **0.34s** |
| MTL流程建设服务介绍.pptx | 23.7MB | 13.76s | 1.20s | — | **0.19s** |

> ¹ docx 的 liteparse 与 markitdown 恰好同为 3.06s，已复核两次计时日志确认是巧合而非笔误。

### 1.2 关键质量发现

| 维度 | 结论 |
|---|---|
| **PDF 附注区字体编码** | pdf-inspector **系统性数字误读**（`2024年12月31日`→`8689年78月97日`、`10,000`→`A0,000`，~1200 token）+ 2,155 行 hex 垃圾 → **财报语料一票否决**。markitdown 无标题（0 个 `#`）、页眉页脚噪声、44.66s。liteparse 干净、679 标题、表格最干净、数字零误读 |
| **docx 表格单元格** | liteparse 经渲染路径**长单元格按换行拆碎**；markitdown/直读单元格完整 |
| **docx 图片膨胀** | 直读（mammoth 默认）把 13 张 EMF 图 base64 内嵌 → 466KB；**`convert_image` 关内嵌后 150.7KB**（与 markitdown 151.0KB 一致），正文分毫未动 |
| **xlsx 数据完整性** | liteparse 经 LibreOffice 转 PDF **丢 71% 行（108/370）+ 2 列**；markitdown/直读全量 370×6 |
| **pptx hex 残渣** | markitdown/直读忠实提取源文件第 2/3/9/15/16/24 页文本框里的 272 字符 hex 串（源文档粘贴残留，非解析器垃圾）；liteparse 渲染路径恰好丢弃。**直读跳过纯 hex 形状后归零、CJK 无损** |
| **pptx 结构** | liteparse 65 标题 > markitdown 4 > 直读 0（平铺）。代价：pptx 渲染 = 13.65s（unoconv 冷/热 14.54s/13.19s 无实质改善——瓶颈是 Impress 渲染本身） |

### 1.3 旧二进制 doc/ppt/xls → OOXML 回环（实测通过）

```
soffice --headless --invisible --convert-to docx test.doc   # 47K→34K
soffice --headless --invisible --convert-to pptx test.ppt   # 1.79M→1.42M
soffice --headless --invisible --convert-to xlsx test.xls   # 123K→44K
# 直读回环：docx 0.14s 全文、pptx 0.14s 38 页、xlsx 0.27s 数据全保留
```

二进制→OOXML 是**无损容器迁移**，绕开了"转 PDF 丢列丢行"的老坑（对比 §1.2 xlsx 行）。

### 1.4 环境前置（生产必需）

| 组件 | 版本 | 用途 | 生产必需 |
|---|---|---|---|
| `lit`（liteparse） | v2.10.0（crates.io） | PDF | 是 |
| `markitdown` | Python 0.1.5 | 文本/代码兜底 | 是 |
| `mammoth` / `openpyxl` / `python-pptx` / `markdownify` | — | Office 直读 | 是 |
| LibreOffice | 24.2.7 | doc/ppt/xls→OOXML | 是（**需 writer+calc+impress 三组件齐全**） |
| `pdf-inspector`（pdf2md/detect-pdf） | v0.7.0 | 仅 spike 评估用，**生产不引入** | 否 |

---

## 2. 目标路由表

| 格式 | 方案 | 后端 | 备注 |
|---|---|---|---|
| `pdf` | **liteparse** | `lit parse --format markdown --no-ocr`（子进程） | 扫描页检测后转 PaddleOCR（§5.2） |
| `docx` | **直读** | mammoth(不内嵌图) + markdownify | 或 markitdown（同库，输出一致） |
| `xlsx` | **直读** | openpyxl → 管道表 | 0.34s 全量 |
| `pptx` | **直读** | python-pptx（跳过纯 hex 形状） | 结构平铺已知限制 |
| `doc` | soffice→docx→直读 | writer | 先转再直读 |
| `ppt` | soffice→pptx→直读 | impress | 先转再直读 |
| `xls` | soffice→xlsx→直读 | calc | 先转再直读 |
| `txt/md/rst/csv/tsv/json/toml/yaml/yml/html/htm` | **markitdown 兜底** | markitdown | 长尾低频 |
| 代码扩展名 | **markitdown 兜底** | markitdown | 或原生文本直读 |
| `png/jpg/jpeg/webp/gif/bmp` | **PaddleOCR** | 现有 `pdf/paddle.rs` | 不变 |

**不支持的边界**：其他扩展名维持 `unsupported_file_type`。

---

## 3. 实现形态（最小 diff）

### 3.1 Rust 侧

1. `ParseRoute` 现状为 `ParseRoute::{Local, PaddleOcrImage}` + `ParsePlan::{Local, External}`（`router/mod.rs:13,90`）。保持两路不变，扩展 `LocalParseKind`：

```rust
pub enum LocalParseKind {
    Markitdown,    // 保留：文本/代码兜底
    LiteparseV2Pdf,// 新增：pdf
    OfficeDirect,  // 新增：docx/xlsx/pptx/doc/ppt/xls（内部按扩展名分派）
}
```

2. **`ParseBackend` 新增变体——撞名规避（审查第 1 点）**：
   - ⚠️ 现有 `LiteParsePdf`（`ir.rs:62`）wire name 已是 `"liteparse_pdf"`（`ir.rs:111`）。**不能**新增 `LiteparsePdf` 变体，否则 snake_case 下两个变体同 wire name，无法共存。
   - **决策：新增 `LiteparseV2Pdf`（wire name `liteparse_v2_pdf`）**，与历史 `LiteParsePdf` 区分。理由：旧 liteparse 行块带 bbox/`source_locator`（liteparse-paddle 时代契约），新路径产 markdown→Paragraph/Heading（无 bbox）——**新老行形态不同，需可区分**，历史 IR 语义不受影响。
   - `OfficeDirect`（wire name `office_direct`）：与现有全部变体无冲突（已核对 ir.rs:54-85）。
   - `LiteparseV2Pdf` / `OfficeDirect` 均**不**加入 `is_historical_ir_only()`。
   - 技术债：`is_historical_ir_only()` 全仓**零调用点**（已 grep 核实），所谓"规则"目前只是注释。实施时要么接入一处校验/审计查询，要么删除该方法（见 §6.4）。

3. `router::route` 按 §2 映射；`doc/ppt/xls` 落到 `OfficeDirect`（脚本内先 soffice 转换）。

4. `execute_local_parse` 分支（沿用 `markitdown.rs` 子进程骨架：temp 文件 + timeout + `kill_on_drop`）：
   - `LiteparseV2Pdf` → 子进程 `LITEPARSE_BIN`（默认 `lit`）`parse <tmp> --format markdown --no-ocr`。
   - `OfficeDirect` → 子进程 `OFFICE_DIRECT_BIN`（见 §3.2 调用形态）`<tmp> <out>`。
   - 均产出 markdown → `blocks_from_markdown` → `DocumentIr`。

### 3.2 Python 侧（`scripts/office-direct/`，可运行版已存 `/tmp/pdf_spike/office_direct_extract.py`）

**调用形态（审查第 3 点）**：打包为 Python console-script `office-direct-extract`（`pyproject.toml` 暴露入口），装入 worker venv；`OFFICE_DIRECT_BIN` 默认 `office-direct-extract`（与 markitdown 同为 PATH 上的可执行文件，`Command::new(bin)` 直接 spawn）。脚本自身 `#!/usr/bin/env python3` + `chmod +x` 兜底支持 `OFFICE_DIRECT_BIN` 指向 .py 路径。

按扩展名分派：

| 扩展名 | 处理 |
|---|---|
| `docx` | mammoth `convert_image=img_element({src:""})` + markdownify（防 EMF base64 膨胀）；**输出后 strip 空图片占位 `![]()`**（见 §4.4） |
| `xlsx` | openpyxl `iter_rows(values_only)` → 管道表；每 sheet 前缀 `## <sheet>` 标题；合并单元格取左上值、跳过全空行 |
| `pptx` | python-pptx 逐页；**跳过 `re.match(r'^[0-9A-Fa-f]{40,}$', text)` 的纯 hex 形状** |
| `doc/ppt/xls` | 先 soffice 转换（独立临时 profile + 独立 outdir），再按上三行直读 |

**soffice 失败语义（审查第 4 点）**：
- **错误语义**：soffice 转换失败 / 产物缺失 / 超时挂死 → **文档 hard-fail**（`ParseWarning` code `office_convert_failed`），**不降级回 markitdown**（doc/ppt/xls 走 markitdown 是 plain-text 乱码，等于无兜底）。按现有「文档 Failed + ParseWarning；任务按重试策略 requeue」模式，重试 N 次后**终态失败**（避免无限 requeue）。
- **超时预算**：整体 `OFFICE_DIRECT_TIMEOUT_MS`（默认 120s）+ soffice 子进程独立 `OFFICE_SOFFICE_TIMEOUT_MS`（默认 90s，`kill_on_drop` 回收）；直读段吃剩余预算。
- **并发约束**：worker 级信号量 `OFFICE_SOFFICE_MAX_CONCURRENT`（默认 **1**）。LibreOffice 实例 ~600MB RSS，且 profile 锁并发竞争（liteparse 源码已注释该风险）；soffice 转换必须串行。
- **临时清理责任**：脚本内用 `tempfile.TemporaryDirectory` 持有 profile/outdir，RAII 保证异常/正常退出均清理；Rust 侧沿用现有 temp 文件清理。

### 3.3 后处理（**仅 pptx 路径**，审查第 7 点）

hex strip **不得全局套用**（代码块里的 git sha 拼接、长 hash、base64 都会误杀）。只对 pptx 输出兜一层防御：`re.sub(r'[0-9A-Fa-f]{100,}', '', md)`，且只作用于 pptx 直读结果（治本形状跳过为主，正则兜底为辅）。**不应用于** docx/xlsx/文本/代码/markitdown 兜底路径。

---

## 4. 决策记录

| 决策 | 结论 | 依据 |
|---|---|---|
| PDF 解析器 | **liteparse**（不用 pdf-inspector、markitdown） | 数字保真 + 结构 + 速度三优；pdf-inspector 附注区误读一票否决 |
| `ParseBackend` 命名 | **`LiteparseV2Pdf`（`liteparse_v2_pdf`）**，不复用 `LiteParsePdf` | 新老行形态不同需可区分；规避 wire name 撞名 |
| docx | 直读 mammoth 或 markitdown（同库） | 单元格完整、语义标题；**必须关图片内嵌 + 敲空占位** |
| xlsx | 直读 openpyxl（或 markitdown） | 全量数据；**禁止任何 LibreOffice→PDF 渲染路径** |
| pptx | 直读 python-pptx + 敲 hex（pptx 路径限定） | 速度 0.19s；接受平铺结构（需结构再评估 liteparse 13s） |
| 旧二进制 doc/ppt/xls | **soffice 转 OOXML 后直读**；转换失败 hard-fail | 无损容器迁移，回环实测通过；markitdown 无这些格式转换器 |
| 兜底 | markitdown | 长尾文本/代码；doc/ppt/xls 必须走 OOXML 转换，不得落 markitdown |
| unoconv | 不用 | 冷 14.54s / 热 13.19s，vs soffice 13.65s 无实质收益 |
| **xlsx 大表切块（审查第 6 点）** | **维持现状（TextSplitter 拆）+ 记录已知行为 + 待验证** | 见 §5.4 |
| **docx 空 src 图片（审查第 5 点）** | 直读不产 `embedded_images_json`，ImageWithContext 不触发；空占位脚本内 strip | 见 §4.4 |

### 4.4 docx 空 src 图片契约（审查第 5 点）

- 直读路径产出 markdown → `blocks_from_markdown` → `BlockIr`（仅 Paragraph/Heading）。**不设置 `metadata["embedded_images_json"]`**（该键仅 HtmlParser 设置，`html.rs:39`），故 `normalize_parsed_document` 的 `ImageWithContext` 分支（`parser/mod.rs:197-228`）**不会触发**，不会产出 `image_path: ""` 的垃圾 unit。
- 但 `![]()` 空占位会作为字面文本进入 Paragraph block → 脚本内 strip（`convert_image` 返回空 src 后，markdownify 产 `![]()`，再做一次 `!\[\]\(\)` / 空 src 变体清除）。
- **范围声明**：直读不提取 docx 内嵌图片实体（mammoth `src=""`），**docx 图实体不进 asset 管线**。如需图 → MM 索引，属独立 asset 提取缺口（本期不做，记录）。

---

## 5. 遗留与边界

### 5.1 扫描版 PDF —— **parity 非回归**（审查第 2 点）

现状 markitdown 对扫描版 PDF **同样无 OCR**（图片 OCR 只覆盖 standalone 图片）。本方案 liteparse 路由同样是文字抽取，**与现状持平，不是新引入的回归**。文档明确此论证，避免评审误读。

### 5.2 零 chunk 硬闸与重试（审查第 2 点追问）

- 零 chunk 完整性检查拒灌时，文档必须落为**终态失败**（`ParseWarning` + `parse_confidence=Low` 或明确失败原因），**不进入无限重试循环**。
- 对策：liteparse 路由后接**扫描页检测**（`lit is-complex` 或页字符数/可读率阈值），命中即转 PaddleOCR（复用 `pdf/paddle.rs` 整页渲染+OCR），**在零 chunk 检查前分流**。此链路为新增（§6.5）。

### 5.3 存量重灌与基线

- 路由变更后按新基线重灌 + 全量 nightly 复跑（对照旧 139/149）。
- **样本覆盖对照**（审查次要点）：本期实测 = 4 现代格式 + 3 二进制回环，对全类产品路由决策偏薄。实施前**先从基线语料抽取格式分布**，确认头部格式被覆盖；补充：扫描件 / 多栏 / 竖排 PDF、合并单元格 / 多 sheet / 公式 xls。

### 5.4 xlsx 大表切块行为（审查第 6 点，实测核实的现状）

- `blocks_from_markdown` 把 370 行管道表拼成**一个巨型 Paragraph block**（无 `#` 行）。
- `split_mode_for_file("xlsx")` = `SplitMode::Text`（`chunker.rs:167`）→ `TextSplitter` **纯 token 分片，无表格感知** → 大表**可能被拆在表格中间**。
- chunker 的"表格原子化"T2 臂（`chunker.rs:406-417`）只对 `BlockType::Table` + `TableIr::from_block` 生效；但 `blocks_from_markdown` 只产 Heading/Paragraph（TableIr 退役）→ **该臂当前为死代码**。
- **这是现状 markitdown 即有的行为（parity）**，非本方案引入。决策：
  - 本期维持现状，接受 TextSplitter 拆表 + struct-supervision 表格重检测兜底（现有链路）。
  - 可选优化（另立单）：`split_mode_for_file` 把 `xlsx/xls` 映射到 `SplitMode::Markdown`，令 `MarkdownSplitter` 按表格行边界切（text_splitter 支持表格行级切分），避免拆在行中。**待实施时用 370 行表验证**。

### 5.5 pptx 结构平铺

直读 0 标题；若语料 pptx 文本密度高、检索靠标题，评估 liteparse 渲染路径（13s/份）或自研 python-pptx 标题推断。

### 5.6 其他技术债（审查次要点）

- `is_historical_ir_only()` 零调用点：接入校验/审计或删除。
- 旧决策注释清理（见 §6.4）。
- `markitdown` 对 `doc/ppt/xls` 无转换器（会静默 plain-text 乱码）——现状 router 路由是假支持，必须摘除。

---

## 6. 建议实施顺序

1. Python 侧：`scripts/office-direct/` 落仓（pyproject + console-script + 单测：三格式 + 二进制回环 + soffice 失败路径 + 空占位 strip + 大表输出）。
2. Rust 侧：`LocalParseKind` 三变体 + `ParseBackend::{LiteparseV2Pdf, OfficeDirect}` + router 映射 + `execute_local_parse` 分支（复用 markitdown 子进程骨架）。**先改 §3.1 撞名方案再动手**。
3. 后处理层接入（pptx 限定 hex strip）。
4. **旧决策注释清理**（审查次要点）：`router/mod.rs:10`、`parse_route.rs:7`、`ir.rs:48-51` 的「文档全类唯一解析器 markitdown」注释改写；`2026-07-29-markitdown-hard-gate-handover.md` 顶部加「部分被 2026-08-02 取代」横幅。
5. 扫描 PDF 检测→PaddleOCR 链路（§5.2）。
6. 目标文件重灌 + `blocks_from_markdown` 契约回归 + 全量 nightly；先做 §5.3 基线格式分布对照。

---

## 7. 配置项汇总

| 变量 | 默认 | 说明 |
|---|---|---|
| `LITEPARSE_BIN` | `lit` | PDF 子进程 |
| `LITEPARSE_TIMEOUT_MS` | 120_000 | PDF 解析超时 |
| `OFFICE_DIRECT_BIN` | `office-direct-extract` | Office 直读 console-script |
| `OFFICE_DIRECT_TIMEOUT_MS` | 120_000 | 整体超时（soffice + 直读） |
| `OFFICE_SOFFICE_TIMEOUT_MS` | 90_000 | soffice 子进程超时（`kill_on_drop`） |
| `OFFICE_SOFFICE_MAX_CONCURRENT` | 1 | worker 级 soffice 并发信号量 |
| `OFFICE_SOFFICE_BIN` | `soffice` | 二进制→OOXML 转换 |

---

*文档 v2 写于 2026-08-02；吸收 code review 修订。实测产物在 `/tmp/pdf_spike/`（`*_liteparse.md` / `*_markitdown.md` / `direct_*.md` / `office_direct_extract.py` / `binary_test/`）。*
