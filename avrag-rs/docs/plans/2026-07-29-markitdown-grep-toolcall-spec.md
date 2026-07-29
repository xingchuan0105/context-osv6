# markitdown 产出物实证 + grep toolcall / 语法校验器设计说明

| 项目 | 内容 |
|---|---|
| 日期 | 2026-07-29 |
| 状态 | 设计说明（用户指定方向：markitdown 统一解析 + grep 化检索 + 格式校验） |
| 实证基础 | `/tmp/markitdown_out/`（6 篇语料原件解析产物）+ markitdown 0.x 源码 |

## 1. 为什么走这条路（背景）

nightly 144/149 的五题非 PASS 诊断（q077/q078/q105/q115/q139）引出三条用户判断：
① LLM 面对多种 chunk 形态自然写不对检索代码——需要**统一格式**；
② 检索工具不该只有 doc_scan（全量装载），需要 **coding-agent 式 grep**——关键词、行号、命中计数、上下文；
③ LLM 检索代码里的格式符号若不符合产出标准，**校验器要给内容提醒**。

## 2. markitdown 源码机制（converter 行号）

| converter | 机制 | 源码 |
|---|---|---|
| PlainText | **透传**（仅 charset 解码），不解析任何结构 | `_plain_text_converter.py:60-71` |
| Xlsx/Xls | `pd.read_excel` → `DataFrame.to_html(index=False)` → markdownify | `_xlsx_converter.py:83-95` |
| Docx | mammoth（docx→HTML）→ markdownify；表格→管道表 | `_docx_converter.py:58-80` |
| PDF | **pdfplumber 逐页空间词簇表格检测**（自适应列容差：间隙 70 分位、clamp[25,50]；列对齐≥2 判定表行；表行占比<20% 整页放弃）→ 管道表；非表页 pdfminer 散文兜底 | `_pdf_converter.py:120-340, 520-560` |

结论修正：markitdown 的 PDF 不是 pdfminer 扁平化——自研无边框表格空间重建，白药 PDF 的三阶段日期得以**同行相邻**保留（q115 的关键空间关系）。

## 3. 产出物实证（语料原件）

| 文件 | 产出 | 关键观察 |
|---|---|---|
| IPD **txt** | TSV 原样（透传） | 统一性的边界：txt 无结构可出 |
| IPD **xlsx** | 管道表，370 数据行一行一行 | ✅ `grep -c "\| 概念阶段 \|" = 81`（golden 真值）；⚠️ pandas 方言：首行被吃掉当列名（真表头降为数据行）、`Unnamed: N` 空列、单元格换行→字面 `\n`（反而利好行寻址）、**空格填充管道**（`\|概念阶段\|` 0 命中，必须 `\| 概念阶段 \|`） |
| 白药 **PDF** | 网格状管道行 + 散文 | ✅ 第 51 行：三阶段日期**同一管道行相邻**；第 53 行紧跟「按4A架构详细设计」——q115 所需邻接关系完整；现行 txt 把第二阶段甩到 56 行外 |
| consulting docx ×3 | markdown + 管道表 | mammoth/markdownify 标准形态 |
| thesis docx | markdown | 同上 |

**方言清单（校验器的 ground truth）**：
- 管道行通用形：`| 单元格 | 单元格 |`（**空格填充**，xlsx 单空格、PDF 列宽对齐多空格）；
- xlsx 特有：`## <sheet名>`、首行标题被 pandas 消费、`Unnamed: N`、单元格内字面 `\n`；
- PDF 特有：对齐空单元格 `|     |`、列宽 ljust 填充；
- txt：无格式（TSV 原文）。

统一性结论：灌**原格式**（xlsx/pdf/docx）经 markitdown → 全量 markdown-ish；语料现行"灌导出 txt"才是格式碎片之源。剩余 ~10% 方言由校验器教管。

## 4. grep toolcall 设计（coding-agent 语义）

新原生工具 `doc_grep`（rag-core，doc_scan 同款注册/桥接）+ 沙箱 `client.grep(...)`：

```python
hits = await client.grep(pattern, doc_ids=None, regex=False,
                         context=0, max_hits=50)
# → [{"doc": "...", "line": 51, "text": "...", "before": [...], "after": [...]}]
#    响应头：{"total_hits": 370, "returned": 50, "truncated": true}
lines = await client.read_lines(doc_id, start, end)   # 行号区间原文
```

- **文档级虚拟行视图**：服务端按 doc 拼接 chunk 文本（doc_scan 同一 list_text_chunks 路径），行号稳定可寻址；
- **命中计数即统计**：`grep("| 概念阶段 |")` → `total_hits: 81`——Rust 数的，零 LLM 解析代码（q078 的机制解）；
- **截断即完备性声明**：`truncated: true` 时模型明确知道自己拿的是样本（"LLM 不知道自己不知道"的机制回答）；
- regex 默认关闭（子串），开启走 Rust regex crate（已有依赖）；
- 上限：max_hits≤200、行宽截断 500 字符、doc 行数上限对齐 doc_scan 16384 段。

## 5. 语法校验器设计

两层，均确定性：

**静态（执行前）**：host 侧扫 LLM 代码块字符串字面量中的格式符号，对照目标文档的方言符号清单：
- 出现 key=value 过滤形（`阶段=…`）而文档为管道方言 → 提醒：「格式符号 `阶段=` 不在标准中；本文档表格为 `| 阶段 |` 空格填充管道行，过滤串应为 `| 概念阶段 |`」；
- 出现无空格管道形 `|概念阶段|` → 提醒空格填充方言。
实现：正则扫字面量（`\w+=`、`\|[^ |]`）+ 文档方言清单（灌库时随 chunk metadata 落 `format_dialect` 字段）。

**动态（执行后）**：代码块 0 命中（stdout 空列表/0 results）→ observation 追加同款方言提醒。0 命中是系统可确定检测的"你不知道你不知道"。

## 6. 灌库策略（换血式，用户指定）

- markitdown 解析语料**原件**（xlsx/pdf/docx/md）→ 通用文本切分（512 tok，不重检测管道表、不走 TableIr 表格臂——本实验搁置 TableIr）→ 同 embedding（qwen3.7, dim 1024）；
- **只替换 `rag_text_chunks` 中对应 doc 的行**（文本+dense 向量同表），`chunk_type='summary'/'profile'` 行、`document_toc`、triplet 图谱一律不动；doc_id 不变 → 同 workspace 检索同时可见新向量与旧 summary/index/triplet；
- 生产含义：eval-only 实验，生产 storage/ 不动。

## 7. 验证

- 五题定向：q077/q078/q105/q115/q139（前置已做：Answer 归因军规 + q105 rubric 分档）；
- 对比上轮：PARTIAL 0.8 / UNGROUNDED 0 / PARTIAL 0.8 / PARTIAL 0.7 / PARTIAL 0.7；
- grep/校验器本批只出说明不实现（产出物驱动下一轮实现）。
