# 仓库树先行原型交付记录

日期：2026-09-10。用户在「Windows 原生优先、在线 embedding、执行轻量方案」之后要求先做原型。本记录覆盖该先行交付，不代表完整 [PRD v0.3](2026-09-10-repository-tree-prd-v0.3.md) 或 [W0–W10 计划](2026-09-10-repository-tree-development-plan.md) 验收完成。

## 入口

- 工程：`C:\Users\xingc\Documents\Codex\repository-tree`，独立于本仓库运行，说明在工程 `README.md`。
- 本机页面：http://127.0.0.1:8765 。停机后在独立工程目录用 PowerShell 7 运行 `./scripts/start-prototype.ps1`；停止用 `./scripts/stop-prototype.ps1`。
- 首次安装：`./scripts/bootstrap-windows.ps1 -IncludeFormats`。本机已完成安装及在线配置复用，无需重复安装。
- MCP：`http://127.0.0.1:8765/mcp`，bearer token 保存在 `.demo/local-token`，不进入版本库。

## 实际实现

1. 浏览器导入 MD/TXT/文本 PDF/DOCX，在 `.demo/documents/` 建立原文目录；当前尚未绑定已有用户项目目录。上传不会移动用户原位置文件。
2. 名称与前 2000 字命中目录关键词规则，冲突进入待处理，无命中按格式归类；展示原因，支持编辑后续归档规则。
3. 保存标题路径、来源定位和 chunk 序列，支持前后块及章节序列展开。示例用「前三项已完整表达，第四项仍在下一块」验证续读。
4. 目录内关键词子串与精确余弦检索，RRF 合并。在线 embedding 使用 SiliconFlow `Pro/BAAI/bge-m3`；原文、向量和索引保存在本地，生成向量时向在线服务发送相关文本。
5. HTTP 和 MCP 共用领域服务，提供 status/browse/search/read；外部 Agent 可按 `document_id/seq/next` 读取正文。

## 技术和验证

运行时是原生 Python 3.12.14、FastAPI、SQLite、NumPy、官方 MCP 2.2.0；页面为 HTML/CSS/JavaScript。Markdown 使用 markdown-it-py，PDF 使用 LiteParse 2.14.4，DOCX 使用 firecrawl-anydoc 0.2.4。LanceDB 0.38.0 和 sqlite-vec 0.1.9 只做候选引擎探针；未安装 Docling 或本地 embedding 模型。

- 自动检查：37 项全量通过（5.60 秒）；随后新增浏览器上传/原件下载闭环，原型组 10 项通过（2.12 秒），当前共 38 个已验证用例。
- 真实 MCP HTTP：认证、工具发现、关键词检索、读取成功；不等于实际 Agent 宿主或 LLM 质量验收。
- 在线接口：3 条合成文本、1 次请求、1024 维、344 ms；浏览器自然语言查询一次 484 ms。均为单次观测，不构成 SLO。
- 浏览器实测：示例 3 份文档、6 个块向量就绪；自然语言查询首条命中第四项；前后块可连续阅读；规则保存成功。
- 证据在独立工程 `evidence/prototype-validation.md`、`evidence/online-probe.json` 及忽略提交的原始 JUnit 中。

## 未完成与后续

用户当前收敛到 [128 题纯 RAG 评测](2026-09-10-repository-tree-rag128.md)，指定 Qwen3.8 Flash、不限 token 预算。独立工程已新增 XLSX 导入、文档集合范围、标题树、连续块读取、配套 Skill 和独立 Agent/评分入口；本轮 53 项全量本地测试通过，10 份原件解析为 872 块。在线批次已启动，以上首轮浏览器和在线探针证据保持原始含义。

正式 V0 尚未关闭。已有目录绑定、监听与重建、快照发布/游标、崩溃恢复/撤销、时间条件归档、语义聚类/SVD、真实语料质量和规模性能均未验收。当前切块是标题/段落结构＋320 字符上限；不是已验证的 embedding 语义边界算法。

DOCX 只标注转换后的 Markdown 行，PDF 标题为解析推断；OCR 和混合扫描文档完整性不在原型验证范围。容量限制为 100 文件、单文件 8 MB/2000 块，不能据此宣称达到正式 S/M 档性能。用户试用后按反馈收敛交互，再推进正式路线图。
