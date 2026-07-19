# 状态：黄金测试集（Orchestrator 新范式 + 能力分组）扩充任务

**日期:** 2026-07-19  
**范围:** 你在另一窗口下达的任务——基于 OneDrive 文档与现有 ADR，优化题目；覆盖 chat / rag / search **及 rag+search 综合**；按能力分组；含 codegen 通道与内置工具/记忆等  
**检查人:** 本会话（只读核查工作树 + commit + 语料路径）  
**状态总览:** **设计 + JSON + runner 接线大体完成（v4 未提交）；新语料 3 份 fixture 缺失 → 全量 E2E 尚不能跑通；未跑 nightly 实测**

---

## 1. 任务对照（你要什么 vs 当前有什么）

| 你的要求 | 当前状态 | 证据 |
|---|---|---|
| 根据 OneDrive 文档优化/扩充题目 | **部分完成** | 设计文档已映射 10 份语料（含你给的论文/IPD/白药/RBF/预制菜/手艺人）；新 3 篇 **fixture 文件未入库** |
| 新范式：chat / rag / search **及 rag+search** | **完成（集内）** | 子集 + `capabilities` 字段 |
| 参考 ADR + 已有 golden 最佳实践 | **完成** | 沿用 `must_include` / 对抗 / 分 scope；扩展 `expect_citations`、环境 skip |
| 能力分组，每组测特定能力 | **完成（集内）** | 17 个 subset，见 §3 |
| codegen：dense / bm25 / triplet / summary / toc / metadata | **部分** | 有 dense/lexical/graph/summary/profile 意图题；**无独立 toc 题**；graph 依赖 triplet 重灌 |
| 计算器 / 天气 / 位置 / 时间 / 指代 / 记忆 | **部分** | 有 calculator、weather、时间(`client_time`)、指代+记忆(`prior_turns`)；**无独立「位置/geo」题** |
| 提交可运行 | **未完成** | v4 改动仍在工作树；新 txt/docx fixture 缺失 |
| 跑通生产评测 | **未验证** | 未见本任务的 `rag_quality_prod` 成功日志 |

---

## 2. 交付物进度（版本线）

| 版本 | 位置 | 题量 | 提交状态 |
|---|---|---|---|
| v2 内容语料 | `golden_set_realistic.json` 前十子集 | 107 | 已在 master 历史中 |
| **v3.0.0-orchestrator** | 同上 + 字段 + runner | **119**（+12 编排范式） | **已提交** `ee73c9c` |
| **v4.0.0-orchestrator-groups** | 同上再拆组 + 新语料题 | **143**（+36） | **仅工作树，未 commit** |

### 2.1 已提交（`ee73c9c`）

- 全部旧题补 `capabilities:["rag"]`（mode 回退保留）
- 新字段：`capabilities` / `doc_scope_hint` / `expect_citations{min_doc,min_web}` / `requires_network`
- `orchestrator_paradigm` 12 题（后被 v4 拆成 8+联合组）
- `realistic_corpus_full_eval`：按 hint 解析 scope + 请求带 capabilities
- 设计文档 §6

### 2.2 未提交（工作树 diff，相对 `ee73c9c`）

| 路径 | 变更 |
|---|---|
| `avrag-rs/tests/rag_quality/golden_set_realistic.json` | v4 版本串 + 分组 + 约 +36 题 |
| `GOLDEN_SET_REALISTIC_DESIGN.md` | §7 能力分组说明 + OneDrive 映射 |
| `src/golden_set.rs` | `prior_turns` / `client_time` / PriorTurn；v4 加载单测 |
| `rag_quality_prod.rs` | corpus 10 文件 + scope `rbf/prepared_food/craftsman` + `chat_v3` |
| `test_context/http.rs` | `chat_v3`（capabilities + messages 历史 + client_context.local_time） |

`cargo test -p rag_quality --lib realistic_v4`：**通过**（JSON 可加载、子集计数正确）。

---

## 3. 当前集结构（v4 工作树）

**版本串:** `4.0.0-orchestrator-groups`  
**总计:** **143** 题 / **17** 子集

### 3.1 内容组（v2 遗留，107）

| 子集 | n | 能力标签 |
|---|---|---|
| thesis_factual / synthesis / numeric / adversarial | 15+10+12+8 | rag |
| adr_factual / cross_adr | 12+5 | rag |
| consulting_factual | 14 | rag |
| ipd_table | 12 | rag |
| baiyao_pdf | 11 | rag |
| cross_document | 8 | rag |

### 3.2 新范式 / 能力组（v4，36）

| 子集 | n | 测的能力 | caps / 备注 |
|---|---|---|---|
| `orchestrator_paradigm` | 8 | 空选择、纯聊天、跨文档张冠李戴、多 chunk | rag 或 `[]`；部分 `doc_scope_hint=empty` |
| `rag_search_joint` | 6 | **RAG+Search 综合** + min_doc+min_web | `["rag","search"]`，`requires_network` |
| `chat_builtin_tools` | 4 | calculator×2、时间(`client_time`)、weather | `[]`；1 题需网 |
| `rag_codegen_channels` | 7 | dense×2、lexical×2、graph×1、summary、profile | rag；1 题 `requires_triplet_reingest` |
| `memory_coreference` | 3 | prior_turns 指代/记忆 | rag 或 chat |
| `search_web` | 2 | 纯 Search | `["search"]`，需网 |
| `new_corpus_factual` | 6 | RBF / 预制菜 / 手艺人各 2 题 | rag + 对应 scope |

**字段使用统计（全 143）：**

| 字段 | 使用次数 |
|---|---|
| `prior_turns` | 3 |
| `client_time` | 1 |
| `requires_network` | 9（joint 6 + search 2 + weather 1） |
| `requires_triplet_reingest` | 1 |
| `expected_tool`（意图记录） | 12（内容门为主，非 SSE 工具观测） |
| `expect_citations` | 新组普遍有；旧 107 多为 null |

---

## 4. 语料与 OneDrive 映射

### 4.1 你点名的路径（本机 `/mnt/e/OneDrive/...` 均可访问）

| 原件 | 状态 |
|---|---|
| `.../答辩/41911407-邢川-林海芬格式2.docx` | ✅ 存在（= fixture thesis，484371 B） |
| `.../H为-IPD流程各阶段370个活动详解(3)(1).xlsx` | ✅ 存在（= fixture ipd） |
| `.../RBF、滴灌通和乐旋乒乓.docx` | ✅ 存在；**fixture 未拷入** |
| `.../预制菜映射中国企业价值观困境.docx` | ✅ 存在；**fixture 未拷入** |
| `.../手艺人模式的增长悖论.docx` | ✅ 存在；**fixture 未拷入** |
| `.../【呈云南白药】…IT规划0411.pdf` | ✅ 存在（= fixture baiyao） |

### 4.2 项目内 fixtures（`product_e2e/fixtures/`）

| 文件 | 状态 |
|---|---|
| thesis / adr×2 / consulting platform+compensation / ipd / baiyao（docx/md/xlsx/pdf + txt 抽取） | ✅ 在 |
| `consulting_rbf_drc.txt` / `consulting_prepared_food.txt` / `consulting_craftsman_paradox.txt`（及 docx） | ❌ **目录中不存在** |

设计文档 §7.1 / §7.5 写「OneDrive 原件已入库 fixtures」——**与磁盘不符**，是当前最大阻塞。

Runner 已写死会上传这三份 `.txt`；缺文件时 `realistic_corpus_full_eval` 会在 upload 阶段失败。

---

## 5. 缺口清单（按优先级）

### P0 — 阻塞全量跑

1. **入库 3 份新语料**：从 OneDrive 拷贝 docx，并生成 `.txt` 抽取版（与现有 thesis/ipd 一致，便于离线 office 也可跑）。
2. **提交 v4 工作树**（json + design + golden_set.rs + runner + fixtures），避免只停留在未提交状态。
3. **smoke 跑** `realistic_corpus_full_eval`（可先 `E2E_SKIP_NETWORK_CASES=1` 跳过 9 道联网题）。

### P1 — 相对你需求的能力空洞

| 能力 | 缺口 |
|---|---|
| codegen **toc** | 无独立题（`doc_profile`/sections 仅部分覆盖） |
| **位置 / geo / IP 城市** | 无 `user_context` 位置题 |
| **conversation_history_load** 编排器记忆工具 | 仅 content 层 prior_turns；未断言 SSE/工具调用 |
| **dense/lexical/graph 真观测** | V2 worker 内通道不可见；题用内容门 + `expected_tool` 意图记录，**不是** tool_coverage SSE 门 |
| graph 真数据 | 需 `INGESTION_TRIPLET_ENABLED=1` 并对新语料 reindex（基准转型报告已做过；**新 3 篇+全语料**未在本任务验证） |
| 题 ID | 大量 example **无稳定 `id` 字段**（仅 query 文本），回归 diff 不便 |

### P2 — 工程卫生

- 旧 107 题多数无 `doc_scope_hint`（runner 默认 all）——跨文档干扰面大，可逐步收窄。
- PRD 100–500：143 已过下限，但 skill/工具面仍偏薄（chat 工具组仅 4 题）。
- `take(5)` 等历史限制：设计 §5 仍提生产子集限制，需确认 prod 入口是否已吃全量 143。

---

## 6. 建议的「完成定义」与下一步

### 完成定义（建议）

1. fixtures 10 份齐 + v4 commit 在 master。  
2. `cargo test -p rag_quality --lib` 全绿。  
3. 至少一次 `E2E_MODE=nightly … rag_quality_prod … realistic_corpus_full_eval`：  
   - 跳过网：≥ 非联网题全跑完有汇总；  
   - 开网 + 代理：joint/search/weather 不因 DNS 红。  
4. 可选：补 toc / geo / 稳定 id 字段的一小批增量题。

### 建议执行顺序（下一窗口）

```text
1. 从 OneDrive 复制 3 篇 docx → fixtures/
2. 抽取 txt（LibreOffice/现有解析脚本）
3. git add fixtures + rag_quality v4 + runner → commit
4. 本地 upload+ingest 10 份 → 跑 full_eval（先 skip network）
5. 补 P1 空洞（toc、geo、id）按需
```

---

## 7. 关键路径速查

| 用途 | 路径 |
|---|---|
| 黄金集 | `avrag-rs/tests/rag_quality/golden_set_realistic.json` |
| 设计 | `avrag-rs/tests/rag_quality/GOLDEN_SET_REALISTIC_DESIGN.md` |
| 类型/加载 | `avrag-rs/tests/rag_quality/src/golden_set.rs` |
| 生产评测入口 | `avrag-rs/crates/app/tests/product_e2e/llm_real/rag_quality_prod.rs` |
| HTTP 助手 | `…/test_context/http.rs`（`chat_v3`） |
| 语料 | `avrag-rs/crates/app/tests/product_e2e/fixtures/` |
| 工具面旧集 | `golden_set_tools.json`（与 realistic 并行，未在本任务合并） |
| v3 提交 | `ee73c9c` |
| 编排验收（另一线） | `docs/engineering/ORCHESTRATOR_HANDOFF_2026-07-18.md` |

---

## 8. 一句话结论

**另一窗口已把「Orchestrator 能力标签 + 分组黄金集」推进到 v4 设计与 143 题 JSON/runner 级，且单测可加载；但 3 份新咨询语料未进 fixtures、v4 未提交、全量 E2E 未跑——相对你给出的文档清单与「可执行的能力分组回归」，任务处于约 70%（集面齐、语料/落地未齐）。**

---

**状态栏**

| 项 | |
|---|---|
| 集版本（工作树） | `4.0.0-orchestrator-groups` / 143 题 |
| 已提交最高 | v3 / 119 / `ee73c9c` |
| 阻塞 | 缺 3× consulting fixture txt/docx |
| 下一动作 | 入库语料 → commit v4 → nightly smoke |
