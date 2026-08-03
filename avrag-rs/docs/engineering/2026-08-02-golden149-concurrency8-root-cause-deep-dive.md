# 全量 149 并发 8 · 根因深探（2026-08-02）

> 接力《…-full-behavior-diagnosis.md》（全量画像）与《…-nonpass-diagnosis.md》（12 题归因）。本文是对四个重点问题的**机制级根因**调查，每条结论都验证到检索平面/语料/代码/提示词原文。
> 时间线更正：跑数实际窗口是 **22:36–22:51**（行为产物 mtime），`v2_20260802-143621` 目录名来自 14:36 的一次先前尝试被复用；所有 prompt 改动（最晚 21:59）均早于跑数，当前 `prompts/` 工作树即跑数时状态。

---

## 根因一：baiyao_pdf recall 0.727 —— 不是检索问题，是 ingest 文本被 markitdown 换血

**结论**：q091/q099 的 gold chunk「检不到」是因为 **gold substring 在检索平面里根本不存在**。2026-07-28/29 的 `markitdown_reingest`（`crates/app/tests/product_e2e/llm_real/markitdown_reingest.rs`）把评测 workspace 的检索平面 `rag_text_chunks`（dense 向量 + BM25 + doc_grep 三通道共读的一张表）整体替换为 markitdown 对原始 PDF 的提取产物。markitdown 把 PDF 数字信息图拆行、给 CJK-latin 之间插空格：

- q091 gold `11大主题域分组`/`100大主题域`/`638个业务对象` → 提取产物里数字（11/100/638）与标签（「大主题域分组（L1）」等）被拆进**不同表格行**，连续 substring 不存在；
- q099 gold `引入4A架构` → 产物里是 `引入4 A 架构`（插空格）；`以客户价值为中心` → fixture 原件本身就是 `以客户价值为中\n心`（pdftotext 断行，gold 标注时没做空白归一，这条即使 ingest 完美也匹配不上）。

**证据链**：22 条 gold 逐一验证——存在的 17 条 ↔ recall=1.0 的 8 题，不存在的 5 条 ↔ recall=0 的 q091/q099，完美相关。fixture 原件里这些 gold 连续存在（md5 全同的 7 份拷贝都在），业务表 `chunks`（pdftotext 纯文本）也在——**只有检索平面被换成了 markitdown 版**。信息其实完整检索到了（q091 模型手工把 L2 计数加总 10+9+5+…=100、L3 加总成 638 交叉验证后答对，q099 也 PASS），评分器（`golden_set.rs:212` ChunkMatch::Substring，大小写归一但**不做空白归一**）记 0。

**定性**：指标假阴性 + ingest 文本损坏的叠加。根子在 ingest/markitdown 提取层与评测匹配层，**检索通道（embedding/lexical/grep）和查询构造无罪**。q096 的 gold `A级：300W≤项目投入<500W` 在检索平面完好存在（chunk 38 单元格内），它 recall=0 纯因零检索（根因三）。

**修复方向**：
- 评测层（短平快）：substring 匹配加空白/管道符归一化，或 baiyao gold 改用 Keywords 类型；gold 标注应基于检索平面实际文本而非 fixture 原件。
- ingest 层（根治）：处理信息图式「数字-标签分离」版式；评估 markitdown 对中文 PDF 的 CJK-latin 插空格是否可关。

---

## 根因二：表格计数口径 —— 模型行为层，教学已到边际

**结论**：不是上下文可见性问题，不是提示词缺失——**模型看到了全部行、算出了行数、写进了答案，然后主动选择按名去重值当头条**。

- **可见性**：q078 cited_context 含 81/81 行概念阶段记录，q088 含 59/59 + 30/30 行（306–312 七行同名行全在）。judge 被单次 `truncated=true` 标记误导称「无法确认 81」，实际 7 次 grep 的 union 覆盖了全表。
- **提示词**：跑时 SKILL.md L123 gotcha「按品名去重后改成更小的数」、L144「不要按列去重」、L92「某一列值重复不改变行数含义」、how-to-read-tables.md 误读对照 B「列重复当去重计数」——**逐字覆盖了本次失败形态，且已教过两轮（08-01 total_hits 载体 + 08-02 上午 S1 波），模型照样去重**。
- **行为（最硬发现）**：q078 答「**57 个不同的活动**（按"活动"列去重统计）」随后「若按…逐行统计，共 **81 行**」；q088 答「45 个活动（占 59 行）」「24 个活动（占 30 行）」并自述「同一个活动为不同角色各占一行，行数多于活动个数」。复算验证 81→57、59→45、30→24 与模型数字完全一致——两个数字都算对了，是**把计量对象从「行」换成「名称去重」的问题解释选择**。对照 q079（同跑同族 PASS）用「每个表格行 = 一个活动」口径 + 与文件名「370 activities」交叉验证；q106 的答案里甚至也出现了 81/86/92/59/30 这组 gold 数字（但总数答了 348——裸 pattern grep 漏了生命周期 22 行，q079 恰恰多跑了那一管）。

**两个真实缺口**：
- A. **教学框架错位**：所有教学都框在「total_hits 读法」，没有一句直接处理「问『有多少个 X』时计量对象 = 表行」的问题解释层规则；现有例子（品名/货位）没迁移到活动/角色形态。
- B. **确定路径是死信**：SKILL 规定表内计数走 struct 两段式（struct_query COUNT「一锤定音」），实际全跑 **struct_query 调用 = 0/1447**（struct_catalog 53 次全是一次性摸范围），计数全压在 doc_grep（611 次，42%）上。

**修复方向**（按有效性）：
1. `prompts/capabilities/knowledge-base/reference/how-to-read-tables.md` 误读对照节加一条与失败同构的条目（「问有多少个 X 且一名多行时，计数对象 = 行；按名称列合并 = 更换计量对象」），并把 strategies.md 的「品名」gotcha 泛化为「名称列」。
2. 把 q079 成功形态固化为教学样例：管道对齐 grep 逐阶段取 total_hits + 与文档自称总数交叉验证。
3. 再加一条「勿去重」标语的边际收益已证伪（两轮复发），别再做。
4. 附带：judge 对 grep 标记形态（单次 truncated vs 全体、裸数字格 vs 明示）的读法也有误判（q078/q106），judge prompt 可补。

---

## 根因三：三个行为异常，三种不同机制

### q096 零检索直答 —— 桥接调用未完成 + 参数化记忆编造，架构策略边界内无闸可拦

模型第一轮写了含 `client.dense/lexical/grep` 的代码块（宿主预扫描 `preview_codegen_client_calls` 据此各发 1 次 `retrieval_started`，`iteration_codegen.rs:636-653`），但这些调用**从未以 Ok 完成**（零 ToolResult、零 sandbox_error、零 retrieval_finished）。随后模型凭参数化记忆编造整段答案——硬证据：答案里的文档全名、《4.6.2.1节》、「战略型/重要型/基础型」分级体系在语料里**全部不存在**（grep 零命中；docscope 清单默认不注入，模型上下文没有任何标题来源，是从查询串向外编的），还模仿知识库合同格式伪造了 `SELECTED: #3, #4`（与 contract.md:28 示例高度相似）。终答形态合规 → S2 四检测器不命中；「零证据直答」按 AGENTS.md stop-decision 策略宿主**故意不拦**（07-31 起 synthesis gate 不再路由 DegradedNoEvidence）→ DirectAnswer 被收下，伪造的 SELECTED 解析落空只 warn。caps=['rag'] 挂载正常、合同也教了「模型自行编写的假执行结果不是证据」（contract.md:21）——这是**教导-遵守缺口**，且按现行策略（语义接地归 skill/模型，host 不设闸）A1 类**无闸可拦**。08-01 的修复全打在形态层（治 A2/B 类，本跑已根除），q096 是语义层的 A1 残留，不是修复遗漏，是策略边界。

**要修只有两条路**：接受策略边界（靠 skill 文案继续压概率）；或破例加一条结构化宿主规则（如「本轮零 Ok ToolResult 且 caps 含 rag 时不收 DirectAnswer」——注意这与 AGENTS.md「no-chunk refuse DirectAnswer 禁止」直接冲突，需先改策略）。

### q050 放弃式空答 —— 不是模型放弃，是宿主降级文案替换

答案那句「未能生成符合引用格式要求的完整答案」来自 `prompts/loop/contract-violation-rag.md`（07-31 就存在），经 `prompt_assets.rs:194` 加载。真实链条：模型正常检索（recall=1.0，证据池含 RETAIN chunk）→ 收尾合成轮终答**整个是代码块** → 触发 S2 `code_only` 闸（`final_answer_rules.rs:101-108`）→ 宿主发 repair nudge → 模型**再次交代码块** → 宿主按「never surface a raw code block as the final prose answer」（synthesis.rs:296-298）替换为降级文案。**与 21:58 改动的 synthesis/contract-internal-*.md 无因果**（那是 JSON 合成路径，q050 走散文路径）。

定性：08-01 B 类（代码块即终答）的残留被新闸接住——闸成功阻止了代码块泄漏给用户（08-01 是 3 题直接漏出），但 repair 失败后把一道证据齐全的好题变成空答。**改进点不是闸本身，是 repair 失败后的 salvage**：当前只给一次修复机会；可考虑 repair nudge 文案诊断化（指出「检测到终答是纯代码块」的现状描述）或允许第二次 repair，而不是直接降级。

### q123 计算器不一致 —— 教导在线后的行为抖动

gold 5107.6 = 4520×1.13（4520+587.6）；模型 4607.6 = 4520+87.6，**恰好差 500**——心算在 ×0.13 步把 587.6 错位成 87.6。08-01 §2-D 的「无提示词教导」已由 d76b72b2 修复（`prompts/system/agent-base.md:30` 明教「算术用它得到确定数值，不在正文里心算」），且同跑 5 道计算题 4 道走了 calculator（q122/q147/q148/q149 全 PASS）。q123 是唯一没走的，也是唯一题面无「计算」动词、用全角括号、带小数乘数的题——题面形式是合理协变量，但 n=1 无法与纯随机抖动区分。提示词教导已到边际，钉死需要确定性手段（G-17 是评测闸非产品闸；产品侧若加结构门同样碰策略边界）。

---

## 根因四：doc_summary Error×5 / web_fetch Error×2 —— 调用方式问题 + 可观测性缺口

- `doc_summary`（`rag-core/src/runtime/tools/doc_summary.rs:7-84`）：已排除「文档无 summary」（空结果返回 Ok+空数组，`repository_retrieval.rs:280-308`）。剩两个候选：(a) 模型把 doc_summary 当 native tool_call 发明 → `native_tools_closed` 拒绝（`tool_registry.rs:126-134`，该工具 native 面已随 SaC 关闭）；(b) codegen 里传 `level="section"` → 设计性拒绝（`:22-27`）。q061/q115 中 `doc_profile Ok` 与 `doc_summary Error` 相邻的形态兼容 (b)；同题大量桥路径 Ok 夹 1 次 Error 兼容 (a)。两个候选都有错误消息引导模型改道，**非数据缺失、非存储故障，P3**。
- `web_fetch`×2（q118）：最可能 CSDN 类目标站对 bot UA 返 403 或 30s 超时（`web_fetch.rs:166-178`，文档化已知局限），web_search 快照已兜底，**环境性质，P3**。
- **真正值得修的是 P1 可观测性**：`qNNN.json` 的 tool_trace 只投影 `{tool,status}`（`rag_quality_prod.rs:1195-1200`），`ToolResult.data.error` 被丢弃；服务端 tracing 也没落到运行日志。7 次 Error 的原始消息事后不可恢复——harness 持久化 `data.error`（一行级改动）即可让这类调查不再停在「两个候选根因」。

---

## 总览：12 题非 PASS + 2 题隐性缺口的最终归因矩阵

| 题 | 归因层 | 机制 | 修复层 |
|---|---|---|---|
| q078/q088（+q106 半） | 模型行为 | 看到行数仍选去重口径；教学两轮未拦住 | prompt 问题解释层条目（有效性存疑，边际） |
| q091/q099 | ingest + 评测 | markitdown 拆行/插空格毁 substring；评分无空白归一 | **评测匹配归一化（短平快）/ ingest 版式修复（根治）** |
| q096 | 模型行为 + 策略边界 | 桥接调用未完成→凭记忆编造整套答案；host 按策略不拦 | 策略决策：接受 or 破例加结构闸 |
| q050 | 宿主 salvage 缺陷 | S2 闸接住 code_only 终答，repair 失败→降级空答 | repair 轮改进（诊断化 nudge / 二次修复） |
| q123 | 模型行为抖动 | 教导在线，5 题中唯一跳工具心算错 | 提示词已到边际，需确定性手段 |
| q106（另半） | 真检索缺口 | 裸 pattern grep 漏生命周期 22 行，370 未进上下文 | q079 成功形态固化 |
| q053/q132 | 枚举完整性 | 漏列方法/字段 | P1 prompt（nonpass 诊断已列） |
| q068/q083 | 多源口径不收敛 | 并陈不选唯一 | P1 prompt |
| q105/q139 | 过度引申/组织颠倒 | 内容对，结构错 | P1 prompt |
| doc_summary/web_fetch Error | 调用方式/环境 | native 发明或 level=section 误用；目标站反爬 | P3 + **P1 harness 记 data.error** |

**优先级重排（根因证据后）**：
1. **P0 评测层**：substring 匹配空白归一化 + gold 标注对齐检索平面（一修解掉 baiyao_pdf 成建制假阴性，让 recall 指标重新可信）；
2. **P0 ingest 层**：markitdown 对中文 PDF 的拆行/插空格评估与修复；
3. **P1 harness**：tool_trace 持久化 `data.error`；
4. **P1 宿主**：q050 类 salvage 改进（repair 诊断化/二次修复）；
5. **P1 prompt**：how-to-read-tables 加问题解释层条目 + q079 形态固化（认清边际收益，这是最后一轮 prompt 尝试）；
6. **P2 策略决策**：q096/q123 类是否值得破例加宿主结构闸（需改 AGENTS.md stop-decision 策略，先讨论再动）。
