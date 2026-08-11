# 质量门禁对照表 — 单 agent / three-loop → Lead+Workers

**状态：** living（与 `AGENTS.md` agent-lane、产品 YAML 同步；有变更时改本表）  
**日期：** 2026-08-11  
**权威交叉引用：**

| 文档 | 角色 |
|------|------|
| 根 `AGENTS.md`（Lead+Workers / 无独立 verify） | 产品法则 |
| `docs/plans/2026-08-11-lead-rag-web-workers-design.md` | Lead+Workers 设计 |
| `avrag-rs/docs/engineering/2026-08-07-retrieve-synthesis-verify-loop-design.md` | 历史 three-loop 设计 |
| `avrag-rs/docs/e2e-gates.md` | 离线 E2E / full-149 / eval_v2 |
| `docs/engineering/2026-08-10-harness-llm-user-channel-philosophy-diagnosis.md` §17 | 用户信道 / verify 面哲学 |

---

## 0. 读表约定

| 列 | 含义 |
|----|------|
| **旧（单 agent / SaC 主路径）** | 2026-07～08 上旬：单脑 retrieve→synthesis（±verify）；检索常为 SaC 或 dual 混用 |
| **新（Lead+Workers）** | 现行 agent-lane：Lead plan → RAG/Web Worker → Pack → Lead 合成 |
| **落点** | Host / RAG Worker / Web Worker / Lead / 离线 harness |
| **产品默认** | `modes/rag.yaml` · `search.yaml` · `chat.yaml`（2026-08-11） |

**不是：** 「把旧质量门禁整体迁到 RAG Worker」。  
**是：** 证据结构门下沉到 Worker 出口；语义裁决上移 Lead；环内 verify 产品关闭；答案级评分仍在环外 eval。

---

## 1. 总览：谁在把关

```
                    ┌──────────────────────────────────────┐
                    │ 离线 harness（full-149 / eval_v2）      │
                    │ 答案 + tool_results 整链打分            │
                    └──────────────────▲───────────────────┘
                                       │ 不进产品环
┌──────────────────────────────────────┴───────────────────┐
│ 产品在线环                                                │
│                                                          │
│  Lead plan ──► RAG Worker ──► PackGate ──┐               │
│              └► Web Worker ──► PackGate ─┼─► rebrief≤1   │
│                                          ▼               │
│                                   Lead synthesis         │
│                                   + final_check（格式）    │
│                                   + [verify YAML 关]     │
└──────────────────────────────────────────────────────────┘
```

| 角色 | 可做 | 不可做 |
|------|------|--------|
| **RAG Worker** | 短程 SaC 检索 → `evidence_pack_v1` | 用户终答；verify；judge；citation 精度门 |
| **Web Worker** | host 多 query 搜索 + CRW → pack | 同上 |
| **Host PackGate** | 结构重写/降级 pack | 语义「覆盖够了」；拒答句；entity checklist |
| **Lead** | 合成用户答案；读 pack 覆盖度做裁决 | 把 Worker 当终答通道 |
| **Host 出站** | 格式闸（code-only / 协议壳） | 把 verify 失败脚注拼进主气泡 |
| **离线 eval** | recall / citation / judge / 标签 | 改写在线环策略（除非改代码/YAML） |

---

## 2. 主对照表（旧机制 → 新落点）

### 2.1 环内：检索与证据

| # | 旧机制（单 agent） | 意图 | 新落点 | 产品默认 | 观测位置 |
|---|-------------------|------|--------|----------|----------|
| R1 | SaC / dense 多轮 retrieve | 找材料 | **RAG Worker** 短程 SaC；失败/空 → host dense leaf 装 pack | 开 | `tool_results` / `tool_trace`；Worker 不暴露独立 exit |
| R2 | dual 单脑 KB∪web 混用 | 双通道 | **拆通道**：RAG Worker ∥ Web Worker；Lead 并包 | 开（dual caps） | `mode_debug.general.lead_workers.channels[]` |
| R3 | `require_evidence` + host 空证据 continue | 无 Ok 检索不交合成 | YAML **`require_evidence: false`**；结构 Ok 计数仍可存在于 SaC 子环，**Lead 路径不以「语义够不够」拒合成** | 关语义 veto | 旧 observation 标签仍可能出现在 SaC 内文，非产品终局 |
| R4 | `evidence_missing_continue` / `required_action_missing_continue` | 结构缺动作再跑一轮 | **非 Lead 主路径语义**；Lead 用 **PackGate + rebrief≤1** 替代「再跑 retrieve 直到够」 | rebrief 开（≤1） | `lead_workers.rebrief_used`、`pack_gate` |
| R5 | query-card `required_actions` | L0 必做原语 | 代码仍在；Lead 主检索由 **plan briefs** 驱动 | 视路径 | `mode_debug.general.query_card` |
| R6 | knockout / EWS | 证据去重/工作集 | 仍在环内（合成/SaC 相关）；**非** Worker 质量分 | 开（实现向） | `mode_debug.general.knockout` / `ews` |
| R7 | （无等价） | Worker→Lead 契约 | **`apply_pack_gate`**（见 §3） | 开 | `lead_workers` Evaluation signals |
| R8 | （弱）coverage 自报 | 模型说够了 | Pack 上 coverage + **host 降级**；**Lead** 读 gaps 写答 | 开 | pack `coverage` / `gaps` |

### 2.2 环内：合成与用户答案

| # | 旧机制 | 意图 | 新落点 | 产品默认 | 观测位置 |
|---|--------|------|--------|----------|----------|
| S1 | Synthesis prose / JSON contract | 用户可见答 | **Lead 合成**（`prose_only`） | 开 | answer + usage |
| S2 | **verify LLM**（pass/fail + route） | 答案×证据裁决 | 代码：`react_loop/verify.rs` + `mod.rs` 环；**YAML `verify: false`** | **关** | `mode_debug.general.verify` 多为 bypass |
| S3 | verify fail → resynthesis / rereretrieve / ceiling | 质量环再入 | 仅 `verify: true` 时；产品默认不进 | 关 | verify calls / ceiling |
| S4 | **final_check**（code-only / 协议壳） | 不出协议残片 | **仍在 Lead 合成**（`synthesis.rs` repair/rerender/disaster） | 开 | `activity_counts` `final_check:*` |
| S5 | `forbid_retrieve_direct_answer` | retrieve 不直出终答 | YAML **true**（rag/search）；Worker 也不写用户气泡 | 开 | 无 DirectAnswer 用户交付 |
| S6 | 软拒答 / 多实体 host 扫描 | host 当质检 | **不做**（法则：Host 非语义 veto） | — | 无 |
| S7 | 用户气泡脚注（ceiling 披露等） | 系统说话 | **禁止**（harness 不进主气泡） | — | telemetry only |

### 2.3 环外：评测 / 发布门

| # | 机制 | 意图 | 落点 | 门禁硬度 | 工件 |
|---|------|------|------|----------|------|
| E1 | Recall@15 等 retrieval 指标 | 检索是否捞到金标 | harness 从 **整 run** `tool_results` 抽 | release 子集硬闸；full-149 常作报告 | qNNN / v2 |
| E2 | Citation accuracy / precision | 引用对不对 | harness 对 answer + citations | 多报告；部分 release 硬 | 同上 |
| E3 | Halluc / substring faithfulness | 硬幻觉 | harness | 报告为主 | 同上 |
| E4 | **eval_v2** LLM-as-Judge | AC / FA / AR | harness 默认开 | soft mean（τ≈0.70）；不挡单题 | `rag_eval_v2/{run_id}/` |
| E5 | Label 体系（PASS / RETRIEVAL_MISS / …） | 归因 | harness | 诊断 | per_query.tsv |
| E6 | Circuit breaker 连续非 PASS | 系统性坏了早停 | full-149 | `E2E_ABORT_AFTER_CONSECUTIVE_FAILS` | 日志 |
| E7 | 能力开关 | 题型通道 | 黄金集 `capabilities[]` | 启动 assert rag/web/dual > 0 | 日志 capability modes |

**要点：** E\* **不**在 RAG Worker 内执行；对象始终是 **Lead 交付后的整链输出**。

---

## 3. PackGate 细则（Worker 出口唯一「结构质量门」）

实现：`agent-loop/src/lead_workers/evidence_pack.rs` · `apply_pack_gate`。

| 检查 | 动作 | 对应旧世界 |
|------|------|------------|
| `schema_version != evidence_pack_v1` | **Reject** → insufficient | （新）契约 |
| `channel` ≠ 期望 rag/web | **Reject** | （新）通道隔离 |
| `tool_ok_count` | **主机重写**（不信模型） | 旧 Ok 计数结构闸的精神 |
| evidence 空 source/content | drop | 防脏证据 |
| evidence 全空 | coverage → **insufficient** | 旧「无证据」 |
| tool_ok=0 且自称 sufficient | → **insufficient** | 防吹牛 |
| key_facts 空且 sufficient | → **partial** | 防空壳 sufficient |
| tool_ok=0 仍有 evidence | coverage 上限 **partial** | 弱化无 Ok 成功叙事 |

**Outcome：** `Accept` | `Downgraded{reasons}` | `Reject{reason}`。  
**不是：** 答案正确性、faithfulness、citation precision。

**Rebrief（host，≤1 波）：** 仅对 **已跑且 empty/insufficient** 的通道再 brief；Lead 故意未开的通道不补。

---

## 4. verify 环：代码 vs 产品

| 项 | 状态 |
|----|------|
| 代码路径 | `react_loop/mod.rs` synthesis 后 `should_run_verify` → `run_verify` |
| 产品 YAML | `rag` / `search` / `chat`：**`verify: false`** |
| Worker | 强制 `worker_mode.loop_exit.verify = false` |
| 打开代价 | 额外 LLM 轮 + fail 再入 retrieve/synthesis；与 §17 用户信道哲学冲突风险 |
| 观测 | `mode_debug.general.verify`（关时 bypass_reason） |

**对照：** 旧 three-loop 的「质量闭环」产品侧 **已关**；**未**迁到 Worker。

---

## 5. final_check（格式闸）对照

| 阶段 | 行为 | 谁 |
|------|------|-----|
| 检出 code-only / 协议壳 | `check_final_answer` | Host（合成后） |
| repair → rerender | 最多再合成 | Lead 合成路径 |
| 仍失败 | disaster 文案（有证据/无证据分叉） | Host 模板，非 verify 脚注 |
| Worker 是否跑 | **否** | — |

旧名「质量」里的 **协议洁净** 仍在线；**语义正确** 不在此闸。

---

## 6. 观测字段速查（full-149 / 调试）

| 字段 | 回答的问题 |
|------|------------|
| `mode_debug.general.lead_workers` | 几包？rebrief？每通道 coverage？ |
| `loop_rounds.action_types` 含 `lead_workers` | 是否走 Lead 路径（非旧 code_gen 主路径） |
| `budget_used` | 产品轮次墙实际 current/max |
| `tool_trace` | Worker 留下的 dense/web 等 |
| `activity_counts.final_check:*` | 格式闸是否触发 |
| `verify` | 产品关时期望空/bypass |
| eval_v2 `score_v2` / label | 环外答案质量 |

---

## 7. 「迁到 RAG Worker 了吗」一句话矩阵

| 旧能力簇 | 迁到 RAG Worker？ | 实际归属 |
|----------|-------------------|----------|
| 检索执行 | **部分**：SaC 短程在 Worker | RAG Worker |
| 证据结构合法性 | **新门** PackGate 在 Worker 出口 | Host @ Worker 边界 |
| 答案语义质量（verify） | **否**（产品关） | 代码在 Lead 后；默认不跑 |
| 格式/协议洁净 | **否** | Lead 合成 final_check |
| 覆盖度裁决（硬答/拒答措辞） | **否** | Lead 合成 + prompt |
| 离线 recall/judge | **否** | harness 整链 |

---

## 8. 变更时维护清单

改下列任一处时更新本表：

1. `modes/*.yaml` 的 `verify` / `require_evidence` / `forbid_retrieve_direct_answer`
2. `apply_pack_gate` 规则或 rebrief 策略
3. `synthesis` final_check 规则
4. eval_v2 门禁强度（report → soft → hard）
5. AGENTS.md agent-lane 质量叙事

---

## 9. 相关代码索引

| 主题 | 路径 |
|------|------|
| PackGate | `avrag-rs/crates/agent-loop/src/lead_workers/evidence_pack.rs` |
| Lead retrieve | `…/react_loop/run_lead_workers.rs` |
| verify 环 | `…/react_loop/verify.rs` · `…/react_loop/mod.rs` |
| final_check | `…/react_loop/synthesis.rs` · `answer_contract` |
| 产品 YAML | `avrag-rs/modes/rag.yaml` · `search.yaml` |
| mode_debug 镜像 | `avrag-rs/crates/app-chat/src/chat/pipeline_steps.rs` |
| full-149 | `…/product_e2e/llm_real/rag_quality_prod.rs` · `scripts/test-full149.sh` |
