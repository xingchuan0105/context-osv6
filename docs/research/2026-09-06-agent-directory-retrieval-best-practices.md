# 调研：小型本地语料的分层检索最佳实践（Agent 目录层）

**日期：** 2026-09-06 · **性质：** 时间点调研快照 · **服务对象：** context-helper PRD（`docs/plans/2026-09-06-directory-plugin-prd.md`）F2/F3/F4 设计

**背景：** 桌面 Agent（Claude Code / Kimi Code / Cursor）的「目录层」插件。用户指定本地项目文件夹（markdown / PDF / 会议转写 / 代码；语料小——数百到数千文件、数十 MB）。Agent 自带 grep/glob；产品静默维护派生索引（文件名、TOC/大纲、摘要、向量），按就绪程度分层暴露 MCP 工具。不整盘监听，不做重 Wiki 编译。

## 1. Agentic 搜索 vs 向量索引：2025–2026 共识

- **Claude Code 删掉了向量管道**。Boris Cherny（HN 2025-03 + Latent Space 播客 2025-05）：早期用 RAG + Voyage embeddings + 本地向量库，很快发现 agentic search 更好——更精确、更简单、索引不会过期、无隐私问题。Cursor agent mode、Windsurf、Cline、Devin、Amp 收敛到同一模式。([vadim.blog](https://vadim.blog/claude-code-no-indexing/)、[aasherkamal.com](https://aasherkamal.com/resources/rag-vs-filesystem-ai-agents-2026))
- **实证**：Amazon Science《Keyword Search Is All You Need》(2026-02) 测得 agentic 关键词检索达到 RAG 90%+ 性能、零向量库；arXiv 2605.15184《Is Grep All You Need?》发现 grep 普遍胜过向量，且 **harness/工具输出格式的影响不亚于检索算法本身**。([tensorOwl 综述](https://tensorowl.dev/writing/keyword-retrieval-in-the-agent-era/))
- **反例（向量该上的地方）**：Augment 在 SWE-Bench 上 grep/find 够用但明确注明"repo 较小"前提；更难基准（SWE-Bench Pro，平均修复涉及 4.1 个文件）纯 grep 触顶，Augment 转向微调代码 embedding。([jxnl.co](https://jxnl.co/writing/2025/09/11/why-grep-beat-embeddings-in-our-swe-bench-agent-lessons-from-augment/)) Cursor 官方 A/B：语义搜索 +12.5% 平均答题准确率，**≥1000 文件库代码保留率 +2.6%——收益随库规模增大**；grep+语义组合最佳。([cursor.com/blog/semsearch](https://cursor.com/blog/semsearch))
- **边界判定**：精确名词/标识符/错误串密集的小语料 → 词法检索够用；转述型、同义词多、概念性提问（**会议转录正是此类**）→ embedding 值得。([tensorOwl §8](https://tensorowl.dev/writing/keyword-retrieval-in-the-agent-era/))

## 2. 小规模混合检索与 reranker

- RRF（k=60）是标准融合算子，混合一致优于单路。([hybrid reference 2026](https://www.digitalapplied.com/blog/hybrid-search-bm25-vector-reranking-reference-2026))
- Reranker：本地 cross-encoder 重排 ~300ms 延迟，对上下文精度提升显著；10³ 文档规模下候选集小、成本可忽略，**常是真正的质量杠杆**；可后期再加，非首日必需。([rsis 论文](https://rsisinternational.org/journals/ijrsi/uploads/vol13-iss5-pg360-371-202605_pdf.pdf)、[kunwar.page](https://www.kunwar.page/chapter/062-reranking-with-cross-encoders))

## 3. 切分与增量索引

- 结构感知切分一致优于定长窗口：FinanceBench 上 structure-based 全胜（[arXiv 2506.13778](https://arxiv.org/pdf/2506.13778)）；Markdown + 标题切分比朴素管线 top-1 高 22 个百分点（[MDisBetter](https://mdisbetter.com/blog/pdf-to-markdown-for-rag-pipeline-complete-guide)）；代码侧 cAST（EMNLP 2025）AST 切分 +4.3 分。
- Cursor 增量索引的 Merkle tree 只为**远端同步**定位变化；本地 tree-sitter AST 切分、正则兜底。([mache 竞品分析](https://github.com/agentic-research/mache/blob/main/docs/reference/competitive-landscape.md)) 纯本地场景 per-file content hash + mtime 即可，Merkle 是过度设计。

## 4. 廉价结构层（pre-vector）

- Aider repo map：tree-sitter 提取 def/ref 标签 → PageRank 排序 → 塞进 ~1024 token 预算，整库结构一图呈现；token 消耗约为无图方案的 1/4。已有 Rust 移植（`repo-mapper` crate）。([codegen.com](https://codegen.com/comparisons/aider-vs-claude-code/)、[libraries.io](https://libraries.io/cargo/repo-mapper)) 对非代码文档，"标题树 + 一句话摘要"的 token 预算化大纲是同构物。

## 5. MCP 工具设计最佳实践

- Anthropic《Writing effective tools for agents》：工具描述要"像给新员工讲"；命名空间前缀划界；返回高信号结构化结果（带出处）；截断/报错要可行动；**描述质量直接影响 agent 选错工具的概率**（arXiv 2602.14878 实证）。([anthropic.com/engineering/writing-tools-for-agents](https://www.anthropic.com/engineering/writing-tools-for-agents))
- 结果呈现是一等变量：给结果打 `matched_via` 标签（精确命中 vs 放宽召回）让 agent 知道可信度、决定要不要换招。([tensorOwl §7](https://tensorowl.dev/writing/keyword-retrieval-in-the-agent-era/))
- Serena/Thoughtworks Radar：agent 优先的高层抽象（先拿结构概览再深入）优于行号级原语。([Thoughtworks Radar](https://www.thoughtworks.com/en-cn/radar/techniques/code-intelligence-as-agentic-tooling))

## 对分层工具设计的 8 条建议

1. **词法层永远是默认主力且先行可用**：filename/grep 先行上线符合共识；小语料下它可能承担大部分查询。
2. **向量层按查询形状辩护，不按默认**：语料含会议转录（转述密集、概念提问多），向量层有正当性；但库小（收益随规模涨），定位为"第三层补充"而非核心。
3. **向量层内部做 BM25+向量+RRF 单库混合**（SQLite FTS5 自带 bm25() + 一个向量扩展即可，无需独立向量 DB）。
4. **Reranker 留接口、后上**：10³ 文档下本地 MiniLM 级 cross-encoder 成本 ~300ms，是性价比最高的质量升级位。
5. **切分全部走结构感知**：markdown 标题、PDF 章节、转录的说话人/时间段为天然边界，chunk 带 heading-path 元数据；杜绝定长窗口。
6. **增量索引用 per-file content hash**，不引入 Merkle tree；变化文件才重新解析/嵌入。
7. **大纲层模仿 Aider repo map**：token 预算化（1–2k tokens）的全语料"文件名 + 标题树 + 摘要"工具，让 agent 在向量就绪前就有全局定位能力。
8. **索引就绪状态写进工具描述，而不是藏工具**：agent 靠描述选工具，动态出现/消失的工具列表风险高；建议固定工具集 + 描述/返回中声明 "semantic index building, N/M files ready; outline search recommended meanwhile"，结果带 `matched_via` 与出处路径，截断信息给出下一步指引。
