# Ingestion 管线改造：原文均切窗口会话 + PS 合一 + Triplet 同链 + SaC 去 profile

| 项目 | 内容 |
|---|---|
| 类型 | 设计规格（ingestion LLM + 查询侧 SaC 同步） |
| 日期 | 2026-08-06 |
| 状态 | **收口提交（2026-08-07）**：windowed PS+triplet + 删 `doc_profile` + `doc_summary` 合一；本机 **migrate 0075 applied**（`document_toc.overview`）；`window_split` 8 单测绿；真文档 LLM 探针仍建议上线前补一次 |
| 范围 | worker 摄取 LLM（profile/summary/triplet）；`doc_summary` / 删除 `doc_profile`；prompts / skill / SDK 原语 |
| 前序 | `docs/plans/2026-08-03-ingestion-gemini-session-struct-codeonly.md`（DashScope 合一会话 + 缓存键约束）；E2E full-149 tool_trace 观察（agent 不用 `chunk_fetch`，profile/summary 分调） |
| 非范围 | 检索 chunker / embedding / BM25 切块逻辑；图检索算法本身；VLM visual triplet |

---

## 0. 一句话

**摄取 LLM 不再按 retrieval chunk 打包喂模型，也不再拆 profile/summary 两阶段；按模型上下文窗 80% 均分原文进 system，同会话两轮（PS 合一 JSON → triplet），超窗多会话后 LLM 融合；查询侧只保留 `doc_summary`，删除 `doc_profile`。**

---

## 1. 动机与事实依据

### 1.1 现状问题

| 点 | 现状 | 问题 |
|---|---|---|
| Triplet 输入 | `build_triplet_extraction_batches` 按 chunk 拼 JSON，默认 `INGESTION_TRIPLET_TOKEN_BUDGET=3000` | Qwen 3.7 约 1M 窗；中等文档（~100 chunk）被切成十余批串行，浪费 |
| Triplet 输出 | 强制 `chunk_id`，进 `supporting_chunk_ids` | E2E 中 agent **不按 id 取正文**（`chunk_fetch` 已从 sandbox 移除）；绑 id 收益低 |
| 会话正文 | seed 用 chunk map 全文；summary 续接不贴正文；triplet **再贴 chunk 批** | 与「正文已在会话」不一致；triplet 破坏前缀复用收益 |
| Profile / Summary | 两阶段、两套 prompt/产物 | 同源同窗可一轮完成；查询侧却常分调两次 |
| 查询 SaC | `client.doc_profile` + `client.doc_summary` | 与灌库合一产物不对齐；`level=section` 误用产生 Error |

### 1.2 E2E 观察（full-149，`v2_20260805-070626`）

- `doc_profile`：~47 次 / ~38 题；`doc_summary`：~24 次 / ~21 题。
- 全跑 `tool_trace.request` 中 **0** 次携带 chunk UUID 去取文。
- 工具面无 `chunk_fetch`；profile 后多为 `doc_grep` / dense / lexical。
- 结论：**查询不依赖 TOC 上的 chunk_id 定点拉取**；灌库强制模型标 chunk_id 对当前 agent 路径价值有限。

### 1.3 仍保留的边界

- **检索侧 chunker 不动**：向量 / 词法 / 引用仍依赖 body chunk。
- **TOC 不再写 `chunk_id`（G1）**：导航靠标题 + 章节简介 + 后续检索；`doc_profile.sections[].chunk_id` 产品语义废弃。
- **Graph**：triplet 无 supporting → 本期 **不写** `graph_passage`（空 `supporting_chunk_ids`）；实体/关系向量仍可建。

---

## 2. 拍板决策一览

| # | 议题 | 决议 |
|---|------|------|
| 1 | TOC / chunk 锚 | **G1+**：无 `chunk_id`；输出含 **元数据 + 文档摘要 + 章节树 + 章节简介** |
| 2 | 超窗策略 | **F1**：每窗独立跑满 PS+triplet，再 **LLM merge（融合）** |
| 3 | Triplet 与 PS 关系 | **同会话第 2 轮**（`produce` 续接），吃 session cache；**不**新开链 |
| 4 | 正文位置 | **system** = 同源短提示 + 本窗原文；任务只在 **user** |
| 5 | PS 输出形态 | **单一 JSON**（非双代码块） |
| 6 | 窗口额度 | 注册 `context_window_tokens`（env）；**K = 0.8 × C**；**均分**最小 N 使 T/N &lt; K |
| 7 | 查询 SaC | **删除 `doc_profile`**；**仅 `doc_summary`**，返回与灌库同构的合一 JSON |
| 8 | Skill / prompt | 全部去掉独立 profile 教学；docscope 链改为 `docscope → doc_summary` |

---

## 3. 窗口切分

### 3.1 配置

```text
# 官方上下文上限 C（tokens）
INGESTION_LLM_CONTEXT_WINDOW_TOKENS=1000000

# 可选覆盖利用率，默认 0.8
INGESTION_LLM_WINDOW_UTILIZATION=0.8
```

`ModelProviderConfig`（`avrag_llm` + `app-core`）增加：

- `context_window_tokens: Option<u64>`
- 可选 `window_utilization: Option<f32>`（默认 0.8）

### 3.2 公式

\[
C = \text{context\_window\_tokens},\quad
K = \lfloor u \cdot C \rfloor\ (u=0.8),\quad
T = \mathrm{estimate}(\mathrm{raw\_text}),\quad
N = \max\bigl(1,\ \lceil T / K \rceil\bigr)
\]

- **均分、不灌满**：将全文切成 **N 段近似等长**（token 估计；边界尽量贴段落/空行），每段约 \(T/N &lt; K\)，**不是**「能塞满 K 再切下一段」。
- \(T \le K\) → \(N = 1\)。
- 实现注：若实测 system 短提示 + user 任务 + max completion 挤占过多，可在 `K` 内再扣 `reserve`，使 **body 预算** 为 `K - reserve`；默认规格以字面 \(T/N &lt; K\) 为准，探针后可收紧。

### 3.3 输入原文

- `raw_text` = 解析后全文（markdown/plain），**不**包装为 `{"chunks":[{chunk_id,text}]}`。
- 检索用 body chunk 仍由现有 chunker 从同一文档产出，与 LLM 窗无关。

---

## 4. 每窗会话形状

每窗一个 `DocumentIngestionSession`，**两轮、同一 `previous_response_id` 链**：

| 轮 | system（窗内**两轮恒等**） | user | 产出 |
|---|---|---|---|
| 1 seed | `SESSION_HINT` + 本窗原文 | PS 联合指令 | 单一 JSON：metadata / summary / sections |
| 2 produce | **与第 1 轮相同** | Triplet 指令（无 chunk_id） | `{"triplets":[...]}` |

### 4.1 DashScope 缓存约束（继承前序）

- 会话缓存键含 **instructions（system）**；续接轮 system 必须与 seed **完全一致** 才命中。
- 因此：阶段指令（PS / triplet）**只放 user**；正文 + 同源短提示放 **system**。
- 窗与窗之间 system 正文不同 → **不同会话**（不能一条链跨窗）。

### 4.2 同源短提示（system 前缀，第三人称观察语气）

要点（正式文案进 `prompts/pipeline/`）：

- 本会话处理同一篇文档的**同一窗口原文**。
- 各轮产出类别不同（结构+摘要 vs 三元组），均基于 system 中已载入的同一源。
- 各轮输出独立成篇；不交叉引用他轮产物。

### 4.3 废弃路径

| 废弃 | 替代 |
|---|---|
| 默认 3k token 的 chunk 批 triplet | 窗内单轮（或极少轮）triplet |
| profile → summary → 多批 triplet 三阶段 | 每窗 2 轮 |
| seed user 塞 chunk map 作「假 system」阶段文案 | system=原文；user=任务 |
| Triplet JSON 强制 `chunk_id` | 无 id；空 supporting |

---

## 5. 输出契约

### 5.1 Profile+Summary（每窗 turn1）— 单一 JSON

```json
{
  "metadata": {
    "language": "zh",
    "domain": "...",
    "genre": "...",
    "era": "...",
    "author": null,
    "publication_date": null,
    "title": "..."
  },
  "summary": "文档级摘要正文（字符串；可多行）",
  "sections": [
    {
      "title": "章节标题",
      "heading_level": 1,
      "rank": 0,
      "overview": "本章节简介",
      "children": []
    }
  ]
}
```

**硬约束：**

- **无** `chunk_id` / `chunk_ids`。
- `sections` 为树（可用 `children` 或扁平 `rank`+`heading_level`；实施时 prompt 与 parse 二选一并写死一种）。
- 落库：`summary` → 现有 summary 路径；`metadata` → profile metadata；`sections` → TOC（`chunk_id = null`；**需支持 overview 字段**，见 §8）。

### 5.2 Triplets（每窗 turn2）

```json
{
  "triplets": [
    {
      "subject": "...",
      "predicate": "类型|部分|参与|依赖|位于|标识",
      "object": "..."
    }
  ]
}
```

- 闭集谓词 + 现有 normalize / semantic lint。
- **无** `chunk_id`；`supporting_chunk_ids = []`；不写 graph passage。
- 多窗：先确定性 (s,p,o) 去重；可选 LLM 融合（默认去重即可，冲突多再 LLM）。

### 5.3 多窗 LLM merge（N &gt; 1）

输入：各窗 PS JSON（及可选已去重 triplets）。**不**再贴全文。

- **PS merge**：一次 LLM 调用，融合规则强调：
  - summary 收成**一篇**文档级摘要（非「片段 1/2」并列）；
  - sections 按文档逻辑序合成一棵树；重复标题合并简介；不发明原文没有的章；
  - metadata 冲突取更具体/完整侧；无法判断则省略臆测字段。
- **Triplet merge**：默认规则去重；需要时再 LLM。
- Merge 可用同一 `INGESTION_LLM`；新会话即可（输入远小于正文）。

提示词：第三人称观察式，落 `prompts/pipeline/`（如 `profile-summary-merge.system.md` / user 模板）。

---

## 6. 查询侧 SaC / Skill（同步改造）

### 6.1 原语

| 动作 | 说明 |
|---|---|
| **删除** `client.doc_profile` / `doc_profile` 工具 / `DocProfileArgs` / `retrieval_doc_profile` | 无兼容 shim（设计原则：不背兼容税） |
| **保留并扩展** `client.doc_summary(doc_ids=None)` | 去掉 `level` 参数；一次返回 metadata + summary + sections（含 overview，无 chunk_id） |

返回形状与灌库 JSON 同构（每 doc 一条，外包 list 或按现有 bridge 包装约定实施时统一）。

### 6.2 代码触点（实施清单）

| 区域 | 文件/模块（示意） |
|---|---|
| SDK 原语表 | `contracts/src/sdk_primitives.rs` |
| 工具 args | `contracts/src/tool_call.rs` |
| 工具实现 | `rag-core/src/runtime/tools/doc_summary.rs`（合并读库）；删除 `doc_profile.rs` 入口 |
| Bridge | `code-interpreter/src/bridge.rs` |
| Catalog | `agent-tools` catalog / tool_registry |
| exit_policy | doc_profile 非证据规则 → 适用于新 `doc_summary`（仍非正文证据） |
| 测试 / mock | multitool「profile → chunk_fetch」改为 `doc_summary → dense/grep`；golden tool 序列 |
| Python SDK | `python/avrag_sdk` 若仍维护则同步删 profile |

### 6.3 Prompts / skills

| 路径 | 改法 |
|---|---|
| `prompts/capabilities/knowledge-base/SKILL.md` | 表合并为一行 `doc_summary` |
| `prompts/capabilities/knowledge-base/contract.md` | 单篇档案只经 `doc_summary` |
| `.../strategies-grounding.md` | metadata 来源改为 summary 回传 |
| `prompts/clusters/docscope/SKILL.md` | 链：`docscope → doc_summary`；删 profile 示例 |
| `prompts/README.md` | 去掉 profile→summary 教学链 |
| `prompts/pipeline/query-card.system.md` | 动作列表只留 `doc_summary` |
| `prompts/loop/codegen-sandbox-error.nudge.md` | 原语列表去 `doc_profile` |
| skillopt `avrag149/skills/*` | 镜像同步 |

**Agent 新习惯：**

| 旧 | 新 |
|---|---|
| `doc_profile` 看作者/章节 | `doc_summary` → `metadata` + `sections` |
| `doc_summary` 只拿概览 | 同次调用的 `summary` 字段 |
| `doc_summary(level=section)` | **删除**；章节看 `sections[].title/overview` |
| profile 后按 chunk_id fetch | **不做**；grep / dense / lexical |

---

## 7. 端到端算法（伪代码）

```
C = env INGESTION_LLM_CONTEXT_WINDOW_TOKENS
K = floor(0.8 * C)
raw = document_full_text()          # 非 chunk JSON
T = estimate_tokens(raw)
N = max(1, ceil(T / K))
windows = even_partition(raw, N)    # 均分，贴段落边界

ps_windows = []
triplets_all = []

for w in windows:
  system = SESSION_HINT + "\n\n" + w.text
  session = DocumentIngestionSession::new(llm)
  turn1 = session.seed(system_is_fixed_via_builder, user=PS_PROMPT)
  ps_windows.push(parse_ps_json(turn1))
  turn2 = session.produce(user=TRIPLET_PROMPT)   # 同链，system 不变
  triplets_all.extend(parse_triplets(turn2))

if N == 1:
  final_ps = ps_windows[0]
else:
  final_ps = llm_merge_ps(ps_windows)            # 融合规则

final_triplets = dedupe_triplets(triplets_all)   # 可选 llm_merge_triplets

persist_summary_and_toc(final_ps)                # TOC 无 chunk_id，有 overview
persist_graph(final_triplets)                    # supporting 空
# chunker + embed 仍走既有 materialize/index
```

`DocumentIngestionSession` 组装需改为：

- system = `INTERACTION`/`SESSION_HINT` + 窗正文（或替换为新的 pipeline prompt 文件）；
- user = 纯阶段指令（不再把 170 行 triplet「假 system」与正文混槽逻辑搞错——阶段文案始终 user）。

---

## 8. 存储与 schema

| 项 | 要求 |
|---|---|
| Summary 正文 | 继续 summary chunk / `update_document_summary` |
| TOC | `title`, `heading_level`, `rank`, **`overview`**；`chunk_id` 恒空或不写 |
| `TocEntry` / PG | 若无 overview 列：扩展列或 metadata JSON（实施时选最小 schema 变更） |
| `doc_summary` 读路径 | 一次组装 metadata + summary 正文 + TOC sections |
| Graph | 无 supporting → skip passage；entity/relation 照常 |

前端内容源弹窗：正文前展示 summary 的 UI 已于 2026-08-06 落地（`WorkspaceSourceViewer`）；灌库 summary 质量随本管线提升后自动受益。

---

## 9. Prompts 资产（prompts-in-md）

| 文件（建议名） | 用途 |
|---|---|
| `prompts/pipeline/interaction-session.system.md` 或继任 | 同源短提示（可改写；正文由代码拼接在后） |
| `prompts/pipeline/profile-summary.joint.md` | turn1 任务 + JSON schema |
| `prompts/templates/profile-summary-user.tmpl` | 若需要薄模板 |
| `prompts/pipeline/triplet-extraction.system.md` | 改：无 chunk；输入为「system 已载原文」 |
| `prompts/templates/triplet-extraction-user.tmpl` | 去掉 Valid chunk IDs / Chunks JSON |
| `prompts/pipeline/profile-summary-merge.*.md` | 多窗融合 |
| 删除或停止引用 | 独立 section-index 作为会话 seed 主路径的旧用法（检索 section-index 独立路径若仍存在需单独标注） |

Voice：第三人称观察，非命令清单（`AGENTS.md`）。

---

## 10. 实施切片与验证

### 10.1 切片顺序

1. **Config**：`context_window_tokens` + `K` + `even_partition` 纯函数与单测  
2. **Session 组装**：system=提示+原文；user=任务；两轮同链  
3. **PS 联合 prompt + parse**；TOC overview 落库  
4. **Triplet 去 chunk**；graph 空 supporting  
5. **多窗 LLM merge**  
6. **Worker 编排替换** profile / summary / triplet 旧路径；删除 3k 批主路径  
7. **SaC：删 doc_profile，扩 doc_summary**  
8. **Skills / query-card / nudge / skillopt 同步**  
9. **E2E / mock / golden 工具序列**  

### 10.2 验证门

| 门 | 口径 |
|---|---|
| 单测 | even_partition；parse PS JSON；triplet 无 id；session 消息 system 两轮相等 |
| 探针 | 单文档：N=1 时 cached_tokens 第 2 轮 &gt; 0；N&gt;1 时 merge 产出单 summary |
| 回归 | `cargo test -p` 触及 crate；frontend skill 文案无 `doc_profile` 引用（grep） |
| 可选 | full-149：`doc_profile` 调用次数 → 0；`doc_summary` 覆盖原 profile 题型 |

### 10.3 明确不做（本期）

- Host 将 triplet 实体对齐回 chunk（H2）  
- 恢复 `chunk_fetch`  
- 兼容旧 `doc_profile` API  
- 改变 dense/lexical/graph 检索算法  

---

## 11. 风险

| 风险 | 缓解 |
|---|---|
| 大 system 下 session-cache 行为与「短 instructions」探针不同 | 实施后补 1 次大正文两轮探针 |
| PS 合一 JSON 截断 / 格式混用 | 硬 schema + salvage；summary 控长 |
| 多窗 merge 丢细节 | 融合 prompt 要求保留数字与专名；可观测 merge 前后长度 |
| TOC 无 chunk_id 后个别旧脚本依赖 | 全仓删 profile；doc_summary 文档化 |
| 图 passage 变稀 | 接受；后续可做 H2 |

---

## 12. 与前序文档关系

| 文档 | 关系 |
|---|---|
| `2026-08-03-ingestion-gemini-session-struct-codeonly.md` | **继承** DashScope Responses、session cache、instructions 恒定约束；**推翻**「seed chunks + 三阶段 profile→summary→多批 triplet」的输入/轮次形态 |
| 本文 | 现行 ingestion LLM + SaC 档案读取的**目标规格**；实施完成后可将 08-03 文中过时段落标 SUPERSEDED 指向本文 |

---

## 13. 状态与下一步

- **规格**：已拍板（2026-08-06 对话确认）。  
- **代码**：未实施。  
- **下一步**：按 §10.1 切片开工；每切片可独立 `cargo test -p …` / 前端 skill 文案 grep 验收。

*完。*
