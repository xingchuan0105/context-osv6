# Prompts / 记忆 / 文档档案 优化方案（2026-06-10）

> 本文汇总三块优化：① `prompts/` 提示词改进；② 影响"齐备性"的缺口与悬空引用；③ 记忆机制改造；④ 文档档案（章节索引 + 元数据）能力。
> 对照基准：Perplexity《Designing, Refining, and Maintaining Agent Skills》。
> 文末附核实证据（关键文件:行）。带 ☑ 的设计取舍已与负责人确认；带 ⚠️ 的为待决子项。

---

## 0. 背景与机制现状（一句话版）

- 提示词体系：目录扫描发现（`build.rs`）+ frontmatter + 三层渐进披露（index / load / runtime）+ 依赖图防环。机制本身与 Perplexity 模型高度契合。
- 注入路由：三种混合——强制注入（`codegen`、各 answer skill）、planner/metadata 提示（`writing`/`format`）、模型显式 `{"skill_request":[...]}`（`memory`/`search`）。
- 真正靠 description 路由的只有 `memory`、`search`；其余靠强制/planner。

---

## 1. Prompts 优先级改进清单（已认）

| 优先级 | 动作 | 依据 |
|---|---|---|
| P0 | 把 `memory`、`search` 的 description 改成"Load when…"触发器（吸收其 body 里的"何时加载"判据）；其余簇 body 删除/精简 `## 何时加载` 段 | description 是索引层路由触发器，但当前写成能力标签，触发判据却埋在加载后才可见的 body 里（时序错位） |
| P0 | 删除 cluster body 中复述 orchestrator 的句子（如 `codegen` body 重申"RAG R0 强制注入 / Chat·Search 不可用"，与 `rag-system.md` 重复） | Perplexity："不要在 skill 里复述 system prompt"，避免 action at a distance |
| P1 | `codegen` 的超短 reference（`retrieval-strategy.md` 17 词、`doc-summary.md` 10 词）合并回 SKILL.md body，取消原子拆分 | atomic 簇会一次性加载全部 reference，超短文件只增加目录指引成本，无渐进披露收益 |
| P1 | 精简 `synthesis/rag-answer.md`（1820 词）中模型已知的通识段（如 "Language" 段：同语言回答、CJK 保留英文术语） | "if it's easy to explain, the model already knows it. Delete it." |
| P2 | `search` 簇（67 词、无 reference）充实项目特定规则（垂直选择细则、可信度标注规范、fetch 时机），否则降级并入 orchestrator | "every skill is a tax"：当前多数句子模型本就会做 |
| P2 | 统一 frontmatter 字段名（cluster 用 `applicable_modes`，orchestrator 用 `applicable_strategies`，同义异名）与语言风格（cluster 中文 / synthesis 英文） | 一致性，降低跨文件审阅成本 |
| 保持 | 继续 append `## 禁止` / `Common Mistakes` / `Red Flags`（gotchas 飞轮） | Perplexity："gotchas 是最高价值内容" |

> 注：description 触发器写得最规范（"Load when…"+负例）的 `rag-answer`/`grounded-answer` 恰是**强制注入**、不经路由的；而真正靠 `skill_request` 路由的 `memory`/`search` 反而是能力标签。P0 即修正此错位。

---

## 2. 影响"齐备性"的缺口与悬空引用（已认）

### 2.1 悬空引用（chat.md）— ☑ 决定：直接删除
- `synthesis/chat.md` 引用 `reference/voice-and-behavior.md`，该文件不存在（chat 为扁平文件，无 reference 目录）。
- `synthesis/chat.md` 指向 `code-generation`、`data-analysis`、`academic-writing` 作为"专用 answer agent"，但 registry 中无这些 skill（`academic` 只是 `writing` 的一个 reference slug）。
- **动作**：删除上述悬空引用文本（不补建对应文件/skill）。

### 2.2 空壳/偏薄文件
- `writing/reference/tone.md`（10 词）、`codegen/reference/doc-summary.md`（10 词）、`codegen/reference/retrieval-strategy.md`（17 词）、`memory/reference/anaphora.md`（23 词）。
- **动作**：`tone.md` 充实或并入 SKILL；`codegen` 两个并回 body（见 P1）；`anaphora.md` 充实指代消解规则与边界。

### 2.3 codegen 提示词与 bridge 不一致（核实修正）
- 沙箱注入的 `client` 是 `crates/rag-core/src/runtime/bridge.rs`，其 `supported_method_names()` = `dense_search / lexical_search / graph_search / chunk_fetch / doc_summary`。
- 这 5 个与 `codegen/SKILL.md` 一致；`doc_summary(level="doc"|"section")` 也被 bridge 支持（章节索引经 `level="section"` 可达）。**先前"方法名错配"判断作废**（此前误对 `python/avrag_sdk/client.py`，那是另一个 HTTP/benchmark 客户端，非沙箱 bridge）。
- ☑ **决定：从 codegen 提示词删除 `client.rerank(...)`**。rerank 不是检索方式，而是 **dense 召回管道内的服务端自动阶段**（`retrieval.rs` 的 `multimodal_rerank_stage` / `rerank_item_chunks`，由 planner 的 `rerank_budget` 驱动，召回后自动执行）。它不应作为可调用方法暴露给模型；bridge 不暴露它是正确的。

### 2.4 文档元数据从 codegen 不可达（核实结论，关联第 4 节）
- bridge **无 `doc_metadata` / `doc_index` 方法**，所以"文档元数据（名称/领域/年代…）"目前 RAG agent 取不到；仅 `doc_summary(level="section")` 能拿到章节索引。
- 详见第 4 节文档档案方案。

---

## 3. 记忆机制改造

### 现状（2026-06-14 更新）
读回与写入路径（L2 已移除）：
- L1 原始轮次：PG `messages`（`prior_turns` + 保底 prior user 注入）。
- ~~L2 会话摘要~~：**已删除**（`chat_sessions.summary`、`update_session_summary`；migrations `0043`–`0045`）。
- L3 用户画像（dream 层）：`user-profile-extraction.system.md`，≤1 次/24h，输入为最近 **12 轮**原文（`build_recent_turns_context`，`service_postprocess.rs`）。
- 读回：L3 → `AgentRequest.user_preferences`；更早历史 → `conversation_history_load`（PG 近序 + jieba FTS，notebook scope）。
- 指代消解：`resolved_query` 一等列 + `user_turn_metadata`（ADR-0008）。

### 目标设计（已认）

1. **删除 L2 session-summary** —— ☑
   - 移除 `maybe_update_session_summary` / `build_session_summary` 调用与 `session-summary.system.md` 引用（`chat_private.rs`、`service_postprocess.rs`；并清理 `guardrails/.../prompt_leak.rs:32` 的 include）。

2. **保留 L3 画像层，输入改接最近 12 轮原文** —— ☑
   - `maybe_update_structured_profile` 使用 `build_recent_turns_context`（12 轮窗口），不依赖 L2。

3. **近 3 轮保底注入 + 按需 skill 调取** —— ☑
   - 默认无条件注入最近 **3 轮**（保底下限，保障指代消解与连续性）。
   - 新增"记忆调取"能力（skill / bridge 方法），模型可**按需**调取：
     - (a) 超出保底 3 轮的**更早历史轮次**；
     - (b) **用户长期画像/偏好**。
   - ☑ scope = 两者都要（both）。
   - 配套：修正 `memory` 簇语义——明确"近轮由 runtime 注入、更早历史与画像由 skill 调取"，纠正"记忆=指代消解"的窄化描述。

4. **指代消解写回：非破坏性** —— ☑（nondestructive）
   - **保留用户原话**（展示/审计用）；将 `resolved_query` 从 metadata 提升为 PG **一等列**；下游检索/上下文默认读 `resolved_query`（缺省回退原话）。
   - **明确不做破坏性覆盖**：消解可能出错（解析 prompt 自带"不要发明上文不存在的实体"约束），覆盖原话会导致错误不可恢复并污染后续每一轮（误差滚雪球）。

### 依赖与顺序
- 必须**先**完成 L3 输入改接（第 2 项），**再**删除 L2（第 1 项），否则画像层断粮。

---

## 4. 文档档案（章节索引 + 文档元数据）

### 场景
用户问"这本书谁写的 / 哪年的 / 什么领域""先给我目录""第三章讲什么"——属于**关于文档本身/结构**的问题。当前 codegen 只会语义检索正文，此类问题答非所问或答不出。

### 数据现状（核实）
| 字段 | 现状 | 来源 |
|---|---|---|
| 章节索引（章→chunk） | ✅ 已生成入库；可经 `doc_summary(level="section")` 取到 | `toc_entries`(migration 0031)、`section_index.rs`、worker `maybe_enrich_toc_with_llm`（仅 toc 稀疏或 `INGESTION_LLM_SECTION_INDEX=1` 时触发 LLM 生成） |
| 名称 name | ✅ | `SummaryMetadata.docname` / `doc_metadata` |
| 领域 domain | ✅（**粗粒度枚举**） | `docscope.rs` |
| 年代 era | ⚠️（**粗粒度时代枚举**，如"当代/古代"，非精确年份） | `docscope.rs` |
| 体裁 genre / 语言 language | ✅ | `SummaryMetadata` |
| **作者 author** | ❌ 数据模型中**完全没有** | — |
| **发表时间 date** | ❌ 无精确日期字段（仅粗粒度 era + 文件上传时间） | — |

### 目标设计（已认）

1. **新增抽取 + 存储 作者 / 发表时间** —— ☑（add_both）
   - ingestion 阶段（扩展 summary/metadata 抽取 prompt）让 LLM 额外抽 `author`、`publication_date`；
   - 新增 DB 字段并迁移存储。
   - 这是档案能力的**前置门槛**：不补数据，codegen 再聪明也无米下锅。

2. **合并为单一"文档档案" bridge 方法** —— ☑（merge）
   - 在 `bridge.rs` 新增一个方法（如 `doc_profile`），一次返回：名称 / 作者 / 发表时间 / 年代 / 领域 / 体裁 / 语言 / 章节目录（字段可选）。
   - 内部复用现有 `toc_entries` + summary 元数据 + 新增 author/date 字段拼装。
   - 降低模型决策难度（符合 Perplexity"降低选择难度"原则）。

3. **codegen 提示词加"档案 vs 正文"分流指引** —— 
   - 规则：问"文档是什么/结构长啥样" → 读档案（`doc_profile`）；问"文档里讲了什么具体内容" → 检索正文（`dense_search` 等）。
   - 配 1~2 个负例（如"第三章讲什么"应先取档案目录，再按 chunk 取正文）。

4. **废弃 doc_index 工具** —— ☑（deprecate）
   - 章节索引已由 `doc_summary(level="section")` / 新 `doc_profile` 覆盖；`doc_index`（Rust 侧存在、bridge/SDK 无入口）属孤儿，删除。

---

## 5. 建议落地顺序

1. **记忆**：① L3 输入改接最近 12 轮 → ② 删 L2 session-summary → ③ `resolved_query` 升一等列（非破坏性）→ ④ 新增"记忆调取"skill（更早历史 + 画像）+ 修正 `memory` 簇语义。
2. **文档档案数据**：⑤ ingestion 抽 `author`/`publication_date` + DB 迁移。
3. **文档档案接口**：⑥ bridge 新增 `doc_profile` 方法 → ⑦ codegen 提示词加分流指引 → ⑧ 废弃 `doc_index`。
4. **Prompts 清单**：按第 1 节 P0→P2 推进；⑨ 从 codegen 提示词删除 `client.rerank`（见 2.3）。
5. **缺口清理**：⑩ 删 chat.md 悬空引用；充实/合并空壳文件。

---

## 6. 待决项

（已无未决项：`rerank` 已定为从 codegen 提示词删除；L3 画像输入窗口已定为 12 轮；保底注入 **2** 轮 prior user。）

---

## 附录：核实证据（关键文件:行）

- 提示词扫描/注册：`crates/app/build.rs`、`crates/app/src/agents/progressive/prompt_registry.rs`
- 索引渲染 / body 渲染：`crates/app/src/agents/loop/disclosure_plan.rs:238-315`（index 用 description；body 不含 description）
- mode 配置回填 description：`crates/app/src/agents/loop/config.rs:183 hydrate_clusters`
- 沙箱 bridge 方法名：`crates/rag-core/src/runtime/bridge.rs:46-176`（dense_search/lexical_search/graph_search/chunk_fetch/doc_summary；无 rerank/doc_metadata/doc_index）
- 章节索引：生成 `crates/llm/src/section_index.rs`、worker `bins/worker/src/main.rs:3159`；存储 `migrations/0031_document_toc.*`；读取 `crates/rag-core/src/runtime/tools/doc_summary.rs:77`、`doc_metadata.rs:46-48`、`doc_index.rs:52`
- 文档元数据字段：`crates/rag-core/src/runtime/tools/doc_metadata.rs:69-142`（name/mime_type/file_size/status/chunk_count/toc）；`crates/common/src/docscope.rs:213-236`（SummaryMetadata / DocScopeProfile：language/domain/genre/era）
- summary map-reduce：`crates/llm/src/summary.rs:101-164, 352-449`；worker 调用 `bins/worker/src/main.rs:1838`
- 记忆写：`crates/app/src/lib_impl/chat_private.rs:44/76/209`、`crates/app/src/chat/service_postprocess.rs:113/181`
- 记忆读：`crates/app/src/agents/runtime.rs:178`、`crates/app/src/rag_prompts.rs:200`
- 指代消解：`crates/app/src/agents/loop/query_normalize.rs:74-92, 188-268`
