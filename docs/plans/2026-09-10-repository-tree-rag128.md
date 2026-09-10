# 仓库树原型：128 题纯 RAG 评测

日期：2026-09-10。用户当前范围为「只做到 128 纯 rag 测试」，指定 Qwen3.8 Flash、不限制 token 预算。独立工程已补齐评测入口，离线验证通过；在线批次 `qwen38-first-pass` 已启动。

## 验收对象

独立工程 `C:\Users\xingc\Documents\Codex\repository-tree`，Windows 原生 Python，在线 embedding。被测对象为独立 Agent 与仓库树检索阅读的完整链路，采用本仓库的原题和 Eval v2 核心评分规则。

不经过原生 Lead/Workers 或产品对话 API，不能把结果写成 Context-OS 原生 E2E 通过或纯检索替换消融。149 中 web、dual 和 chat 共 21 题不纳入；不为本次评测引入大型项目依赖、跨文档聚类或 SVD。

## 已完成

1. 固定 `golden_set_realistic.json` 的 128 道 `capabilities == ["rag"]` 原题号，保留文档 scope、prior turns、拒答与评分约定；参考答案和 gold 片段仅对裁判可见。
2. 实际原生 full-149 runner 指定的 10 份 MD/DOCX/XLSX/PDF 原件离线导入完成，共 872 块。原件留在主仓库，评测副本、题库、答案均位于独立工程忽略提交的 `.eval/`。
3. AnyDoc 扩展至 XLSX，检索/浏览/标题树/阅读统一施加文档范围；一次读取 1–32 个连续块，引用带稳定块 ID 和来源哈希。
4. Qwen3.8 Flash 多轮工具调用、实际加载的独立 `repository-tree/SKILL.md`、原生裁判文本提取、核心标签适配、逐题产物、断点续跑和逐次消耗记账。没有整批 token 上限、max_tokens/thinking_budget 或宿主上下文字符预算。
5. Windows 后台子进程启动器先向量化，再执行 10 题技术小批，再续跑至 128 题；包含静默/总时长 watchdog 和系统失败停止条件。

## 验证证据

| 检查 | 当前结果 | 可支持的结论 |
|---|---|---|
| 独立工程全量本地测试 | 53/53 通过，6.50 秒；Skill 格式校验通过 | 本地功能与模拟在线协议通过；不是实际 LLM 效果 |
| 真实原件离线解析 | 10/10，872 块 | 当前 10 份文件无解析阻塞；未逐页人工验证完整性 |
| 历史 Eval v2 核心标签核对 | 20/20 与原生产物一致 | 历史 Layer A/judge 输入下的核心标签适配一致 |
| 本批在线向量与真实问答 | 已启动，待结果 | 尚无完整 128 题质量或性能结论 |

题库 SHA256：`191adc72fdd494f7f3e580f51896b837b253abbe7f35c03ad8bb33b76232c743`。每次运行冻结题库、原件、解析索引、代码、模型和裁判源码哈希；变化后使用新 run-id。

## 评分边界

主指标是答案正确性、忠实度、相关性、拒答行为和原生核心标签。召回保留原生逐块 substring 口径，主报本题全部已观察片段的 recall，另报 recall@15。跨块连续窗口只作补充诊断，不改变主标签、不读取未观察的来源。

适配器未复刻原生产品工具断言、计算卡修正及所有 MRR/nDCG 诊断。XLSX/DOCX 仅提供转换后 Markdown 行定位；当前 chunk 仍是标题/段落结构加 320 字符上限。结果不会证明 embedding 语义边界、语义聚类或正式目录插件已实现。

## 下一步与运行入口

在独立工程运行 `./scripts/run-rag128.ps1 -RunId qwen38-first-pass -Concurrency 2`。10 题小批验证调用、裁判和产物完整性后继续；小批有效结果直接复用，普通质量低分继续记录。128 个有效问答和裁判结果齐备后输出质量、耗时、token、失败分类和限制，不预设全题 PASS。

预计 30–90 分钟，实际以小批修正；用户明确不限制 token 预算，仍记录实际消耗与无回执请求。详细口径、参数和产物路径见独立工程 `eval/RAG128.md`，Agent/Skill/MCP 分工见 `eval/AGENT-INTERFACE.md`。

本阶段不关闭完整 [开发计划](2026-09-10-repository-tree-development-plan.md) 的 W0/V0 或后续正式波次；当前交付仍属于[先行原型](2026-09-10-repository-tree-prototype.md)。
