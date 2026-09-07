# 调研：收件箱 + 建议归类 + 确认式归档最佳实践

**日期：** 2026-09-06 · **性质：** 时间点调研快照 · **服务对象：** context-helper PRD（`docs/plans/2026-09-06-directory-plugin-prd.md`）F6 与交互设计

**背景：** 产品只监视用户指定投放点（下载目录、Agent 工作区）。新文件落地 → 建议归入某个已绑定项目目录（附理由）→ 确认后才移动。全自动是罕用选项且仅限已绑定项目根内部。一次误归档 = 信任归零，设计按最高保守标准。用户日常主阵地在桌面 chat agent 里，几乎不进自有 UI。

## 1. 组织方法论给出的规则

- **PARA**（Tiago Forte）：按**可行动性**而非主题排序——Projects（有 deadline）> Areas > Resources > Archives，外加 Inbox。目的地问题：「这个文件服务于哪个有截止期的项目？」([thomasjfrank.com](https://thomasjfrank.com/productivity/books/the-para-method-summary-and-book-notes/))
- **Johnny Decimal**：最多 10 个 area × 10 个 category、不超两层、每项有编号。暗示：建议目标集应是**小而固定的有限集合**，映射不上就该留在 inbox。([官方论坛](https://forum.johnnydecimal.com/t/applying-jd-system-to-email-organization/1721))
- **GTD**：inbox 只做捕获，「这是什么？可行动吗？」是独立的 clarify 决策步——方法论本身反对跳过人做判断。
- 证据侧：无公开证据表明规则引擎能稳定匹配这些体系；复杂框架「tend to get abandoned」，能坚持下来的是「少量顶层文件夹 + 搜索」。([1dot.ai 排名](https://1dot.ai/blog/mac-file-organization-systems-ranked))

## 2. 规则型自动归档（Hazel / DropIt / File Juggler）

- 用户实际用的规则：扩展名、名称模式、日期、来源 app、**正文关键词**（发票号、「合同」等词），特殊规则优先于兜底规则。([少数派 DropIt 教程](https://sspai.com/flipboard/post/45532)、[MacSales Hazel](https://eshop.macsales.com/blog/86195-app-star-of-the-week-hazel-for-mac-is-file-automation-for-the-rest-of-us/))
- **纯规则崩在哪**：内容匹配极脆——「同一家银行的账单，这张读得到账号、下一张读不到」，OCR 质量不一 + 半年后无人能调试的规则漂移。([Sortio 对比页](https://www.getsortio.com/compare/hazel-alternative-mac))

## 3. AI 归档产品的置信度与信任策略

- **paperless-ngx**：每个 tag 可选 Any/Exact/Regex/Fuzzy/**Auto**（ML 分类器，在用户已确认的归档上训练）；官方推荐**兜底 = 打 inbox 标签留人工审**；社区反馈「存得好、分得差」，不少人直接关掉分类器。([elest.io](https://blog.elest.io/paperless-ngx-on-elestio-automating-document-ingestion-ocr-processing-and-smart-tagging/)、[mattgoodrich](https://mattgoodrich.com/posts/local-ai-on-paperless-document-pipeline/))
- **SaneBox**：自动移动 + 拖拽纠错训练 + **每日 digest 批量复核**；1–6 周训练期；误分重要邮件是核心信任成本。([PCMag](https://uk.pcmag.com/software/2115/sanebox))
- **Spark**：Smart Inbox 官方承认「AI sorting makes mistakes」，要求用户定期复核。([Spark glossary](https://sparkmailapp.com/glossary/auto-sorting-in-email))
- **Sortio**（AI 原生）：**apply 前预览 + 备份**；「AI Rule Builder 把信任的重复 AI 分拣固化为确定性规则」。
- 反面信号：OneDrive/Copilot **刻意不做**自动整理。([thedrive.ai](https://thedrive.ai/blog/onedrive-file-organization-problem))
- HITL 通用模式：**双阈值**（高置信直通、低置信转人工）与**三通道**（straight-through / review / manual exception）。([arXiv 2601.05974](https://arxiv.org/pdf/2601.05974)、[Parseur](https://parseur.com/blog/hitl-best-practices))

## 4. 用户主阵地在 chat/agent 时的确认 UX

- **MCP elicitation**（2025-06-18 规范）：server 可向用户发结构化问询，accept/decline/cancel 三态。([规范解读](https://forgecode.dev/blog/mcp-spec-updates/))
- 客户端支持：**VS Code** 2025-07 起（[den.dev](https://den.dev/blog/vscode-mcp-elicitations-stop-guessing/)）；**Cursor** v1.5 完整支持，但 Windows + HTTP transport 有挂起 bug（[Cursor 论坛](https://forum.cursor.com/t/mcp-elicitation-create-hangs-agent-on-windows-in-cursor-3-10-20-but-works-on-macos/165391)）；**Claude Code** 官方 hooks 文档已含 Elicitation/ElicitationResult 事件。([Claude Code hooks](https://code.claude.com/docs/en/hooks))
- OS 通知：Windows toast 最多 5 个操作按钮 + 输入框，适合「接受/换一个/忽略」轻量确认；但通知是瞬态的，必须有批量复核队列兜底。([Microsoft Learn](https://learn.microsoft.com/en-us/uwp/schemas/tiles/toastschema/element-action))

## 5. 从确认中学习（write-back）

成熟模式三种：SaneBox 的**拖拽即训练**（隐式信号）、paperless-ngx 的**在用户确认过的归档上重训分类器**、Sortio 的**把被信任的重复 AI 决策提升为显式规则**。Gmail「filter messages like this」是人工触发的规则生成。

## 对 inbox+confirm 设计的 8 条建议

1. **三档置信策略，默认不移动**：高置信也仅在已绑定项目根内、且用户显式开启全自动时才直通；中置信 → 建议+理由；低置信 → 静默留在 inbox，不发通知打扰。
2. **批量复核队列是家，通知只是入口**：digest 式批量确认（SaneBox Daily Digest 验证形态）；单条 toast 带「接受/换目标/忽略」做轻量即时确认，过期自动进队列。
3. **Agent 内确认走 MCP elicitation，但不依赖它**：三态正好是「建议移动到 X，理由 Y」的 schema；Cursor Windows bug 说明需要本地 fallback 队列。
4. **建议目标集小而固定**：只对已绑定项目根（≤10，JD 上限）做建议；映射不上 = 留 inbox，绝不新建/深钻目录。
5. **规则先行，AI 殿后**：确定性规则（扩展名/名称/来源）命中可自动执行（用户自己写的规则，信任已存在）；AI 建议必须确认。「正文关键词」规则标注为易碎。
6. **确认即训练 + N 次后提议固化**：accept/reject 写回建议模型；同一模式被接受 N 次后，提议「固化为规则」由用户一键确认。
7. **永远可撤销 + 建议附理由与预览**：误归档的代价 ≫ 未归档。
8. **明示训练期**：onboarding 说明「前几周需要纠错，越纠越准」，管理预期。
