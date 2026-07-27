# RAG Eval v2 — LLM-as-Judge 行业对齐设计

| 项目 | 内容 |
|---|---|
| 状态 | **已采纳设计**（ADR-0012）；实现待切片 |
| 日期 | 2026-07-24 |
| 判定模型 | **DeepSeek V4 Flash**（默认 `MEMORY_LLM_*` / 显式 `JUDGE_LLM_*`） |
| 关联 | **[ADR-0012](../adr/0012-rag-eval-v2-judge-first.md)**；取代 ADR-0011 生成层 must/substring 主门闩；**不**在 `metrics_v2` 上打补丁 |
| 范围 | 离线 / nightly 质量评测；不改生产 chat 路径 |

---

## 0. 决策摘要

1. **错设计不坚持**：`must_include` / 裸 `answer.contains` / 硬锚点 substring 作为 **PASS 主门闩** 正式退役。  
2. **向行业标准对齐**：检索用 **ID/rank 确定性指标**；生成正确性与忠实度用 **LLM-as-Judge**（参考 RAGAS / TruLens / ARES 分层）。  
3. **Judge 模型固定为 Flash**：廉价、低延迟、适合全量 100+ 题；与 agent 主模型（Pro）解耦，避免「自评偏袒」。  
4. **新管道并行落地**：`rag_eval_v2` 新模块 + 新 report；旧 `metrics_v2` 仅作过渡期对照，**不**继续加启发式规则。  
5. **评测保持中性**：只报告检索–选择–生成质量维度；不为某次产品 bug 加专项探针。

---

## 1. 为什么推倒生成层旧门闩

| 旧做法 | 问题 |
|---|---|
| `must_include` 字面 `contains` | 空格/全半角/合法改写 → 假阴性（Q1「2019 年」vs「2019年」） |
| 缺 must → 自动 true | 假阳性（胡编也可能 PASS） |
| Substring faithfulness 作 ungrounded | 只能抓硬数字；与 must 失败混成同一 label |
| 英文词重叠 hallucination | 中文几乎无意义（ADR-0011 已承认） |

行业共识（RAGAS 等）：

- **Context 指标**：context 是否相关、是否充分（可确定性 + 可选 judge）  
- **Answer 指标**：**faithfulness**（相对 context）+ **answer correctness / relevancy**（相对问题与参考答案）  
- 字符串 keyword 最多当 **开发期 smoke**，不作 release 语义标准。

---

## 2. 目标架构

```
┌─────────────────────────────────────────────────────────────────┐
│  Runner（realistic_corpus / future suites）                      │
│  chat_v3 → Artifact v2（answer, retrieved[], cited[], meta）     │
└────────────────────────────┬────────────────────────────────────┘
                             ▼
┌─────────────────────────────────────────────────────────────────┐
│  Layer A — Deterministic (零 LLM 成本，先算)                      │
│  • Retrieval: Recall@k / Hit@k / MRR / nDCG@k（gold chunk ID）   │
│  • Selection: Citation precision/recall vs retrieved ∩ gold      │
│  • Contract: empty answer / HTTP 5xx / parse fail（基建）        │
│  • Refusal heuristic (only for routing to judge rubric)          │
└────────────────────────────┬────────────────────────────────────┘
                             ▼
┌─────────────────────────────────────────────────────────────────┐
│  Layer B — Judge (DeepSeek V4 Flash, temp=0, JSON schema)        │
│  单次或两次调用，输出结构化分数 + 理由（见 §4）                    │
│  • answer_correctness  (vs reference + question)                 │
│  • faithfulness        (vs cited/retrieved context)              │
│  • answer_relevancy    (vs question only)                        │
│  • refusal_correctness (vs expected_should_answer)               │
│  • context_sufficiency (optional, vs gold intent)                │
└────────────────────────────┬────────────────────────────────────┘
                             ▼
┌─────────────────────────────────────────────────────────────────┐
│  Layer C — Aggregation                                           │
│  • 连续分数 + 阈值 label（诊断用，非 keyword 驱动）               │
│  • Suite 汇总 / 子集分层 / drift vs baseline                      │
│  • Gate：仅对校准后的阈值指标硬门闩（§7）                         │
└─────────────────────────────────────────────────────────────────┘
```

**原则：**

- Layer A 回答「证据有没有、管道有没有炸」。  
- Layer B 回答「答得对不对、有没有胡编」。  
- Label 由 **分数阈值** 推导，**不**由 must 字符串失败推导。

---

## 3. 与行业指标的映射

| 行业（RAGAS 等） | 本设计 v2 | 输入 | 输出 |
|---|---|---|---|
| Context Recall / Retrieval | Layer A `retrieval.*` | retrieved chunk ids + gold | 0–1 |
| Context Precision（简化） | Layer A `selection.*` + 可选 judge `context_precision` | cited vs retrieved/gold | 0–1 |
| Faithfulness | Judge `faithfulness` | answer + **cited**（优先）/ retrieved 兜底 | 0–1 + unsupported claims |
| Answer Correctness / Accuracy | Judge `answer_correctness` | answer + **reference** + question | 0–1 + verdict |
| Answer Relevancy | Judge `answer_relevancy` | answer + question | 0–1 |
| （拒答） | Judge `refusal_correctness` | answer + `expected_should_answer` | 0–1 |

**不做**的（YAGNI）：

- 全量 human double-blind 每轮（只做校准小集）  
- 多 judge 投票（Flash 单 judge；漂移时再开 second opinion）  
- 把 agent 主模型当 judge（成本高 + 同源偏见）

---

## 4. Judge 契约（DeepSeek V4 Flash）

### 4.1 配置（ENV，不询问用户）

| 变量 | 含义 | 默认 |
|---|---|---|
| `JUDGE_LLM_BASE_URL` | Judge API | 回退 `MEMORY_LLM_BASE_URL` → `AGENT_LLM_BASE_URL` |
| `JUDGE_LLM_API_KEY` | Key | 回退 `MEMORY_LLM_API_KEY` → `AGENT_LLM_API_KEY` |
| `JUDGE_LLM_MODEL` | 模型 | **默认 `deepseek-v4-flash`**（或 `MEMORY_LLM_MODEL` 若已是 flash） |
| `JUDGE_LLM_TIMEOUT_MS` | 超时 | `60000` |
| `JUDGE_LLM_TEMPERATURE` | 采样 | **`0`**（强制） |
| `JUDGE_LLM_ENABLE_THINKING` | 思考链 | **`false`**（结构化 JSON 优先） |
| `RAG_EVAL_V2=1` | 启用 v2 管道 | 未设则 runner 仍可走旧报告（过渡） |
| `RAG_EVAL_V2_ONLY=1` | 只跑 v2、不写旧 scorecard | 迁移完成后默认 |

实现：`JudgeClient::from_env()`，**禁止**默认绑 `AGENT_LLM_MODEL=pro`。

### 4.2 调用形态

**推荐：每题 1 次合并 judge**（控制费用与延迟）。

```
System: 你是严格的中文 RAG 评测员。只输出合法 JSON，不要 markdown 围栏。
User:   [question, reference_answer, expected_should_answer,
         model_answer, cited_context[], optional retrieved_summary]
```

**JSON schema（稳定字段，versioned）：**

```json
{
  "schema_version": "rag_eval_judge_v2",
  "refusal": {
    "is_refusal": true,
    "correct_for_expectation": true,
    "score": 1.0,
    "rationale": "…"
  },
  "answer_correctness": {
    "score": 0.0,
    "verdict": "correct|partial|incorrect|not_applicable",
    "rationale": "…",
    "key_points_hit": ["…"],
    "key_points_missed": ["…"]
  },
  "faithfulness": {
    "score": 0.0,
    "verdict": "grounded|mixed|ungrounded|not_applicable",
    "unsupported_claims": ["…"],
    "rationale": "…"
  },
  "answer_relevancy": {
    "score": 0.0,
    "rationale": "…"
  },
  "context_sufficiency": {
    "score": 0.0,
    "verdict": "sufficient|partial|insufficient|unknown",
    "rationale": "…"
  }
}
```

**评分细则（写入 system/user 固定段落，可版本化到 prompts）：**

1. **answer_correctness**  
   - 与 `reference_answer` **语义等价**即高分，允许改写、空格、同义、合理语序。  
   - `partial`：核心事实对但缺关键限定（年份对但公司张冠李戴等）。  
   - 参考答案是 **rubric**，不是字面模板。  
   - `expected_should_answer=false` 时：若正确拒答 → correctness=`not_applicable`，看 refusal；若仍作答 → correctness 低。

2. **faithfulness**  
   - 只根据 **cited_context**（若 cited 空则用 retrieved top 段落，并标记 `context_source=retrieved_fallback`）。  
   - 答案中每个实质性事实 claim 须被 context 支持；数字/日期/专名从严。  
   - 允许同义改写；不允许 context 没有的具体数字/实体。

3. **answer_relevancy**  
   - 是否在回答所问；文不对题即使「正确事实」也低分。

4. **refusal**  
   - 对齐 `expected_should_answer`；拒答话术多样化仍算拒答。

5. **禁止**  
   - 不要因「未出现某个精确字符串」扣 correctness。  
   - 不要用训练知识补全；context 没有就判 ungrounded / insufficient。

### 4.3 解析与失败

- `extract_first_json_object` + schema 校验。  
- 解析失败：`judge_status=error`，该题 **不**自动 PASS；label=`JUDGE_ERROR`（基建类，可 fail-fast 可选）。  
- 可选 1 次重试（仅 JSON 损坏）。

### 4.4 成本模型（估）

| 项 | 量级 |
|---|---|
| 题量 | ~150 |
| 每题 1 call Flash | ~1.5–4k tokens in + 0.4–0.8k out |
| 全量 | 远低于 agent 主路径多轮 ReAct |
| 缓存 | 对 (answer_hash, context_hash, prompt_version) 落盘，重跑可复用 |

Artifact：`e2e_output/rag_eval_v2/{run_id}/q{nnn}.judge.json`。

---

## 5. 诊断标签 v2（分数驱动）

优先级（错答归因，先管道后质量）：

| 优先级 | Label | 条件（示意阈值可校准） |
|---|---|---|
| 0 | `INFRA_ERROR` | HTTP 5xx / empty parse / empty answer |
| 1 | `JUDGE_ERROR` | Judge 调用/解析失败 |
| 2 | `RETRIEVAL_MISS` | gold 存在且 Recall@k = 0 |
| 3 | `SELECTION_MISS` | Recall>0 且 cited∩gold=0 且 correctness 低 |
| 4 | `REFUSAL_WRONG` | refusal.correct_for_expectation = false |
| 5 | `UNGROUNDED` | faithfulness.score < τ_f 且有 unsupported claims |
| 6 | `INCORRECT` | answer_correctness.score < τ_c（答错/偏） |
| 7 | `PARTIAL` | τ_partial ≤ correctness < τ_c 或 verdict=partial |
| 8 | `PASS` | correctness≥τ_c 且 faithfulness≥τ_f 且 refusal OK 且无 infra |

**初值（校准前仅报告不门闩）：**

- `τ_c = 0.7`，`τ_f = 0.7`，partial 区间 `[0.4, 0.7)`  

与旧标签差异：

- 不再有「must 未命中 → GENERATION_UNGROUNDED」。  
- **UNGROUNDED ≠ INCORRECT**：忠实度与正确性拆开（行业标准做法）。

---

## 6. Golden 数据模型 v2

### 6.1 新字段（主）

```json
{
  "query": "…",
  "reference_answer": "Y冷冻设备公司于2019年在大连投资建厂。",
  "expected_should_answer": true,
  "source_chunks": [ { "substring": "…", "doc_hint": "thesis" } ],
  "capabilities": ["rag"],
  "doc_scope_hint": "all",
  "difficulty": "easy",
  "subset": "thesis_factual",
  "rubric_notes": "接受「2019 年」「2019年」；必须指向大连建厂而非南京前身。"
}
```

### 6.2 退役 / 降级字段

| 字段 | v2 态度 |
|---|---|
| `must_include` / `must_not_include` | **不参与评分**；可选保留作文档注释，加载时忽略 |
| `expected_answer` | 重命名/映射为 `reference_answer`（兼容读旧 JSON） |
| `source_chunks` | **保留**（检索层 gold，确定性） |
| `relevance_grades` | 保留给 nDCG |

### 6.3 语料与题目

- 继续用现有 realistic corpus + `golden_set_realistic.json`。  
- 迁移脚本：`expected_answer` → `reference_answer`；去掉评分路径对 must_* 的依赖。  
- 对抗题：`expected_should_answer=false`，`reference_answer` 写「应拒答，语料未记载 X」。

---

## 7. Suite 汇总与 Gate

### 7.1 报告输出

```
e2e_output/rag_eval_v2/{run_id}/
  summary.json          # 均值、label 直方图、子集表
  summary.md            # 人读
  per_query.tsv         # n, subset, labels, scores, query
  qNNN.artifact.json    # chat 产物子集
  qNNN.judge.json       # judge 原始 + 解析
  judge_prompt_version  # git hash or semver
```

### 7.2 硬门闩（分阶段）

**Phase 0（首版上线）——只报告，不因质量分数 fail 测试**

- 基建：HTTP 5xx 率、JUDGE_ERROR 率可 `E2E_FAIL_FAST`  
- 打印：mean correctness / faithfulness / recall@15 / label 分布  

**Phase 1（小集人工校准后）——软门闩**

- 30 题人工 binary label → 与 Flash judge 算 Cohen’s κ（目标 κ≥0.6）  
- 校准 τ_c / τ_f  

**Phase 2（稳定后）——硬门闩建议**

| 指标 | 建议 |
|---|---|
| Recall@15（answerable） | 相对 baseline 降幅 ≤ 3%（沿用 ADR-0011 精神） |
| mean answer_correctness | ≥ 校准后阈值 |
| mean faithfulness | ≥ 校准后阈值 |
| REFUSAL_WRONG 率 | = 0（answerable/adversarial 分层） |
| JUDGE_ERROR 率 | = 0 |

**明确不门闩：** 单题「字符串是否等于 reference」。

---

## 8. 代码布局（新，不贴旧皮）

```
tests/rag_quality/
  src/
    eval_v2/
      mod.rs              # 对外：score_run, ScoreV2, LabelV2
      artifact.rs         # 从 ChatResponse 抽 retrieved/cited/answer
      retrieval.rs        # 从 metrics 抽出的纯检索指标（可 copy 精简，不 import 旧 label）
      judge_client.rs     # ENV → LlmClient (Flash)
      judge_prompt.rs     # versioned prompt strings
      judge_parse.rs      # JSON schema parse
      aggregate.rs        # labels + suite summary
      report.rs           # md/tsv/json
    … 旧 metrics_v2.rs 保留但标注 deprecated for generation gate
```

Product e2e：

```
rag_quality_prod.rs
  if RAG_EVAL_V2 {
    eval_v2::score_and_report(...)
  } else {
    // legacy path (过渡)
  }
```

e2e-analyzer：

- 新 subcommand `rag-eval-v2-drift`（对比两次 summary.json）  
- 旧 `rag-diag` 保留只读旧产物

---

## 9. Runner 集成（realistic_corpus）

1. 每题 chat 照旧，写 artifact。  
2. Layer A 本地算 retrieval/selection/contract。  
3. Layer B 调 Flash judge（可并发上限 2–4，尊重 RPM）。  
4. 写 per-query + 更新 summary。  
5. `E2E_START_AT` / `E2E_END_AT` 行为不变；**一题一停**仍可用。  
6. **不**把 judge 分数写回产品日志；仅 e2e_output。

---

## 10. 与 ADR-0011 的关系

| ADR-0011 | v2 |
|---|---|
| 分轨（检索/选择/生成） | **保留并加强** |
| 检索用 tool_results | **保留** |
| 生成层 substring / must_include 作正确性 | **废弃** |
| 诊断标签优先级 | **重写为分数驱动**（§5） |
| LLM-as-Judge Phase 2 | **升为生成层默认** |

建议后续 ADR-0012：`RAG Eval v2 — Judge-first generation metrics`，状态 Accepted 后标注 0011 生成层门闩为 Superseded。

---

## 11. 风险与缓解

| 风险 | 缓解 |
|---|---|
| Judge 不稳定 | temp=0、prompt 版本化、可选答案哈希缓存、κ 校准 |
| Judge 过严/过松 | 30 题人工集；调 τ 不调产品 |
| 费用 | 仅 Flash；缓存；END_AT 切片 |
| cited 空导致 faithfulness 失真 | retrieved_fallback + context_sufficiency；SELECTION_MISS 分流 |
| 旧 report 对比断裂 | 过渡期双写；明确「v1 label 与 v2 label 不可直接比」 |
| 把 Pro 误配成 judge | `from_env` 默认 flash；启动时 eprintln 实际 model |

---

## 12. 实现切片（建议 PR，非评测探针）

| 切片 | 交付 | 验证 |
|---|---|---|
| **P0** | `eval_v2` 骨架 + JudgeClient ENV + 单题 CLI/单测 mock | `cargo test -p rag_quality` |
| **P1** | 合并 judge prompt + parse + artifact 适配 | 离线 JSON fixture |
| **P2** | 接入 `realistic_corpus_full_eval` + `RAG_EVAL_V2=1` | 单题 Q1 人读 summary |
| **P3** | summary.md/tsv + 缓存 | 10 题切片 |
| **P4** | 校准集 + κ + 阈值建议 | 文档更新 e2e-gates |
| **P5** | ADR-0012 + 默认切 v2 + 旧 generation gate 标注 deprecated | — |

**P0–P2 即可开始替代「must 误杀」的日常诊断**；门闩数字等 P4。

---

## 13. 非目标

- 不在本次改 agent 提示词或 tool_pool（装配已另修）。  
- 不在评测里加「user_context 泄漏检测」等产品回归探针。  
- 不用 must_include 的 normalize 补丁冒充新设计。  
- 不强制每题人工。  

---

## 14. 成功标准

1. Q1 类「2019 年」语义正确题：v2 **correctness 高分 / PASS 或 PARTIAL**，不再因空格 INCORRECT。  
2. 真胡编数字：faithfulness 低 → `UNGROUNDED`。  
3. 真检索失败：Recall=0 → `RETRIEVAL_MISS`（先于 judge 语义）。  
4. 全量报告以 **mean correctness / faithfulness / recall** 为主，**不以** must 命中率为主。  
5. Judge 默认模型日志可见为 **deepseek-v4-flash**。

---

## 15. 附录：单题人读模板（报告用）

```
Q{n} [{subset}] label={LABEL}
  retrieval: recall@15=… hit=…
  selection: prec=… rec=…
  judge: correctness=… (verdict) | faithfulness=… | relevancy=… | refusal=…
  reference: …
  answer: …
  rationale: …
```

---

**下一步（实现前需你点头）：** 按 §12 P0 开工，或先只落 ADR-0012 文本 + 空模块骨架。  
默认假设：实现时 **静默复用** `.env` 的 `MEMORY_LLM_*`（已是 `deepseek-v4-flash`），可用 `JUDGE_LLM_*` 覆盖。
