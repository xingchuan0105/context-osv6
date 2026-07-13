# Agent 工作过程披露（四模式 WorkFact）设计

| 字段 | 值 |
|------|-----|
| 日期 | 2026-07-13 |
| 状态 | **APPROVED 决策已锁定 · P0 实施中** |
| 范围 | Chat / RAG / Search / Write 共用 Progress 层 |

---

## 1. 目标

用户不要「所有 query 同一句硬编码过程」，而要 **本轮可证实的工作进展**（检索式、产品动作名、命中情况等），且：

- **不暴露** codegen / 沙箱 / 内部 tool id（如 `dense_retrieval`）
- **不**在答案区展示工具卡
- 四模式 **同一套机制**，分模式 **Fact Adapter**

---

## 2. 产品决策锁定（2026-07-13）

| # | 决策 | 取值 |
|---|------|------|
| 1 | L1 粒度 | **A**：每个真实检索/工具动作一步（RAG：每个 `client.*`；Search：每个 `web_search` 等） |
| 2 | Chat | **B**：有内部动作（如 memory）时出 **产品文案** 步骤，不暴露 skill 名 |
| 3 | 0 命中 | **A**：显示「未找到相关内容」+ 检索式 |
| 4 | 默认展开 | **A**：RAG / Search / Write **进行中默认展开**；完成后折叠摘要 |
| 5 | Reasoning P0 | **A**：有 `ReasoningSummaryDelta` / 消毒摘要才显示，**折叠** |
| 6 | Write 文案 | **A**：中文产品句 |
| 7 | 第一刀范围 | **C**：四模式 adapter **最小可用** |
| Search chips | domain | **不展示** domain chips |
| counts | 数字 | **不始终展示**；仅成功且有意义时（如 hits>0） |
| 合成流式 | | **真流式/等价体验**：JSON 契约校验后对 prose **分段 MessageDelta**（P0）；LLM 生成期流式属后续可选 |
| 工程默认 | 事件形状 | 扩展现有 SSE `activity`（phase/title/detail/counts/sources_preview），不新开 event 名 |

---

## 3. 本仓库真实执行体（设计约束）

```text
Chat   → ReAct, tool_pool=[], prose_only 流式, memory skill
RAG    → ReAct, tool_pool=[], mandatory codegen skill
         client.* → RuntimeBridge → dense/lexical/graph_retrieval…
         合成 internal_answer_v1（complete_json，非整段 LLM token 流）
Search → ReAct, tool_pool=[web_search, web_fetch, …]
         合成 internal_search_answer_v1
Write  → 非 ReAct 产品 execute：run_write_mode 阶段管线 + refine
```

- **无独立 planner**；「子查询」= LLM 写入 `client.*(query=…)` / `web_search` args 的字符串。  
- **codegen = 管道**，禁止进 Progress 产品面。  
- 今日成功 codegen 多发 `ToolResult{code_gen}` → SSE **Trace**（前端忽略）→ 用户看不到检索过程。

---

## 4. 统一机制：WorkFact + Adapter

### 4.1 WorkFact（observed-only）

| 字段 | 说明 |
|------|------|
| `phase` | `accept` \| `act` \| `reason` \| `compose` \| `done` |
| `kind` | 见 §4.2 |
| `product_action` | 中文产品名（非内部 id） |
| `query_text` | 本步真实检索式/任务短语（可空） |
| `status` | `started` \| `succeeded` \| `failed` |
| `hits` | 可选；仅 >0 时进入 counts |
| `previews` | 文档短名等；**Search 不放 domain** |

`evidence` 恒为 observed：只能从真实 seam 产生，禁止编造。

### 4.2 kind → 产品名

| kind | 产品名 | 模式 |
|------|--------|------|
| `understand` | 理解问题 | 全 |
| `retrieve_semantic` | 语义检索 | RAG |
| `retrieve_keyword` | 关键词检索 | RAG |
| `retrieve_graph` | 关系检索 | RAG |
| `retrieve_doc` | 阅读文档 / 文档结构 / 通读片段 | RAG doc_* |
| `search_web` | 网页搜索 | Search |
| `fetch_url` | 读取网页 | Search |
| `memory` | 回忆相关上下文 | Chat（产品句，非 skill 名） |
| `write_research` | 收集写作素材 | Write |
| `write_outline` | 规划文章大纲 | Write |
| `write_draft` | 起草正文 | Write |
| `write_refine` | 润色修订 | Write |
| `write_validate` | 校验文稿 | Write |
| `compose_answer` | 整理回答 | 全 |
| `reason_preview` | 思考摘要 | 有摘要时 |

**禁止出现在 Progress**：`codegen`、`code_gen`、`dense_retrieval`、`sandbox`、`skill_request`、raw UUID 列表。

### 4.3 映射到 SSE `activity`

| WorkFact | ChatEvent::Activity |
|----------|---------------------|
| phase | `phase` = `{phase}:{kind}`，如 `act:retrieve_semantic` |
| title | **稳定 i18n key**：`progress.retrieve_semantic.running` 等（**非**已本地化中文） |
| detail | **原始** query / 参数片段（无语言包装引号） |
| hits>0 | `counts.hits` |
| previews | `sources_preview`（Search 空） |

**Locale 切换（方案 B，2026-07-13）：**

- 后端 **不**按语言出产品名；前端 `localizeProgressActivity(locale, event)` 用 `formatUiMessage` 映射 `progress.*`。  
- UI 语言（设置 → 外观/locale）变更后，**新** activity 按新 locale 显示；种子步骤本身已是前端双语。

### 4.4 Adapters（采集 seam）

| Adapter | Seam | 发出 |
|---------|------|------|
| **RAG** | `RuntimeBridge::call` 记录 method+query+result；`iteration_codegen` 成功后 emit | 每 `client.*` 一步 |
| **Search** | `dispatch_native_tool_calls` 在 web_search/web_fetch 前后 | 每 native call 一步 |
| **Chat** | 开局 understand；memory 等内部动作完成时产品句 | 瘦时间线 |
| **Write** | `writer/mod.rs` 阶段点 | 中文 write_* |

L0（前端或后端开局）：≤300ms，`understand` + user query 截断 40 字；有第一条 L1 后可并存或降权。

### 4.5 Reasoning（P0）

- 仅当存在 `ReasoningSummaryDelta`（或等价消毒摘要）  
- 前端折叠展示，截断约 80–120 字  
- 不进答案气泡  

### 4.6 合成「真流式」（P0 定义）

RAG/Search 契约为 JSON，**生成期**难以边 gen 边出合规 prose。P0 定义：

1. `complete_json` 校验通过 → `render_synthesis_prose`  
2. 对 prose **按字符块多次 `MessageDelta`**（与 `emit_buffered_agent_answer_if_needed` 同量级，如 24 字）  
3. 前端 typewriter 继续润色观感  

P1（可选）：`prose_only` 路径已 `complete_stream`；若未来 RAG 改为可流式契约再真 delta。

---

## 5. 前端

| 项 | 行为 |
|----|------|
| ProgressTimeline | 消费 `activity`；展示 title/detail；counts 仅非空展示 |
| 默认展开 | rag/search/write 进行中 expand；done 后 collapse |
| 工具卡 | 四模式 **不渲染** ToolResultsPanel |
| Search | **无** domain chips |
| Reasoning | 折叠区，有数据才显示 |

---

## 6. 非目标（P0）

- 展示 codegen / 沙箱代码  
- 伪造 planner subquery  
- 答案区 tool dump  
- 为 Chat 伪造检索步骤  
- 独立 planner 服务  

---

## 7. 实施切片

| 切片 | 内容 |
|------|------|
| D0 | 本文档 |
| D1 | `agent-loop` progress 模块 + 扩展 `AgentEvent::Activity` + SseSink |
| D2 | RAG bridge 捕获 + codegen 后 emit；Search native emit |
| D3 | Chat/Write 中文 Progress |
| D4 | JSON 合成 prose 分块 MessageDelta |
| D5 | 前端 counts/detail/展开/reasoning 对齐 |
| D6 | 编译与针对性测试 |

---

## 8. 验收句

1. 两个不同 RAG query → 进度 detail 中检索式 **可见不同**。  
2. 进度文案出现「语义检索/关键词检索」，**不出现** `dense_retrieval` / `code_gen`。  
3. hits=0 → 有「未找到」+ 检索式。  
4. Search → 无 domain chips。  
5. 答案区无工具卡。  
6. Write 进度为中文阶段。  
7. 合成答案以多个 token 事件到达（或前端可感知分段），非仅 done 一包（在 JSON 路径下为分块 delta）。

---

*决策确认：用户 2026-07-13「1A 2B 3A 4A 5A 6A 7C + Search 无 domain + 数字不始终 + 真流式合成 + 文档后开工」。*
