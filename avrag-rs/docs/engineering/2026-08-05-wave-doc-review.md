# 2026-08-05 波次文档审查（遗漏 / BUG / 缺口 / 漂移）

| 项目 | 内容 |
|------|------|
| 日期 | 2026-08-05 |
| 范围 | 本波 5 篇 engineering 设计 + 已落代码/金标/prompt + 合并 non-PASS 处置 |
| 目的 | 进入实现阶段前的文档一致性闸 |

**被审文档：**

| 文件 | 表头状态 | 代码状态（2026-08-05 全量实现后） |
|------|----------|--------------------------------------|
| `2026-08-05-multi-round-retrieval-context-management-design.md` | 未实现 | **P0 已实现**：bridge 跨轮 `reseen` + body 省略；notes/clearing/working-set 仍后续 |
| `2026-08-05-retrieval-adjacent-shortlist-merge-design.md` | 未实现 | **已实现**：`adjacent_merge_shortlist_longlist` + cursor hydrate；**默认开**（`RETRIEVAL_ADJACENT_MERGE=0` 关） |
| `2026-08-05-eval-v2-retrieval-primary-gate.md` | 已实现 | **已实现** + ScoreV2 持久化 `eval_gate` + rejudge 读旧分 |
| `2026-08-05-eval-v2-correct-ungrounded-label-design.md` | 设计提案 | **已实现** `LabelV2::CorrectUngrounded` |
| `2026-08-05-multi-caliber-table-count-policy.md` | skill+金标已改 | **prompt/golden 已改**；无新 label 码 |

---

## 1. 严重度一览

| ID | 级 | 类型 | 摘要 |
|----|----|------|------|
| R1 | **P0** | 缺口 | 无「波次总图」：实现优先级、依赖、与 non-PASS 映射未集中 |
| R2 | **P0** | 漂移 | `retrieval_primary` 的 **PARTIAL 语义双载**（答案半对 vs 检索不全）未在报表/运营说明里强约束 |
| R3 | **P0** | BUG/缺口 | `rejudge` 固定 `EvalGate::Full`，离线重判 **抹掉** q105 硬闸 |
| R4 | **P1** | 遗漏 | 多轮上下文文 **未记 q141**（假拒主因=多轮证据噪声）作验收锚 |
| R5 | **P1** | 遗漏 | 多轮文 **未记** 已落地的 `MAX_CONSECUTIVE_SANDBOX_ERRORS=4` 与 break 可观测性（相邻但不同轴） |
| R6 | **P1** | 缺口 | `CORRECT_UNGROUNDED` 与 `LabelV2` 枚举/histogram **零接线**；SkillOpt 仍会把 q030 当 UNGROUNDED |
| R7 | **P1** | 缺口 | `ScoreV2` / artifact **不持久化 `eval_gate`** → 仅靠金标+全量跑才正确 |
| R8 | **P1** | 漂移风险 | 邻并默认「可先开」vs 多轮 P0 未排序；**双开**时体积/噪声交互未写 |
| R9 | **P2** | 遗漏 | `docs/README.md` 未链本波 engineering（发现成本） |
| R10 | **P2** | 缺口 | JE×4（062/107/121/123）无文档结论；可接受为 judge 轴 |
| R11 | **P2** | 漂移 | 邻并挂钩 `ScoredChunk`「当前无 cursor」——实现前须再确认字段落点（data-plane vs rag-core） |
| R12 | **P2** | 已对齐 | skillopt `strategies*.md` 与 product **SAME**（多口径/grounding） |
| R13 | **P2** | 语义 | multi-caliber「仅 81 可 partial 或 correct」偏松，与「不合格=仅 57」不对称；依赖 judge 读 rubric |
| R14 | **P2** | 遗漏 | VGRAG 作噪声**放大器**（非主因）只在对话中成立，**未写入**多轮/邻并任一文 |
| R15 | **P2** | 缺口 | 假拒答 gotcha（有命中整题未覆盖）对话里提过，**未确认**是否已写入 `strategies-grounding` 正文（列表漏写 gotcha 有；整题假拒是否同条） |

---

## 2. 分文档

### 2.1 多轮上下文管理

| 项 | 评 |
|----|-----|
| 问题陈述 / 去重边界 | 与现码一致（bridge 新 alias、synthesis 48k snip、seen_aliases 只 summary） |
| Durable vs visible | 清晰；对齐 AGENTS stop-by-model |
| 落地序 P0–P3 | 可用 |
| **缺** | q141 案例；与邻并、VGRAG 放大器关系；sandbox 连续失败阈值（已码 4）交叉引用 |
| **风险** | 实现时若只做 dedupe 不做 clearing，q141 类仍可能假拒 |

### 2.2 邻接 S+L

| 项 | 评 |
|----|-----|
| 机制 / 禁区 | 清楚 |
| cursor hydrate 前置 | 正确标未实现 |
| **缺** | 与多轮 working-set：**邻并增加条数/字数** 后如何与 cap/clearing 共存 |
| **风险** | 文档写「产品默认可先开」——在多轮折叠未上前可能 **加厚** context |

### 2.3 retrieval_primary

| 项 | 评 |
|----|-----|
| 代码路径 | `EvalGate` + `label_for` + golden q105 + finish_score **齐** |
| **BUG** | rejudge 写死 Full → 对 q105 离线重判 label 回退旧规则 |
| **语义债** | `PARTIAL` 在 full=答案半对，在 retrieval_primary=**recall 不全**；histogram 混读会误判 |
| **缺** | artifact 不存 `eval_gate`；means 仍含 AC/FA（文已说明，OK） |

### 2.4 CORRECT_UNGROUNDED

| 项 | 评 |
|----|-----|
| 边界 q030 vs q088 | 清楚 |
| **缺口** | `LabelV2` 无变体；aggregate 无分支；无单测 |
| 与 multi-caliber | 正交声明 OK |

### 2.5 多口径表计数

| 项 | 评 |
|----|-----|
| golden 概念 81/57、验证 59/30 | **已在** `golden_set_realistic.json` |
| skill / skillopt | **已同步** |
| **弱** | 依赖 LLM judge 读 rubric；无确定性 label 分支 |

---

## 3. non-PASS 映射（是否「文档闭环」）

| q | 标签 | 文档/代码归属 | 闭环？ |
|---|------|---------------|--------|
| 017 | PARTIAL | 邻并设计 | 设计 ✅ 实现 ❌ |
| 028 | INFRA | （无专文；infra） | 可接受 |
| 030 | UNGROUNDED | CORRECT_UNGROUNDED | 设计 ✅ 实现 ❌ |
| 062/107/121/123 | JE | 无 | 快扫可补 |
| 074 | PARTIAL | 弱文档 | 可接受/再标 |
| 078/088 表 | 多口径 | 政策+golden+skill | **闭环** |
| 093/099 | 选段/列表 | grounding gotcha | 部分闭环 |
| 105 | PARTIAL | retrieval_primary | **码+金标闭环**（rejudge 除外） |
| 117 | SELECTION_MISS | codegen 阈值/提示（码已 4） | 部分；非专文 |
| 141 | REFUSAL_WRONG | **应归多轮上下文** | 对话有、**文缺锚** |

---

## 4. 建议实现序（审查结论，供下一阶段）

```text
1. 修 rejudge + ScoreV2 可选持久化 eval_gate   （堵 P0 BUG，小）
2. 多轮 P0：跨轮 chunk_id delta body + 饱和收紧 （q141 主杠杆）
3. 邻并 P0：cursor hydrate + S+L               （q017；在 2 的 cap 下开）
4. CORRECT_UNGROUNDED 入 LabelV2               （q030 归因）
5. 报表：PARTIAL 旁注 eval_gate / 检索不全标记
```

**不要**在 2 未上时默认全量开邻并（R8）。

---

## 5. 审查结论

- **可进入下一阶段**，但应先认：**3 设计未实现 + 1 评测闸已实现有 rejudge 洞 + 1 政策已落 prompt/golden**。  
- 最大产品杠杆仍是 **多轮 model-visible 折叠**（q141），不是再写一篇政策。  
- 最大工程小洞是 **rejudge × retrieval_primary**。

本审查文本身应随实现进度改「代码状态」列，避免再漂。
