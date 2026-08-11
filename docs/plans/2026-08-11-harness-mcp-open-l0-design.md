# L0：检索 harness MCP 化 × 摄入契约开放 —— 设计草案

**状态：** 草案（讨论中，未动工）
**日期：** 2026-08-11（同日三轮修订：同一路径两种驱动、题卡×SaC 合并、web 归 agent 侧、闸分层、平台计费定锤、旧 loop hospice、§10 L2 波及清单）
**后续：** 实现载体定为 osv7 Go 重写 —— 见 `2026-08-11-osv7-go-rewrite-design.md`；本文档保持为契约层权威。
**触发：** 2026-08-11 生产事故（session `da167808-5069-4e25-9525-93e64be490ef`）+ 架构方向讨论

## 0. 背景与触发

2026-08-11 事故（dual rag+search，7 轮语料检索未调 web，2m8s，verify 判 fail 的草稿被 ceiling 路径直送）暴露的不是单个 bug，而是自编排主 agent 的结构性成本：漂移控制、交接坍缩、交付路径护栏，全都要自己养。

产品核心目标是**多种检索 harness，让任何 agent 具备检索质量**；产品形态是开放的（用户自带 Claude Code / Codex / pi 等 agent，在自己的界面里完成检索）。结论：智能从产品中拆出去，契约留在产品中。

**封闭性拆账（讨论结论）：**

- 现状对外唯一检索门是 `workspace.rag_query` / `search_query` / `chat`（整个自研 loop 黑盒）——每次外部调用烧产品侧 chat LLM（事故单 102k prompt tokens），经济上必须严防死守，事实上封闭；且对 agent 返回预制终答而非证据，是劣质工具。
- ingestion 的 LLM 依赖只在**后段富化**（doc summary / KG 抽取 / 图像 caption)；解析器全确定性（liteparse_v2 / anydoc / markitdown / PaddleOCR)。富化智能可委托给用户 agent。
- 检索原语本身只需 embedding（每调用几十 token)，无 chat LLM。原语级开放的边际成本比现状低约三个数量级 —— **开放首先是 COGS 问题，其次才是架构问题**。

## 1. 目标 / 非目标

**目标：**

- 检索 harness 原语 MCP 化：外部 agent 以原语组合完成检索，智能由调用方 agent 承担。
- **一条路径，两种驱动**：主agent（产品界面）与外接 agent 走同一条 harness 路径；主agent 只是产品托管的调用方，不是特权方。
- 摄入契约开放：用户 agent 作为「制备生产者」提交结构化摄入包，免去单独配置 LLM endpoint。
- 产品侧只保留必须服务端锚定的能力：平台模式免 key、按量扣余额；BYOK 可选。
- 能力缺失第一时间以结构化错误反馈给 agent，无静默卡死。

**非目标（本阶段）：**

- 不动现有 `workspace.rag_query` 等黑盒端点（退役决策在 L2，由 full-149 A/B 数据驱动）。
- 主agent 切换为 pi 是既定方向，但切换动作在 L1 A/B 数据成立之后（L2)；本阶段不动现有 loop。
- 不做第二套检索/摄入协议；复用现有 transport 原则：MCP 为 thin transport。

## 2. 设计原则

1. **智能谁用谁付。** 产品不承担调用方的推理成本；产品侧只保留必须服务端锚定的能力。
2. **反馈与契约分层。** 能被模型自由裁量的部分给第三人称观察反馈（SKILL.md / 工具回传观察）；不允许失败的不变量进工具契约（schema、校验、preflight)。事故教训：纯反馈在边缘 case 必然失效（web 未调用、漂移未停止、verify 判决被无视）。
3. **一个 IR，两个生产者。** 摄入与检索都只有一条下游管线；服务端解析器与 agent 制备包收敛到同一版本化契约。
4. **无静默卡死。** 能力缺失在最早可判定点以结构化、agent 可行动的错误返回；异步任务必须在有界时间内到达终态。
5. **一条路径，两种驱动。** 同一套原语、同一份 capability 提示词、同一张题卡契约。差别仅在题卡来源与校验强度（§4)：产品显式选择 = 卡与 UI 选项强一致（硬卡）；外接智能路由 = agent 自填卡，卡填什么验什么。
6. **闸只能站在流量必经之处。** 校验集 = 卡声明 ∩ 信道可观测。web 检索等 agent 侧行为不经过 harness，服务端物理上不可验 —— 能力归 agent，责任也归 agent（§4 闸的分层）。

## 3. 能力分层矩阵

| 能力 | 谁提供 | 配置要求 | 缺失时行为 |
|------|--------|----------|-----------|
| 智能（检索推理 / 合成 / 摄入富化 summary·KG) | **调用方 agent**（用户订阅） | 无 | 不适用 |
| embedding | 服务端锚定模型（索引一致性） | **平台模式免 key，按量扣余额**；BYOK 可选（配 key 走自己额度、不扣余额） | 无余额且无 BYOK → preflight 拒绝（§6) |
| rerank | 同上 | 同上 | 同上 |
| **OCR(PaddleOCR，扫描件/重版式 PDF)** | 服务端锚定能力 | 与 embedding/rerank 同待遇（平台免 key 扣余额 / BYOK 可选） | preflight / 首阶段快速失败（§6) |
| web 检索 | **调用方 agent**（主agent = pi + websearch 插件，走 deepseek 原生检索；外接 = 各自自带） | 产品不出 web 能力；主agent 的 web 成本进 deepseek 账单 | 不进 harness 契约；卡上 web 字段为声明性元数据（§4) |
| 存储 / 索引 / 契约校验 / preflight | 产品 | 无 | — |

**余额即防滥用地板。** 平台计费模式下，钱包余额是唯一的滥用边界 —— 没余额就没服务；这比「强制 BYOK 当地板」更简单，且保留零配置体验。BYOK 永远可选，配了 key 的能力走用户自己的额度。

## 4. 检索腿（harness 原语 MCP）

**工具清单**（薄封装现有 SaC bridge：`avrag_rag_core::runtime::bridge`):

`dense` / `lexical` / `grep` / `struct_catalog` / `struct_query` / `doc_summary`（+ 可选 `verify_draft`）。

**web 不在 harness 内。** 原 `web` / `fetch` 原语从清单移除：web 检索是调用方 agent 的自身能力，harness 只担保语料侧。dual（corpus+web）由驱动方 agent 自行编排，不再是 harness 的编排责任。

**题卡（query card）—— SaC 必填输入：**

- 题卡从「模型自报的旁路元数据」升级为契约：**无卡不检索**。粒度 = **任务级一张卡**：一次检索任务填一张，多轮原语调用共享；任务转向时换卡。
- capability 由此从「前端挂载状态」变成两份可移植资产：**提示词**（contract/SKILL，外接 agent 经 MCP prompts/resources 渐进发现）+ **题卡字段**（启用声明：scope、required_action、web 意图、证据要求）。
- **校验双模式**（同一校验器，不同对齐基准）：
  - 显式选择（产品 UI）：卡与选项强一致，不一致硬拒（实现上卡可由选项直接派生）。
  - 智能路由（外接）：无用户选项可对齐 —— **卡填什么验什么**：声明 scope 验 scope 存在；声明 required_action 验 Ok 返回（现有 `required_action_missing_continue` 的泛化）。

**闸的分层（怎么卡 agent 的行为）：**

1. **资源闸（服务端，硬，两模式相同）**：卡的是资源，不是行为 —— 卡合法性、scope 存在、能力齐备（§6)、quota / 余额。非法即结构化报错。
2. **契约闸（服务端，硬，两模式相同）**：声明 ∩ 观测的一致性 —— 卡声明的 harness 可见行为必须有对应 Ok 回传（q121 与 2026-08-11 事故的双向镜像失败证明纯裁量不可靠）。反馈信道：主agent = loop observation 注入；外接 = tool result 结构化错误（§6 错误形状）。**履行有无 = 结构事实，可硬；履行质量 = 语义判断，只软**（第三人称观察，不 veto —— 与 prompts 语音规则同源）。
3. **行为闸（agent 侧，两模式各管各的）**：web 调用、推理节奏、终答质量不经过 harness，服务端物理上卡不到。
   - 主agent：运行时是产品自己的 —— pi 插件栈（websearch + harness MCP client + **card-keeper**）看得见 websearch 调用，对照卡声明做声明-观测校验（与契约闸同语义，位置在 agent 侧）；未履行向 pi loop 注入第三人称 observation，结构违规可硬卡 deliver。逃生口 = **改卡**：显式撤销声明即可通过，改卡进 telemetry —— 闸不强迫 web，只强迫卡说实话。
   - 外接agent：行为闸不存在也不该存在。卡上 web 字段降级为声明性元数据（telemetry /「agent 声明使用了 web」展示），校验器跳过。
   - 回执式自报（attestation，终答附「已查 web，来源 N 条」）：可记录不可证实，只值一个 telemetry 字段，不值一个闸。

**产品担保边界（写进契约）：** 语料检索质量 + 反馈诚实；外接模式的端到端答案质量 = 用户自己 agent 的能力上限。这是开放架构的诚实定价。

**已排除项：** harness 保留可选 web 原语（产品 key 计费）—— 与「利用 agent 自身能力」矛盾且新增 COGS；若未来要担保外接 web，唯一路径是让 web 流量经过产品。

**契约要点：**

- **证据句柄协议**：alias / `SELECTED: #n` / `KEEP` 沿用现有线协议；检索结果以句柄为引用单位；`verify_draft` 服务端校验引用-句柄对应关系（仅语料侧；web 来源主张服务端不可观测，主agent 模式由 card-keeper 做声明-观测）。
- **观察反馈**：工具回传附第三人称观察（复用 `prompts/loop/*` 文案资产）。新增 tag 先在 `host_markers.rs` 注册。
- **计量**：per-call usage（复用 usage observer / cost_events）；embedding 按调用计费，平台模式扣余额。

## 5. 摄入腿（摄入契约 MCP）

**契约**:`DocumentIr`(`crates/ingestion/src/ir.rs`)+ doc summary + KG 三元组，版本化、经 typeshare 公开分发（`contracts/` + `typeshare.toml`)。

**工具**:`ingest_begin`(声明文档元数据，返回 doc_id + schema 版本 + 能力 preflight 结果，见 §6)→ `ingest_blocks` / `ingest_summary` / `ingest_kg` → `ingest_commit`（触发校验 → embed → 索引）。

**服务端硬校验**（校验失败返回具体缺口清单 —— 缺口清单本身即 agent 反馈，反馈与契约在此闭环）:

- schema 形状、summary 存在性与长度区间、KG 三元组形状；
- **文本覆盖度启发**：抽取文本量 vs 声明页数（防只读前两页）;
- **`normalized_name` 服务端归一化**：不同 agent 实体命名不一致，不收口则混合来源 workspace 的 graph_augment 静默退化；
- agent 提交内容按不可信输入处理（接现有 guard 管线），计量走 ingestion_tasks / wallet。

**反馈层**：发布 ingestion SKILL.md（block 切分约定、summary 写法、实体规范化约定、preflight 先行）——指导层；校验为强制层。

**两个生产者收敛**：服务端解析管线重构为同一契约的一个生产者（零配置默认路径）；agent 制备为另一生产者。下游唯一：校验 → embed → 索引。

**适用边界**：agent 制备最适合文本原生格式（md/docx/html/code/小 PDF)；扫描件/重版式走服务端 OCR 路径（BYOK 或平台计费，见 §3/§6)。

## 6. 能力 preflight 与错误契约（无静默卡死）

**能力发现**:`account.capabilities`（或 `ingest_begin` 响应内嵌）返回能力表：

```json
{ "embedding": "hosted|byok|missing", "rerank": "...", "ocr": "..." }
```

agent 在承诺用户之前先查能力表（ingestion SKILL.md 教此动作）。`hosted` = 平台计费可用（需余额）；`byok` = 已配自有 key。

**第一时间反馈的三个判定点（按最早优先）:**

1. **静态判定（同步）**：能力未配置（如无 BYOK 且余额不足）→ `ingest_begin` / `create_upload` 同步拒绝，不进入队列。
2. **内容嗅探（同步）**:PDF 是否扫描件提交时才能知道 —— `ingest_begin` 对上传内容做轻量嗅探（前几页文本层检测）；需 OCR 而未配置 → 同步拒绝。
3. **管线漏网（有界异步）**：仍有个案进入 worker → 首个依赖缺失能力的阶段**立即终结**，`document_status` 置 failed 并带结构化 reason；任何异步任务必须在有界时间内到达 `done | failed(reason)` 终态 —— **queued 悬挂是 bug 类**。

**错误形状**(agent 可行动，第三人称事实陈述）:

```json
{
  "error": "capability_missing",
  "capability": "ocr",
  "fact": "该文档为扫描件 PDF，文本层为空；本 workspace 未配置 OCR 能力",
  "remediation": "通过 <配置入口> 配置 OCR BYOK；或将文档转换为文本原生格式后重新摄入",
  "doc_id": "…"
}
```

平台计费模式下余额不足与能力缺失同等待遇：`balance_insufficient` 在判定点 1 同步返回，`remediation` 指向充值入口（`/pricing#topup`)。

原则：错误文案写给 agent 读 —— 说明发生了什么、缺什么、有哪些可行动作；不写命令式祈使句（与 prompts 语音规则一致）。

## 7. 迁移路径

- **L0（本文档）**：检索原语 + 摄入契约两套 MCP 工具集落地；现有黑盒端点不动。独立有产品价值。
- **L1**：主agent pi 形态 = pi + websearch 插件（deepseek 原生检索）+ harness MCP client + **card-keeper** 插件；复用 `prompts/capabilities` skill 文案，跑 full-149 A/B 对比现有 loop。门槛：pass 率 ≥ 基线（109/149 PASS）且 token 成本显著低于均值（47.8k)。验收含**出站薄闸**（用户气泡只见 pi 自然语言终答，协议残片 / tool transcript 不入泡）。先一天 spike 验证 pi 的 MCP client 与扩展机制（card-keeper 落点）。
- **L2**：数据成立 → 产品内主 agent 切换，自研 loop **删除**（no backward compat tax)；黑盒端点退役或改为薄封装。永不两套 loop 并存。

## 8. 旧 loop hospice 与事故修复清单

**决定（2026-08-11 三轮）：旧 loop 进入 hospice，全部修复项冻结。** P0（ceiling 直送 fail 稿）/ P1（dual 强制 web）/ P1b（合成前任务重锚）/ P2b（无进展检测）针对的机制（verify / ceiling / 合成交接）随 L2 整体删除，在新架构中没有对应物 —— 再投入是打磨将删代码。对应能力由新架构原生承接：P1 → card-keeper 声明-观测闸；P1b / P2b → pi 原生上下文管理 + card-keeper 事卡重锚。

**唯一必须带进新架构的护栏本能**：用户主气泡只见 pi 的自然语言终答 —— pi 输出与用户气泡之间的薄出站闸，写入 L1/L2 验收（否则 P0 故障类借尸还魂）。

**回退条款**：若 L1 spike 或 A/B 失败导致旧 loop 延期服役，P0 重新评估 —— 它是用户可见的最差面失败（协议腔直送主气泡），修复成本约一小时不变。

## 9. 开放问题

已决：embedding / rerank / OCR 计费 = 平台模式免 key 扣余额，BYOK 可选（§3）；OCR 是否保留强制 BYOK 待最后一确认。

- `verify_draft` 对外部 agent 是可选工具；主agent（pi）是否在其 skill 中写为必选？
- 摄入契约版本演进策略（schema 变更如何兼容已入库文档的 IR)。
- dashboard 进度面板在 L2 后如何从 MCP 调用事件重建（现有 bridge→activity 映射可复用）。
- card-keeper 的硬度边界（结构违规硬卡 deliver 是否可行）依赖 pi 扩展机制，L1 spike 验证。

## 10. L2 波及清单（pi 置换主 agent）

判据：**离开 LLM 推理还存在的功能保留（harness 价值本体）；不存在的被 pi 置换。** 按 187k 行 Rust 现状估算，删除/坍缩约 55–60k 行（≈1/3）。

**A. 被置换删除（L2）：**

- `agent-loop`（27.1k 行，最大单块）：react_loop 三循环、停止决策、结构闸、budget 执行、host observation / host_markers / prompt_assets。
- `llm` 的 chat 部分（12k 行大头）：provider 抽象 / 流式 / 重试 / 模型路由。残余 = 摄入富化瘦客户端（零配置路径仍由服务端做 summary/KG）。
- `app-chat`（10.1k 行大头）+ `agent-tools` loop 面（7.7k 行）：ToolCatalog / dispatch 删除；检索执行实现保留并改为 MCP 薄封装。
- `code-interpreter`（1.4k 行）：pi 自带执行环境。
- `prompts/loop/*`（约 60 个 nudge/tmpl）与 `prompts/clusters` 大头；有价值的第三人称文案迁移为 MCP 回传 / card-keeper 反馈文案。
- `modes/*.yaml` 的 loop 语义（forbid_retrieve_direct_answer / verify 开关 / budget）；产品模式退化为 pi 会话 profile（模型 + MCP 工具集 + 卡预设）。
- 题卡提示词（模型自报路径）：卡成为 MCP schema 必填字段。

**B. 保留（产品价值本体）：** `rag-core` / `rag-core-ports` / `retrieval-data-plane`（检索执行 + SaC bridge）、`storage-pg` / `pgvector` / `milvus`、`ingestion`（+IR 契约）、`billing` / `app-billing`、`guardrails`、`telemetry`、`search` / `share`、auth / RLS / workspace 作用域（T7/T8）、证据句柄协议、`prompts/capabilities`（改经 MCP prompts 暴露）、frontend_next 大部分页面。

**C. 变形：**

- 会话持久化：chat_* 表 ↔ pi transcript 模型对齐（真源选择 + UI 投影）—— 最大隐藏决策，spike 项。
- `app` 的 ConversationApp + `execute` / `execute_stream` → pi 会话管理器（拉起/附着、provider 配置、流中继、出站闸）；`transport-http` 的 chat SSE 端点跟随；`app-bootstrap` 组合根瘦身。
- `heavytail`（7.3k 行）/ `write-core`（2.1k 行）：确定性部分（切片/调度/存储/编辑原语）保留为原语，LLM 驱动部分交 agent。
- `e2e-analyzer` / full-149 runner：改打 pi+MCP。
- BYOK 设置：chat provider key 从后端 client 配置变为 pi 会话注入；embedding/rerank/OCR BYOK 维持后端。
- capability 挂载 UI → 显式选择（卡预设）。
- 规则文档层：AGENTS.md 三循环/停止决策段、prompts README、e2e-gates 随 L2 重写；voice 规则与 prompts-in-md 保留（适用于 MCP 反馈文案）。

**D. 新增：** MCP server（L0）、pi 会话管理器、card-keeper + websearch 插件、LLM 计量网关（pi 用量 → 扣余额）、出站薄闸、pi transcript → 产品会话投影。

**最易低估：** ①pi transcript 与产品会话模型的真源选择；②pi 进程模型与单机多用户并发（每会话进程？池化？资源上限）；③dashboard 活动面板从 MCP 事件重建的粒度（不够则面板降级）。三者均进 L1 spike 验证清单。
