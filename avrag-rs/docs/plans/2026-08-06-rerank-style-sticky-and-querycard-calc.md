# 2026-08-06 — Rerank style 粘连修复 + query-card 计算题 vs 文档数字题

状态：**已落地**（2026-08-06）— §A rerank style 粘连修复 + §D.2 P-calc-ok 评测 + §E 提示补强  
相关：E2E `CORRECT_UNGROUNDED` q30；tool_trace `Reranker call failed: Multimodal rerank API error 404`  
前置：`.env` 主 rerank = SiliconFlow `Pro/BAAI/bge-reranker-v2-m3`（`RERANK_API_STYLE=openai`）；`MM_RERANK_*` 注释 = mm 关闭。

---

## A. Rerank：为何仍报 Multimodal，以及修复方案

### A.1 现象

dense 路径 tool_trace 大量：

```text
Reranker call failed: Multimodal rerank API error 404 Not Found: Not Found
```

同时伴随 `vgrag_evidence_dropped`。用户期望主路径已是 SiliconFlow **字符串** `/rerank` + bge-m3，不应再走 multimodal / VL 协议。

### A.2 根因（配置粘连，不是模型又切回 VL）

| 层 | 实际行为 |
|---|---|
| `.env` | `RERANK_BASE_URL=siliconflow`，`RERANK_MODEL=Pro/BAAI/bge-reranker-v2-m3`，`RERANK_API_STYLE=`（空） |
| 代码默认 `AppConfig::rerank` | 仍为历史默认：`model=qwen3-vl-rerank` + **`api_style=dashscope_vl_rerank`** |
| `model_config_from_env` | `RERANK_API_STYLE` 空串被 `env_optional_string` 滤掉 → **fallback 默认 VL style** |
| `RerankerClient::rerank` | `uses_dashscope_vl_rerank()` 因 **style=DashScopeVlRerank** 为真 → 把纯文本包成 multimodal body 发出 |
| 请求 | base_url 已是 SiliconFlow，body 却是 DashScope VL 形状 → **404**；错误字符串固定为 `Multimodal rerank API error` |

关键代码：

- `app-core/src/config.rs` — `rerank` / `mm_rerank` 默认仍 VL  
- `app-core/src/config_helpers.rs` — `api_style: env.or(default.api_style)`  
- `llm/src/reranker.rs` — `uses_dashscope_vl_rerank` + text 热路径误进 `rerank_multimodal_text_query`  
- `rag-core/.../retrieval.rs` — 包装为 `Reranker call failed: …`（**text 热路径** degrade，不是「又开了 mm 模型」）

真正的 **mm 候选池路径**（`pool_has_image` + `mm_reranker`）是另一条线；`MM_RERANK` 注释后仍可能用默认 + `DASHSCOPE_API_KEY` 挂上 mm client，但 **只在有图候选时** 用。当前 149 失败里的 404 **主因是 text rerank style 粘连**。

`.env` 注释「API_STYLE 留空 → 不匹配 VL」**与实现不一致**：空 style 不会清掉默认 `dashscope_vl_rerank`。

### A.3 修复方案（建议一次做完）

#### A.3.1 立刻运维绕过（不改代码）

```bash
# avrag-rs/.env
RERANK_API_STYLE=openai
# 任意非 dashscope_vl_rerank / 非 openai_vl_rerank 的值均可，
# 只要 uses_dashscope_vl_rerank / uses_openai_vl_rerank 为 false，
# 即走 openai_rerank_once（裸字符串 documents）。
```

**不要**再依赖「留空 = 正确」。

#### A.3.2 代码修复（推荐落地）

1. **改默认 `rerank` 配置**（`AppConfig`）：
   - `api_style: None`（或显式非 VL）
   - `model` 默认改为与产品一致的 bge id，或空 model + 必须由 env 配置
2. **加载逻辑**：若 env 覆盖了 `RERANK_MODEL` 且 **不是** `qwen3-vl-rerank` / 不含 VL-reranker 名，则 **禁止** 再 fallback 默认 `dashscope_vl_rerank`（即便 `RERANK_API_STYLE` 未设）。
3. **可选显式**：支持 `RERANK_API_STYLE=` 的「清空默认」语义（需区分 unset vs empty；当前 empty=unset）。
4. **单测**：
   - 配置：`model=Pro/BAAI/bge-reranker-v2-m3` + style unset → `uses_dashscope_vl_rerank()==false`，走字符串 `/rerank`
   - 配置：style=`dashscope_vl_rerank` 或 model=`qwen3-vl-rerank` → 仍走 VL 分支
5. **文档**：同步 `.env.example` 注释；删除「留空即可」误导句。
6. **mm 默认**：评估 `mm_rerank` 在 `MM_RERANK_*` 全注释时是否仍应用 `DASHSCOPE_API_KEY` 静默启用；若产品已去 VL 主路径，默认应 **不配置** mm reranker（`make_reranker` 返回 None），仅显式 env 启用。

#### A.3.3 验证

- 单元：`llm` / `app-core` 配置与 `uses_*_vl_rerank`  
- 集成：单次 dense + rerank，日志无 `Multimodal rerank API error`，SiliconFlow 200  
- 回归：定向 E2E 若干此前 `vgrag_evidence_dropped` 高的题（如 q61/q99），对比 drop 与 PASS

#### A.3.4 明确不做

- 不在本修复里重开 Qwen3-VL-Reranker 为主路径  
- 不把 text 池强制绑 mm_reranker  

### A.5 落地记录（2026-08-06）

| 项 | 实现 |
|---|---|
| 默认 `rerank` | SiliconFlow base + `Pro/BAAI/bge-reranker-v2-m3` + `api_style=None` |
| 默认 `mm_rerank` | 空配置；`mm_rerank_config_from_env` 仅在显式 `MM_RERANK_*` 时启用（不再单靠 DASHSCOPE 静默开） |
| style 粘连 | `model_config_from_env`：非 VL model 时不继承默认 VL `api_style` |
| 单测 | `app-core` sticky/bge/mm-off；`llm` `bge_string_rerank_with_null_style_uses_openai_documents_array` |
| `.env` / `.env.example` | 注释更正；本机 `.env` 设 `RERANK_API_STYLE=openai` |

---

## B. query-card：计算题 vs 文档数字题（q30 类）— 方案讨论

### B.1 题目与轨迹（q30）

| 项 | 值 |
|---|---|
| 题 | 「按 1T/H、每天 8 小时、每年 350 天，一台年产多少吨？」 |
| 题面 | **操作数全部写在 query 里** |
| gold | `2800吨`，`source_chunks` 指向文档子串「2800吨速冻食品」 |
| mode | `rag` + capabilities rag |
| query_card（实测） | `question_type=calculation`，`required_actions=["calculator"]` |
| 轨迹 | calculator×4 Ok；**零检索**；`evidence_missing_continue` 后仍只算 |
| 答案 | 2800 正确 |
| label | **CORRECT_UNGROUNDED**（corr=1，faith=0，cited 空 / `retrieved_fallback`） |

### B.2 张力：三方各说各话

```
  题面（算术自洽）     query-card 提示文案          评测 gold / faith
  ─────────────────    ─────────────────────       ──────────────────
  数字全在 question →  「纯 calculator 仅当         要求文档 chunk 命中
                        操作数全在题面、不期望        + 答案 grounded
                        文档 grounding」→ 纯 calc
                        是「按字面正确」的
```

现行 `prompts/pipeline/query-card.system.md` **已经**写了：

> 操作数或费率若是文档事实（或题意要求对知识库核对）→ `dense`/`lexical`/`grep` **与** `calculator` 同列；  
> 纯 `calculator` 仅当数字全在题面且不期望文档 grounding。

q30 的数字 **确实全在题面**，卡填纯 calculator **符合当前提示字面**，却与 **RAG 评测 gold（文档子串）** 冲突。  
因此 q30 不是「卡完全没写 dual」，而是：

1. **题面自洽算术** vs **语料里有同一数字** 的边界未在运行时/评测上对齐；  
2. 宿主 **required_actions 闸** 只保证 calculator Ok，不要求检索；  
3. 评测 faith 把 calculator 结果当成「需 retrieval cited_context」，落 `CORRECT_UNGROUNDED`。

### B.3 方案选项（按层）

#### 方案 B1 — 只改 query-card 提示（强化 dual）

**规则：** 只要当前请求挂了 RAG 能力 / 非空 `doc_scope`，凡 `calculation` 一律 `required_actions` 含 `dense`（或 lexical）**+** `calculator`，除非题明确「不要查库、只按下列数字算」。

| 优点 | 缺点 |
|---|---|
| 改动面小（md only） | 题面已给全数字时仍强制检索 → 多一轮、费额度 |
| 与「workspace 在就优先文档」产品一致 | 纯算术闲聊式计算（capabilities 含 rag 误挂）会误检 |
| 可压住 q30 类 CORRECT_UNGROUNDED | 卡填错仍依赖模型；闸只数 Ok 不保证查到 gold |

**适用：** 产品立场是「进了 KB 模式的数字题默认要可引用」。

#### 方案 B2 — 拆题型 / 动作语义（推荐讨论方向）

在 taxonomy 或 `required_actions` 语义上显式二分：

| 概念 | 含义 | 卡示例 |
|---|---|---|
| **self_contained_calc** | 操作数全在 user 文本，不要求文档 | `calculation` + `["calculator"]` |
| **grounded_calc** | 操作数/公式/产能来自文档，算完仍要可引用 | `calculation` + `["dense","calculator"]` 或 `rag_fact` + calculator |

实现增量：

- 提示：用第三人称写清边界 + 1～2 条抽象 few-shot（**禁止**写进 gold 原文）。  
- 可选：新 type `grounded_calc`（要改 `QuestionType` + serde + 文档）——更清晰，但 taxonomy 膨胀。  
- 更轻：不新增 type，只在 `required_actions` 约定「有 doc_scope 且题涉文档实体/型号/专名 → 必须带 retrieval」。

| 优点 | 缺点 |
|---|---|
| 边界可测、可回归 | 模型仍可能误分类 |
| 与现有 required_action 闸兼容 | 专名启发规则要写进提示，避免过拟合语料 |

#### 方案 B3 — 评测 / 标签契约（不改运行时也能止血）

对「题面已含全部操作数 + calculator 正确 + must_include 命中」的题：

- 设 `expect_no_retrieval` / 或 faith 的 context_source 允许 **`tool_outputs`**（calculator stdout）算 grounded；  
- 或 label 映射：`CORRECT_UNGROUNDED` + calculator-only + 数字全在 question → 计 **PASS**（产品可接受「无引用但算对」）。

| 优点 | 缺点 |
|---|---|
| 不增加检索成本 | 不提升真实引用质量 |
| 分数不再因「算对无引用」被打脸 | 与「RAG 题必须可引用」产品 KPI 可能冲突 |

q30 golden 现有：`mode=rag`，`source_chunks` 有文档子串，**没有** `expected_tool: calculator`——评测意图偏 **文档事实**，不是 pure calc。

#### 方案 B4 — 宿主结构闸（偏硬）

当 `question_type=calculation` 且 mode 挂载 rag 组且 `doc_scope` 非空：

- 必做动作闸：除 calculator 外 **还要求** 至少一次 Ok 的 dense/lexical/grep；  
- 或 evidence 闸：calculation 在 rag mode 下也要求 retrieval observation。

| 优点 | 缺点 |
|---|---|
| 不依赖卡是否写 dual | 与 AGENTS「宿主不语义判完备、只数 Ok」一致的是结构扩展，但仍改变 stop 行为 |
| 对 q30 类强约束 | 真·纯算术（数字全在题面）在 rag session 里也会被逼检索 |

与现有哲学：卡是「声明」不是「tool_choice 强制」；加硬闸等于把 dual 变成强制结构。

#### 方案 B5 — skill / knowledge-base 第三人称观察（软）

不改卡 taxonomy，在 KB skill 写：

> 题面给出算式操作数、但问题指向文档实体/型号/产线时，observation 中若尚无文档回传，终答侧数字仍处「未与语料核对」状态。

| 优点 | 缺点 |
|---|---|
| 符合 third-person voice | 不保证模型改行为 |
| 无代码闸 | 评测仍可能 UNGROUNDED |

### B.4 已拍板组合（2026-08-06）

**q30 类：只改评估标准（B3 / P-calc-ok）。** 不改 query-card 提示、不改宿主闸。

- 运行时题卡 `question_type=calculation` → 评测 `expect_no_retrieval` 语义  
- 算对（correctness ≥ τ）→ **PASS**，不因 recall=0 / faith 低打 CORRECT_UNGROUNDED  
- 代码：`product_e2e` `finish_score` + `rejudge` + `aggregate` 单测  

**§A 已落地**（见 §A.5）。B1/B2/B4 **本议题不做**。

### B.5 明确不做（本议题）

- 不在 host 做「答案是否等于 2800」语义裁判。  
- 不把 calculator 结果伪造成 retrieval citation 糊弄 faith（除非明确 tool_outputs 进评测契约）。  
- 不把 golden 题面写进 query-card few-shot。

### B.6 验证计划（实施时）

| 门 | 内容 |
|---|---|
| 配置/单测 | rerank style + bge 路径 |
| prompt | query-card 文案 diff；第三人称自检 |
| 定向 E2E | q30 + 1 道 pure calc（如 option_d calculator）+ 1 道文档费率计算 |
| full-149 切片 | thesis_numeric 子集 PASS / CORRECT_UNGROUNDED 变化 |

---

## C. 与 full-149 其它病灶的边界

| 病灶 | 本文是否覆盖 |
|---|---|
| Rerank 404 / Multimodal 误路由 | **A 覆盖** |
| q30 CORRECT_UNGROUNDED | **B 覆盖** |
| q61 SELECTION_MISS / q99 RETRIEVAL_MISS | 部分依赖 A；其余为检索排序与提示，另案 |
| UNGROUNDED 外推（Stage-Gate/TOGAF） | 不在本文；归 skill 外推纪律 |
| JUDGE_ERROR / Grok 上游 | 不在本文 |
| UNGROUNDED 外推 / 假截断 / 天气字段 / 文档元数据 | **§E 提示补强（2026-08-06）** |

---

## E. 提示补强：外推 · 截断 · 天气 · 元数据（2026-08-06）

针对 full-149 行为诊断中的 **A 类**（第一次提示/读 observation 问题），在 `prompts/` 第三人称补强，**不**加宿主语义闸。

| 病灶 | 代表题 | 改动位置 |
|---|---|---|
| 文档侧正确、业界框架侧训练记忆填表 → UNGROUNDED | q117/q118 | `knowledge-base/contract.md`、`SKILL.md` 证据节；`strategies-grounding` FS8 + gotcha |
| 回传已有条目仍称「截断」拒答 → REFUSAL_WRONG | q139 | `SKILL.md` 空结果表；`strategies-grounding` FS9 + gotcha |
| 天气 tool Ok 但实况字段编造 → UNGROUNDED | q125 | `system/agent-base.md` 天气字段对应表 |
| domain/genre unknown、忽略 name 后缀 → PARTIAL | q132 | `SKILL.md` doc_summary 行；`strategies-grounding` FS10 + gotcha |

版本：knowledge-base skill **4.8**，contract **1.4**。

---

## D. 决策（已拍板 2026-08-06）

### D.1 Rerank — **已落地**

见 **§A.5**。

### D.2 q30 类 — **P-calc-ok（只改评估标准）**

**拍板：** 若运行时 **query-card `question_type=calculation`**，评测 **不要求检索证据**；答案算对（judge correctness ≥ τ）即按 **PASS** 路径，不得因 faith 低 / recall=0 打成 `CORRECT_UNGROUNDED` / `RETRIEVAL_MISS`。

| 项 | 决定 |
|---|---|
| 运行时 / query-card 提示 | **不改**（卡仍可为纯 calculator） |
| 宿主闸 | **不改** |
| 评估 | 题卡 `calculation` ⇒ 视同 `expect_no_retrieval` 语义（faith / retrieval 硬标签不适用） |
| 范围 | 仅 **runtime 卡类型**，不以 golden 手写字段替代（卡是 L0 真实产出） |

**实现要点（eval_v2 + product_e2e）：**

1. `score_question` 读 `obs.query_card.question_type == "calculation"`。  
2. 传入 `label_for` 时：`expect_no_retrieval = example.expect_no_retrieval || calculation_card`。  
3. `ScoreV2.expect_no_retrieval` 同步为 true，便于 suite 统计排除检索均值。  
4. 单测：AC=1、FA=0、recall=0、`expect_no_retrieval=true` → **PASS**（原 CORRECT_UNGROUNDED 用例保留 `expect_no_retrieval=false`）。  
5. `rejudge` 读 artifact 上的 `query_card`，calculation 同样放宽。

**明确不做：** B1/B2/B4 提示或硬闸；不为 calculator 伪造 retrieval citation。
