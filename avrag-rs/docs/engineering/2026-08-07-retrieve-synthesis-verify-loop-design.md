# 检索 → 合成 → verify 三环编排

| 项目 | 内容 |
|------|------|
| 日期 | 2026-08-07 |
| 状态 | **V1.4 产品路径已接通**：`assemble_mode` / `apply_single_agent_loop_exit` 继承 YAML 三环开关；见修订记录 |
| 动机 | 现状检索轮可 `DirectAnswer` 交卷、合成可选、无答案×证据裁决与回环；口径错选/漏点等残差缺少「写完再判、按错因退回」路径 |
| 非目标 | verify 内检索或代写终答；host 对照 golden；无限回环；每题长 ReAct critic |
| 相关 | 现行 `ReActLoop`（retrieve → optional synthesis）；`query_card` / 结构证据门；`check_final_answer` 格式闸；`AGENTS.md` stop-decision（落地时需与本编排对齐，见 §7） |
| 评测锚 | full149 类：多口径数字（如 q026）、读证误读（如 q074）、列表不全（如 q053）；天气/纯计算旁路见 §5 |

---

## 0. 一句话

**检索只找料且出环硬门；合成只写/改答；verify 只裁决+开方（不检索、不写答）；不合格带意见回合成或回检索，最多 3 次，到顶交卷并降级说明。**

---

## 1. 总览

```text
┌──────────── 检索 loop ────────────┐
│  只找料，禁止交卷（禁止 DirectAnswer） │
│  出环：硬门（题卡 + 质检流水线）        │
└───────────────┬───────────────────┘
                ▼
┌──────────── 合成 loop ────────────┐
│  根据材料写答案 / 按意见改答案          │
└───────────────┬───────────────────┘
                ▼
┌──────────── verify ─────────────┐
│  verify skill；只裁决 + 意见        │
│  不做检索、不写终答                   │
│  · 通过 → 交用户                     │
│  · 不合格 → 回合成（回答纠正意见）     │
│           或 回检索（检索纠正意见）     │
│  不合格累计 ≤ 3；到顶 → 交卷 + 降级说明 │
└───────────────────────────────────┘
```

| 环 | 只做什么 | 不做 |
|----|----------|------|
| **检索** | 调工具/codegen 找料；出环过硬门 | 不向用户交终答 |
| **合成** | 写终稿、按 verify 意见改稿 | 不替代 verify 裁决；出检索硬门不在这里重复发明 |
| **verify** | 通过 / 不合格；不合格时去向 + 纠正意见 | **不检索、不写/改用户终答** |

---

## 2. 检索 loop

### 2.1 职责

- 多轮找料（dense / lexical / grep / web 等，按 mode 与 skill）。
- 模型在本环 **不得** 以用户可见终答收工。
- 离开本环的唯一业务出口：材料就绪 → **进入合成**（或预算耗尽走 §6 降级，仍不经「检索直答」交卷）。

### 2.2 出检索硬门

在 **进入合成之前** 必须通过（host 结构闸，非语义「答得对不对」）：

| 门 | 含义 |
|----|------|
| **题卡** | `query_card.required_actions`：声明的必做动作须有 Ok 回传（与现 `required_action` 语义一致，**挂载点改为出检索**，而非「DirectAnswer 接受点」） |
| **质检流水线** | 结构类：如挂载了检索能力却零 Ok 回传则不得进合成（对齐现 evidence 结构门意图）；形态类可复用/前移现有格式检测中与「工作草稿」相关的部分 |
| **旁路** | 见 §5（计算卡、天气 tool 成功等产品例外） |

硬门失败：继续检索（在轮次/token 预算内）或预算尽 → §6，**不**因语义覆盖不足由 host 臆断。

### 2.3 与现状差距

- 现状：检索可 `DirectAnswer` + `skip_synthesis_on_direct_answer` 跳过合成。
- 目标：**RAG/需 grounding 的 mode 关闭检索直答**；合成必经（旁路除外）。

---

## 3. 合成 loop

### 3.1 职责

- **首入**：据检索材料写用户可见答案。
- **再入**（来自 verify）：据 **回答纠正意见** 改稿；默认 **不再开检索**（补料只能经 verify 判去检索后重走检索环）。

### 3.2 出口

- 产出一版终稿 → 进入verify。
- 格式壳（纯代码、host 标签泄漏等）：可在进 verify 前做 **一轮** 格式修（现有 `check_final_answer` / prose-repair 思路），**不占用** §4.3 的 3 次不合格额度（建议）。

---

## 4. verify

### 4.1 形态

- **不是** 与主检索同级的长 ReAct 找料环。
- **是**：**一拍裁决**（实现上可为单次 LLM + verify skill；输入题面、终稿、只读证据视图/`claim_notes` 等）。
- 挂 **verify skill**（文案在 `prompts/`，第三人称观察语气；禁止命令式「必须改成某某」作为唯一政策——意见陈述事实与张力，合成/检索自行决定怎么改）。
- **核对维度显式化**（skill）：忠实 / 无未化解冲突 / 题面覆盖 / 内部自洽 / 无关键捏造；合理改写与一步直接推论可与证据一致，无锚点关键数字/实体为未支持。
- **形态 few-shot**（skill 末尾）：pass、fail→synthesis、fail→retrieve 各一；抽象题面，**禁止** golden 实体泄漏。
- **边界默认**：证据空或关键主张不可核对 → fail+retrieve；终稿空 → fail+synthesis。

### 4.2 明确禁止

| 禁止 | 原因 |
|------|------|
| 调用检索/web/codegen 找料 | 补料只回 **检索 loop** |
| 撰写或直接替换用户终答 | 改答只回 **合成 loop** |
| 对照 golden / 评测标准答案 | 防泄漏；只做题面×终稿×证据 |

### 4.3 输出信号（仅两类主结果）

Host 只认结构化主出口（字段名实现可定，语义固定）：

```text
通过
  → 交付用户（当前合成终稿）

不合格
  · route: synthesis | retrieve
  · advice: 纠正意见（第三人称、可执行）
       route=synthesis → 回答纠正意见（口径、漏槽、内部矛盾等）
       route=retrieve  → 检索纠正意见（缺什么料、建议补什么方向）
```

- **通过 / 不合格** 为唯一业务分叉；细节标签可进 telemetry，不充当第二套主协议。
- 意见应 **可执行**：指出终稿中的具体句子/数字/列表与证据摘录中的张力位置，避免空泛「再想想」。

### 4.4 回环上限与到顶

| 规则 | 约定 |
|------|------|
| 计数 | 每次 **不合格** 计 1（回合成或回检索均计） |
| 上限 | **3** |
| 第 1～3 次不合格 | 按 `route` + `advice` 进入对应 loop，再合成后再 verify |
| 已满 3 次仍不合格 | **不再回环**；**交卷**当前合成稿 + **降级说明** |
| 通过 | 交卷；本题 verify 回环结束 |

**降级说明**（到顶时）：

- 用户可见：短、第三人称，说明自动核对已达次数上限，部分要点可能仍不完整或与材料存在张力（具体文案进 `prompts/loop/`，禁止写死在 Rust 长文）。
- 内部：完整 verify report 进 telemetry / 评测旁路。

### 4.5 verify 输入（最小集）

- 用户问题  
- 当前合成终稿  
- 只读证据视图（pool / 可见摘录 / claim 板等已有积木）  
- 可选：query_card 类型、mode  
- **不含** golden  

---

## 5. 旁路（跳过或弱化 verify / 检索硬门）

产品例外，避免无意义烧预算：

| 场景 | 建议 |
|------|------|
| 题卡 calculation / P-calc-ok 类 | 可弱化检索硬门；verify 极简或跳过 |
| 天气等：约定 **tool 成功即过** | 有 **Ok `weather_query`** 时跳过 verify（实现：`weather_tool_ok`）；不对气温陈述做忠实度裁决 |
| 明确 expect_no_retrieval | 按现合同，不要求检索硬门 |
| 用户取消 | 立即停，无强制 verify |

具体名单落地时与 mode / skill 表对齐，本文只定原则：**旁路显式登记，默认走满三环。**

---

## 6. 预算与取消

- 检索 / 合成 / verify **共享** 产品 token·轮次预算；回环次数 **3** 是 verify 不合格专用上限，不替代全局预算。
- **产品策略（2026-08-10）：** 当 `verify: true` 且非旁路时，**至少跑一次 verify**——不得因「首遍合成后 token 已顶格」跳过裁决。顶格只限制 **fail 后的 re-entry**。
- **用户通道（2026-08-10 信道哲学）：** `DeliverCeiling` **不再**拼接 ceiling / evidence disclosure 脚注。轮次/失败次数到顶且仍有 token → 一次 LLM `user_facing_closeout` 人话收束；token 预算尽 → `finalize_delivery_without_llm`（合法 prose 直出，非法格式 → `prompts/loop/disaster/*`）。见 `docs/engineering/2026-08-10-harness-llm-user-channel-philosophy-diagnosis.md` §17。
- 全局预算尽：优先 **交卷当前最佳稿 + 披露**（可与到顶降级说明合并策略），禁止静默死循环。
- 取消：与现 loop 一致，尽快退出。

---

## 7. 与现行策略 / 代码的关系

### 7.1 策略

现行 `AGENTS.md` 强调：是否 stop 主要归模型 + skill；host 不做语义「覆盖够了」拒答。  
**本设计是编排升级**：

- 检索模型 **无交卷权**（对适用 mode）；
- **交付权**在verify「通过」或「到顶强制交卷」；
- host 做 **状态机 + 硬结构门 + 信号路由**，语义对错仍由 verify skill（模型）陈述，host 不读 golden。

落地前需在 `AGENTS.md` / product 文档增加本三环为权威路径说明，避免与「retrieve 内 DirectAnswer」旧叙述并存。

### 7.2 现状缺口（实现对照）

| 目标 | 现状 |
|------|------|
| 检索禁止直答 | 可 DirectAnswer 并 skip synthesis |
| 必经合成 | 合成可选 |
| verify + skill | **无** |
| 通过 / 不合格 + 双去向意见 | **无** |
| 回环 ≤3 + 到顶降级 | **无** |
| 出检索硬门（题卡+质检） | 门散在 DirectAnswer 接受点等，且不完整 |

可复用零件：检索工具链、observation、`query_card`、结构 evidence 门、合成阶段、`check_final_answer` 格式闸、`claim_notes` / evidence pool、loop prompt 加载与 `host_markers`。

### 7.3 建议实现顺序（非本文件范围，仅索引）

1. 禁适用 mode 的检索 `DirectAnswer`；强制进合成。  
2. 题卡 + 结构门挂到 **出检索**。  
3. verify skill + 信号 schema + 短裁决调用。  
4. 状态机：不合格路由合成/检索；计数 3；到顶交卷 + `prompts/loop` 降级说明。  
5. 旁路表与单测（通过、回合成、回检索、到顶、旁路跳过）。

---

## 8. 信号与文案约定

| 项 | 约定 |
|----|------|
| LLM 面向指令 | 仅 `avrag-rs/prompts/**`（verify skill、loop 降级说明、纠正意见包装 observation） |
| 观察语气 | 第三人称「发生了什么 / 存在什么张力」，非命令清单（与仓库 prompts 纪律一致） |
| 新 host 标签 | 若回灌模型须先登记 `host_markers.rs` |
| Telemetry | 记录 route、advice 摘要、回环次数、是否到顶降级，供 full149 诊断 |

---

## 9. 验收标准（设计层）

- [ ] 适用 mode 下，无「检索直答交卷」路径。  
- [ ] 每条成功用户答案均经合成稿；经 verify 通过或到顶降级二者之一。  
- [ ] verify 实现与 skill **无** 检索工具、**无** 直接产出替换终答的主路径。  
- [ ] 不合格信号必含 `route` ∈ {synthesis, retrieve} 与非空可执行 `advice`（测试可构造）。  
- [ ] 第 4 次本应不合格时强制交卷且带降级说明；无无限循环。  
- [ ] 登记旁路题型不误伤（计算/天气等）。

---

## 10. 修订记录

| 日期 | 说明 |
|------|------|
| 2026-08-07 | 初稿：三环、verify（仅裁决+意见）、回环 3、到顶交卷降级；与会话定案一致 |
| 2026-08-07 | **V1 实现**：`LoopExitConfig::{forbid_retrieve_direct_answer,verify,verify_max_fail_rounds}`；rag/search YAML 开启；`content_dispatch` 散文 → `BreakToSynthesis`（worker_handoff 除外）；`verify` 模块 + `prompts/clusters/verify` + loop 观察；`ReActLoop::run` 合成后 verify 回环（合成/检索） |
| 2026-08-07 | **V1.1**：`AGENTS.md` 三环 stop 表；`weather_query` Ok 旁路 verify；`follow_up_after_verify_fail` 纯函数 + 单测（fail→合成/检索观察、第 4 次到顶） |
| 2026-08-07 | **V1.2 review 修复 1–5**：回检索共享 prior token + 最多 2 轮；verify 前不 stream/Done；仅 **weather-only** Ok 旁路；证据含 code_execution/claim_notes；parse 失败打点 + soft Pass；verify LLM 失败 fail-open 交卷 |
| 2026-08-07 | **V1.3 二轮 review 5 项**：产品 token 含 synth+verify；`effective_max_verify_fails` / 耗尽强制 ceiling；`deliver_synthesized` 去重；证据分桶截断；`verify-empty-advice.md`；rereretrieve Activity 分计 |
| 2026-08-07 | **V1.4 验收阻断修复**：`mode_assemble` 继承 `forbid_retrieve_direct_answer`/`verify`/`verify_max_fail_rounds`；回检索共享 product rounds；回合成注入 draft；`verify_report` telemetry；verify cancel；assemble 断言 |
| 2026-08-07 | **V1.5 提示词**：skill 显式五维核对 + 边界默认 + 三则形态 few-shot；system 强化 advice 锚点与空证据默认；user 模板分段 +「优先核对事实性主张」；host schema 未变（无 `issues`） |
