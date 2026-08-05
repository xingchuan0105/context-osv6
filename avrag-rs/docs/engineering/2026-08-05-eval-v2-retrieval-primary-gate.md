# eval v2：`eval_gate=retrieval_primary`（开放综合 / 双读法）

| 项目 | 内容 |
|------|------|
| 日期 | 2026-08-05 |
| 状态 | **已实现**（`GoldenExample.eval_gate` + `label_for` 分支） |
| 触发题 | full149 q105（跨文档「专注细分市场」相似；双读法均可满分，AC partial 难界定） |
| 相关 | `eval_v2/aggregate.rs` `label_for`；`CORRECT_UNGROUNDED` 设计（正交）；多口径表计数政策 |

---

## 0. 一句话

**开放综合 / 双读法 / 「是否算相似」类题：v2 根因标签的硬闸改为全流 gold 检索覆盖；答案措辞与 judge AC/FA 只作报告分，不决定 PASS/PARTIAL/UNGROUNDED。**

---

## 1. 问题

- 金标写「两种读法均可」+ 分档 partial，**标准论述难自动/人工稳定裁决**。
- 若产品/回归目标是 **检索能力**（该召回的 chunk 不漏），把终答合不合 rubric 绑进同一 PASS 会 **稀释信号**、制造假 non-PASS。
- q105 在 skill-only / VGRAG 下均可 AC partial，但 **全流 recall 可为 1.0**——更应记检索成功，而非写作分。

---

## 2. 字段

`GoldenExample.eval_gate`（`tests/rag_quality/src/golden_set.rs`）：

| 值 | 语义 |
|----|------|
| **`full`**（默认） | 现网 §5 全表：AC / FA / selection / refusal 参与 label |
| **`retrieval_primary`** | 开放综合主闸：全流 `retrieval.recall` |

JSON：

```json
"eval_gate": "retrieval_primary"
```

省略字段 → `full`（旧金标零迁移成本）。

---

## 3. `label_for` 行为（`retrieval_primary`）

优先级：

1. `INFRA_ERROR` / `JUDGE_ERROR`（不变）  
2. **`REFUSAL_WRONG`**（仍硬：该答却拒 / 该拒却答）  
3. 有 gold 且非 `expect_no_retrieval`：  
   - `recall == 0` → **`RETRIEVAL_MISS`**  
   - `0 < recall < 1` → **`PARTIAL`**（此处语义 = **检索覆盖不全**，非答案半对）  
   - `recall == 1` → **`PASS`**  
4. **不**因 AC partial / incorrect / FA unsupported / cited∩gold=0 改 label  

Judge 输出仍写入 `ScoreV2.judge` 与 means（correctness / faithfulness 均值**仍含**该题，便于观察写作质量）；**label histogram 与「是否算 non-PASS」以 gate 为准**。

### 与 `full` 的差异摘要

| 维度 | full | retrieval_primary |
|------|------|-------------------|
| recall=0 且答对 | 可 PASS（防 markitdown 误伤） | **RETRIEVAL_MISS** |
| AC partial、FA 低 | PARTIAL / UNGROUNDED | **忽略**（report only） |
| SELECTION_MISS | 可触发 | **不触发** |
| 跨文档 1/2 gold | 答案层 PARTIAL 常见 | **检索 PARTIAL** |

---

## 4. 适用题型（打标准则）

**宜 `retrieval_primary`：**

- 金标含「两种读法均可」/ 开放对比「相似/异同」且无唯一硬事实句  
- 主目标明确为 **多源 chunk 是否进工作集**  
- 终答标准依赖间接归纳、修辞同构、贬义/正面语境辩论  

**仍应用 `full`：**

- 数字 / 实体 / 表计数（可核对）  
- 拒答题  
- 需严格 grounding 的 fact 题（编造不可放行）  
- 选段即产品契约的 SELECTION 题  

首批金标：**q105 跨文档「专注细分市场」**。

---

## 5. 实现挂点

| 位置 | 变更 |
|------|------|
| `golden_set.rs` | `EvalGate` + 字段 |
| `eval_v2/aggregate.rs` | `LabelInput.eval_gate` + 分支 + 单测 |
| `rag_quality_prod.rs` `finish_score` | 传入 `example.eval_gate` |
| `rejudge` bin | **缺口（P0）**：当前写死 `EvalGate::Full`，离线重判会**抹掉**本闸；应 load 金标 query→`eval_gate` 或把 gate 写入 `ScoreV2`/artifact |
| `golden_set_realistic.json` | 该例 `"eval_gate": "retrieval_primary"` |
| `ScoreV2` | **未**持久化 `eval_gate`（仅跑全量时从 golden 注入 `label_for`） |

### 语义债：`PARTIAL` 双载

| `eval_gate` | `PARTIAL` 含义 |
|-------------|----------------|
| `full` | 答案半对 / judge partial verdict |
| `retrieval_primary` | **全流 gold 覆盖不全**（`0 < recall < 1`），与答案措辞无关 |

报表/人工读 histogram 时须带 gate；实现阶段建议 TSV 增加 `eval_gate` 列或 `partial_kind`。

---

## 6. 报告与运营

- 解读 non-PASS 时：若 `eval_gate=retrieval_primary` 且 label=PARTIAL → **先查 `first_hit_ranks` / recall**，勿开 skill 写作优化。  
- PASS 不表示终答「范文级」；需要写作质量时看 **AC/FA 报表列**，或另开人工抽检。  
- 与 **CORRECT_UNGROUNDED** 正交：后者是 full 闸下「答对无路径」；本闸是「路径/召回优先、答不裁决」。

---

## 7. 非目标

- 不修改 judge prompt 模板（仍可出 dual-read 分档，供观察）。  
- 不把 recall@k 作为硬闸（全流 recall 与 agent 多轮一致；@k 仍为诊断）。  
- 不自动推断 `eval_gate`（须金标显式，防 silent 降标）。

---

## 8. 验收

- 单测：partial AC + recall=1 → PASS；recall=0 → RETRIEVAL_MISS；recall=0.5 → PARTIAL；拒答仍 REFUSAL_WRONG。  
- 金标反序列化 `retrieval_primary`。  
- 下一 full149：q105 在全流双金命中时应标 **PASS**（即使 AC=0.7 partial）。
