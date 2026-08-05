# eval v2 标签：CORRECT_UNGROUNDED（结果合格、路径不合 RAG 规范）

| 项目 | 内容 |
|------|------|
| 日期 | 2026-08-05 |
| 状态 | **已实现**（`LabelV2::CorrectUngrounded`：AC ok + FA 低 + recall==0） |
| 触发题 | full149 q030（1T/H×8h×350d→2800）：AC=1、FA=0、仅 calculator、现标 UNGROUNDED |
| 相关 | `eval_v2/aggregate.rs` `label_for`；`eval_bridge_miss`；q028 INFRA 空答（不归本标签）；开放综合硬闸见 `2026-08-05-eval-v2-retrieval-primary-gate.md`（正交） |

---

## 0. 一句话

**答案在正确性维度已过阈值，但忠实度/证据链未过（通常无检索 context 或无检索类 tool），与「数字/事实答错」区分开，单独打 `CORRECT_UNGROUNDED`（名可再议）。**

---

## 1. 问题

当前 `label_for` 在 AC 已高时：

- **不会** 因 `recall==0` 打 `RETRIEVAL_MISS`（刻意避免答对误伤）；
- 但 FA 低 + unsupported claims → **`UNGROUNDED`**。

`UNGROUNDED` 在运营语义上常被读成「瞎编/答错」。  
q030 类实际是：

| 维度 | 状态 |
|------|------|
| 有没有答 | 有 |
| 对不对（相对 gold） | 对 |
| 是否按 RAG 路径（检索 observation） | 否 |

与「算错 2800」「胡编均价」混在同一桶，不利于 SkillOpt 残差归因和产品排期。

---

## 2. 新标签（提案）

### 2.1 名称

| 候选 | 说明 |
|------|------|
| **`CORRECT_UNGROUNDED`** | 推荐：结果 correct + 证据 ungrounded |
| `PATH_NONCOMPLIANT` | 强调路径，弱化「对」 |
| `ANSWER_OK_NO_EVIDENCE` | 口语化，偏长 |

**推荐实现 id：`CORRECT_UNGROUNDED`**（histogram / artifact 稳定字符串）。

中文说明（报告用，非注入模型）：**结果合格、证据链不合 RAG 规范**。

### 2.2 语义边界

| 标签 | 答案质量 | 证据/路径 | 典型 |
|------|----------|-----------|------|
| **PASS** | ≥τ_c 且非 partial | FA 达标（或 FA N/A） | 正常 grounded |
| **PARTIAL** | 半对 / 漏 claim | 可有可无证据 | q017 漏第四点 |
| **UNGROUNDED** | 常错或对但夹杂无据编造 | FA 低 + unsupported | 主数错；或对了又编辅数（q088） |
| **CORRECT_UNGROUNDED** | **≥τ_c，且非 partial/incorrect** | FA 低，**主因无检索 context / 无检索 tool** | **q030** |
| **RETRIEVAL_MISS** | 答案未过 τ_c | recall=0 | 真没检到又答不好 |
| **INFRA_ERROR** | 无有效答 | — | q028 empty |

### 2.3 判定条件（草案）

在现有 `label_for` 中，于 **REFUSAL_WRONG 之后、现 UNGROUNDED 之前**（或替换部分 UNGROUNDED 分支）：

**进入 `CORRECT_UNGROUNDED` 当且仅当：**

1. 非 INFRA / 非 JUDGE_ERROR；  
2. Judge Ok 且解析成功；  
3. **正确性过线**：`correctness >= τ_c` 且 verdict 不是 `partial` / `incorrect` /（按现规则）不是仅 NA 拒答误用；  
4. **忠实度可评且未过线**：`faithfulness_applicable` 且 `FA < τ_f`；  
5. **路径/证据形态**满足至少一条（实现可先只做 5a+5b）：  
   - **5a** `context_source` 为空检索类（如 `retrieved_fallback` 且 retrieved 空）或 `retrieval_recall == 0`；  
   - **5b** 无任何 Ok 的检索类 tool（与 `eval_bridge_miss` 的 tool 集合一致）；  
   - **5c（可选收紧）** gold 未标 `expect_no_retrieval`，且 `expected_should_answer`（确认为 RAG 应答题）。

**不进入本标签（仍 UNGROUNDED 或其它）：**

- AC 过线，但 FA 低是因为 **context 非空仍有 unsupported 辅数/编造**（q088 类）→ 保持 **UNGROUNDED**；  
- AC 不过 → PARTIAL / INCORRECT / SELECTION_MISS 等；  
- empty / 5xx → INFRA。

> 粗分：**「没证据」且「主答全对」→ CORRECT_UNGROUNDED；「有证据仍编」→ UNGROUNDED。**

### 2.4 与 `eval_bridge_miss` 的关系

| | |
|--|--|
| bridge_miss | e2e **契约 Failures 行**（rag 能力下缺检索 tool） |
| CORRECT_UNGROUNDED | **v2 质量直方图标签** |

两者可同时为真（q030）。  
**不**用 bridge 替代 v2 标签；标签用于 suite 统计，bridge 用于契约回归。  
后续可选：bridge 失败且 CORRECT_UNGROUNDED 时 Failures 文案改为 `path_noncompliant`（P1）。

### 2.5 优先级（插入 `label_for`）

```text
INFRA → JUDGE_ERROR
  → RETRIEVAL_MISS（recall=0 且答案未过 τ_c）
  → SELECTION_MISS
  → REFUSAL_WRONG
  → CORRECT_UNGROUNDED（新：答过线 + FA 不过 + 无证据/无检索 tool）
  → UNGROUNDED（FA 不过 + unsupported，且非上条）
  → INCORRECT → PARTIAL → PASS
```

---

## 3. 报告与 SkillOpt 用法

| 用途 | 做法 |
|------|------|
| suite histogram | 单独一列 `CORRECT_UNGROUNDED` |
| 「真质量」粗看 | PASS + CORRECT_UNGROUNDED ≈ 结果合格池（若产品接受路径另计） |
| Skill 训练 | **默认不**把本类当「事实错误」负样本；可作 **路径/路由** 专用 spoke 正负样本 |
| 产品门 | Phase0 仍 report-only；若将来 hard gate，本类是否计 fail **单独产品决策** |

---

## 4. 实现落点（未开工）

1. `LabelV2` 增枚举 + serde 字符串 `CORRECT_UNGROUNDED`  
2. `label_for` 按 §2.3–2.5  
3. 单测：仿 q030（AC=1, FA=0, empty retrieved, unsupported claims）→ 新标签；仿 q088（AC 高但 FA 低且有 context 编造）→ 仍 UNGROUNDED  
4. 文档：`metrics_v2` / e2e-gates 直方图说明补一行  
5. **不改** 宿主 ReAct 停答策略（AGENTS：停决策归 model+skill）

---

## 5. 决策（本对话）

| # | 议题 | 决定 |
|---|------|------|
| D1 | 是否单独标签 | **要**，与真答错 / 真编造拆开 |
| D2 | 推荐名 | `CORRECT_UNGROUNDED` |
| D3 | 典型题 | q030 类：题干可算或 AC 满但无检索证据 |
| D4 | 实现 | 先设计本文；实现另开 |

---

## 6. 一句话

**不是放行成 PASS，而是换一个更诚实的失败类型：结果合格、RAG 路径/证据不合格。**
