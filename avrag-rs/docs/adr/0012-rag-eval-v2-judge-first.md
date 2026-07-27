# ADR-0012: RAG Eval v2 — Judge-first 生成层评测

| 项目 | 内容 |
|---|---|
| 状态 | **已采纳（P0–P5 已实现；质量硬门闩待 30 题 κ 校准）** |
| 决策日期 | 2026-07-24 |
| 关联 | ADR-0011（分轨记分卡）；详细方案 [`docs/plans/2026-07-24-rag-eval-judge-v2-design.md`](../plans/2026-07-24-rag-eval-judge-v2-design.md) |
| 判定模型 | DeepSeek V4 Flash（默认 `MEMORY_LLM_*` / 可 `JUDGE_LLM_*` 覆盖） |

---

## 1. 背景

ADR-0011 正确解决了「检索 vs 选择」混计问题：检索读 `tool_results`，选择读 `citations`，分轨报告。

生成层正确性仍依赖：

1. `must_include` 字面 `answer.contains(...)`
2. 硬锚点 substring faithfulness（数字/日期/编号）

实践中暴露为**错设计**，不是实现细节：

- 合法改写与空格（如「2019 年」vs「2019年」）→ 假阴性  
- 缺 `must_include` 时正确性门闩过松 → 假阳性风险  
- `GENERATION_UNGROUNDED` 与「字符串未命中」混用 → 诊断误导  
- 与行业 RAG 评测（RAGAS / TruLens 等）的 **语义正确性 + context 忠实度** 主路径不一致  

ADR-0011 已声明 substring 仅 smoke、LLM-as-Judge 为后续；本 ADR 将 **Judge-first 升为生成层默认标准**，并明确 **不再在旧 must_include 门闩上打补丁**。

---

## 2. 决策

### 2.1 分轨保留，生成层换轨

| 层 | 数据源 | 指标 | 实现态度 |
|---|---|---|---|
| **检索** | `tool_results` → chunk id 序列 | Recall@k / Hit@k / MRR / nDCG@k | **保留** ADR-0011 确定性算法 |
| **选择** | `citations` ∩ retrieved / gold | Citation precision / recall | **保留** |
| **生成** | `answer` + reference + cited context | LLM-as-Judge 分数 | **本 ADR：默认路径** |
| **基建** | HTTP / parse / empty | INFRA 类 label | **保留** |

### 2.2 废弃作为 PASS 主门闩的做法

以下**不得**再作为 suite 正确性 / PASS 的主条件：

- `must_include` / `must_not_include` 裸子串匹配  
- 英文词重叠式 hallucination rate  
- 单独用硬锚点 substring faithfulness 判定「答对了」

上述字段可暂留在 golden JSON 中作注释或迁移兼容，**评分管道忽略**。

### 2.3 生成层标准：Judge-first

每题（或每题一次合并调用）使用 **DeepSeek V4 Flash**，temperature **0**，结构化 JSON，至少输出：

| 维度 | 含义 | 主要输入 |
|---|---|---|
| `answer_correctness` | 相对参考答案是否**语义**答对 | question + `reference_answer` + model answer |
| `faithfulness` | 答案事实是否被证据支持 | model answer + **cited** context（空则 retrieved 兜底并标记） |
| `answer_relevancy` | 是否在回答所问 | question + model answer |
| `refusal_correctness` | 拒答是否符合期望 | answer + `expected_should_answer` |

细则与 schema 以设计文档 §4 为准；prompt **版本化**（`schema_version: rag_eval_judge_v2`）。

**正确性与忠实度必须拆开**：答对但未 grounding → `UNGROUNDED`；胡编或语义错误 → `INCORRECT`；不得再用单一 `GENERATION_UNGROUNDED` 吞掉 must 失败。

### 2.4 Judge 配置（凭证静默复用）

| 变量 | 含义 |
|---|---|
| `JUDGE_LLM_BASE_URL` / `API_KEY` / `MODEL` | 优先；未设则回退 `MEMORY_LLM_*`，再回退 `AGENT_LLM_*` |
| `JUDGE_LLM_MODEL` 默认 | **`deepseek-v4-flash`**（与当前 `.env` 中 MEMORY 一致） |
| Thinking | **关闭**（结构化 JSON） |

**禁止**默认使用 agent 主模型（Pro）做 judge，以控制成本并降低同源自评偏见。

### 2.5 新管道，不贴旧皮

- 新模块：`tests/rag_quality/src/eval_v2/`（名称以实现为准）  
- Runner 开关：`RAG_EVAL_V2=1`（过渡）；稳定后默认 v2  
- 产物：`e2e_output/rag_eval_v2/{run_id}/`（summary + per-query + judge raw）  
- **不**在 `metrics_v2::answer_correctness` 上继续堆 normalize / alias 作为产品标准  

### 2.6 Golden 主字段

| 字段 | 角色 |
|---|---|
| `reference_answer`（兼容读旧 `expected_answer`） | Judge 正确性参考 / rubric |
| `source_chunks` | 检索层 gold（确定性） |
| `expected_should_answer` | 拒答期望 |
| `rubric_notes`（可选） | 写入 judge 的补充约定 |
| `must_include` / `must_not_include` | **不参与 v2 评分** |

### 2.7 诊断标签（分数驱动，示意）

优先级：基建 / Judge 失败 → `RETRIEVAL_MISS` → `SELECTION_MISS` → `REFUSAL_WRONG` → `UNGROUNDED` → `INCORRECT` / `PARTIAL` → `PASS`。

阈值（`τ_c` / `τ_f` 等）**经小集人工校准后再作硬门闩**；首版实现只报告、不对质量分 fail 测试（`JUDGE_ERROR` / HTTP 5xx 可另作 fail-fast）。

### 2.8 与评测纪律

- 评测保持中性：只衡量检索–选择–生成质量。  
- **不为**某次产品装配 bug 增加专项探针。  
- 一题一停（`E2E_START_AT` / `E2E_END_AT`）能力保留。

---

## 3. 后果

### 3.1 正面

- 与行业「reference 语义正确 + context 忠实」一致  
- 消除 must 字面匹配假阴性  
- 标签可指导修检索 / 选择 / 生成，而非修评测字符串  
- Flash 成本可控，可对 100+ 题 nightly 全量 judge  

### 3.2 负面与缓解

| 负面 | 缓解 |
|---|---|
| Judge 方差 | temp=0、prompt 版本、答案/上下文哈希缓存、κ 校准 |
| 与历史 label 不可直接对比 | 报告标明 v1/v2；过渡期可双写但不合并直方图 |
| 依赖 API | 与现有 e2e 相同；`JUDGE_ERROR` 显式暴露 |
| 实现工作量 | 设计文档 §12 切片；本 ADR 不要求一次切默认 |

### 3.3 对 ADR-0011 的修订

ADR-0011 的下列内容 **被本 ADR 取代**：

- 生成层以 substring faithfulness / must_include 作为正确性或 ungrounded 主依据  
- 将 LLM-as-Judge 仅标为远期、非默认  

ADR-0011 的下列内容 **继续有效**：

- 检索 / 选择分轨与数据源  
- Recall 相对 baseline 等**检索**门闩精神（具体数字可在 e2e-gates 更新）  
- tool_results 形状兼容要求  

---

## 4. 非目标

- 不改生产 agent 工具装配或提示词（属产品路径，与评测正交）  
- 不强制每题人工双标  
- 不在本 ADR 范围内实现代码（实现跟设计文档切片）  

---

## 5. 落地指针

| 项 | 位置 |
|---|---|
| 完整设计（schema、label、切片、报告） | `docs/plans/2026-07-24-rag-eval-judge-v2-design.md` |
| **实现（P0–P5 已落地）** | `tests/rag_quality/src/eval_v2/`（types/judge/parse/aggregate/report/cache）+ `eval_v2_calibration` bin；runner `realistic_corpus_full_eval` **默认 v2**（`RAG_EVAL_V2=0` 过渡期内可切回旧管道）；drift 对比 `e2e-analyzer rag-eval-v2-drift` |
| 既有分轨与提取 | `tests/rag_quality`（`harness_extract`、检索指标） |
| 旧 generation gate | `metrics_v2::answer_correctness` — 迁移完成后标 deprecated |
| 开关 | `RAG_EVAL_V2` / 后续默认 v2 |

---

## 6. 成功标准（验收本决策，非单题特例）

1. 语义正确、字面不同的答案可获高 `answer_correctness`，不因空格/同义单独失败。  
2. Context 不支持的具体事实拉低 `faithfulness`，与 correctness 分列。  
3. 检索 miss 仍由 Layer A 在 judge 之前归因。  
4. Suite 主报告为 mean correctness / faithfulness / recall，而非 must 命中率。  
5. 运行日志可见 judge 模型为 Flash（或显式 `JUDGE_LLM_MODEL`），默认非 Pro。
