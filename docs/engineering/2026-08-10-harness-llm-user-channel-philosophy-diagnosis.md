# 诊断：Harness / LLM / 用户 三角信道 vs 现网实现

| 字段 | 内容 |
|------|------|
| **状态** | **P0–P2 已开工落地**（2026-08-10）：删脚注、ceiling 分叉、disaster 目录、DSML 闸、AGENTS/verify skill、过程卡 progress 键；见 git diff |
| **日期** | 2026-08-10（初稿）；同日主审复核 + 方案补丁 + **实施** |
| **范围** | 主路径 rag/search 三环（retrieve → synthesis → verify）出站与环内观察；含 full149 相关个案（q048 DSML 等）作证据 |
| **非范围** | 本文件本身不改代码；不重新定义评测 label |
| **相关** | 根 `AGENTS.md`（observation 声线）；`avrag-rs/prompts/loop/README.md`；`avrag-rs/docs/engineering/2026-08-07-retrieve-synthesis-verify-loop-design.md`；`avrag-rs/prompts/system/agent-base.md`；`avrag-rs/prompts/clusters/verify/SKILL.md` |

### 主审复核摘要（2026-08-10）

- **事实可信度：高。** 抽查的代码锚点、文案资产、评测 artifact 与正文一致，未见夸大。
- **方案方向：正确**——改「谁有权改写用户字符串」，不拆环；符合 layered-growth。
- **原文方案三条落地点缺口**已写入 **§17（方案补丁）**；未补齐前禁止「只删脚注」裸跑 P0。
- **一处事实出入：** q048 泄漏标记为含 `DSML` 的 provider 形态（artifact 中为双竖线类 DSML 块），匹配规则须用 **`DSML` 子串**，不得依赖某一字面全角竖线形态（见 §4.3 / §17.4）。

---

## 1. 产品哲学（验收标准）

主审确认的三角关系：

| 角色 | 地位 | 职责 |
|------|------|------|
| **用户** | 前台 | 提问、读答复、再对话 |
| **LLM** | 前台 | 判断、行动、**对用户说人话**；对 harness 读环境 |
| **Harness** | 后台 / 环境 | 执行工具、给 **准确观察**（非祈使）、存中间态、内部裁决与重试；**不**冒充对用户说话的人 |

### 1.1 信道约束

1. **Harness → LLM**：事实 / 环境态（第三人称 observation），不是「请你必须…」。
2. **LLM → Harness**：工具意图 / 中间草稿（可多轮、可持久化）。
3. **LLM → 用户**：**唯一**主气泡合法内容源（自然语言；说明 / 澄清 / 追问均可，**措辞由 LLM 自主**）。
4. **Harness → 用户**：原则上 **无**。过程 UI 若存在，须产品化文案，不是状态机原文、不是评测诊断句。
5. **中间产物**可存、可记、可回灌模型；**不得**原样进入客户可见终答。

### 1.2 失败时的用户形态（馆员隐喻）

- 检索失败、终审不过、证据不足：统一要求是 **自然语言** 描述、澄清或追问——像图书馆管理员仍正面回应，而不是堆程序化状态。
- **不**强制固定拒答模板；由 LLM 判断如何说。
- 程序化报错、轮次上限、是否有 tool 回传、verify fail 细节 → **后台诊断 / telemetry**，不暴露给用户主通道。
- **不要**「终稿审不过仍强制交付 + 挂系统脚注」。

### 1.3 与既有法则的关系

| 既有 | 与本哲学 |
|------|----------|
| `AGENTS.md`：prompts 第三人称 observation、非祈使 | **对齐** harness→LLM |
| `AGENTS.md`：verify 非 host 语义 checklist | **对齐** 环内自主 |
| loop disclosure / ceiling 用户可见脚注 | **冲突** harness→用户 |
| 评测 label / 可观测 disclosure | 可保留在 **后台与 eval**；不得镜像进主气泡 |

---

## 2. 总判

| 层 | 对齐度 | 一句话 |
|----|--------|--------|
| Harness → LLM（环内） | **高** | 第三人称 observation、结构闸 Continue、verify 回环，大体是「环境说话」 |
| LLM 自主（环内） | **中高** | 调工具 / 写稿多由模型；但 C5 / disclosure / 固定 fallback 存在宿主代写用户话 |
| LLM → 用户（出站） | **中低** | 理想路径是模型散文；失败路径常被 **宿主模板 / 脚注 / 协议泄漏** 劫持 |
| 中间态隔离 | **中** | 过程卡、telemetry、tool_results 有分路；**disclosure 故意并进 answer** 是明确违规 |
| 出站消毒 | **有骨架、有漏洞** | `FINAL_ANSWER_RULES` 方向对；DSML 等未覆盖；闸失败后用固定句顶替而非再请 LLM 说人话 |

**根矛盾：**  
环内按「环境 + 馆员后台」运作；出站按「可审计评测系统」做 **用户可见诚实披露**。后者与「LLM 前台、人话收束」冲突。

`avrag-rs/prompts/loop/README.md` 写明：loop 文案既是 model-visible observation，**也**含 fixed user-facing fallback——**双信道文件是设计债，不是偶然 bug。**

---

## 3. 三信道现网地图

```text
                 ┌──────────── 用户（前台）────────────┐
                 │  主气泡 = answer / MessageDelta      │
                 │  过程卡 = Activity（部分工程腔）       │
                 └──────────▲────────────▲──────────────┘
                            │ ① 应只有 LLM 人话
                            │ ② 现状：LLM + 宿主脚注/模板
       ┌────────────────────┴────────────┴─────────────────┐
       │                 Harness（后台）                     │
       │  tools / sandbox / 结构闸 / verify / repair / 存证   │
       │  telemetry · knockout · ews · tool_results         │
       └──────────▲────────────────────────┬────────────────┘
                  │ ③ 观察（多轮）            │ ④ 执行
                  │                          ▼
       ┌──────────┴────────────────────────────────────────┐
       │              LLM（前台执行者）                       │
       │  retrieve / code / synthesis /（参与）verify         │
       └───────────────────────────────────────────────────┘
```

---

## 4. 用户主通道污染源（Harness → 用户，与哲学直接冲突）

### 4.1 故意拼接系统脚注

| 机制 | 位置 | 文案资产 | 意图（代码注释） | 哲学判定 |
|------|------|----------|------------------|----------|
| Verify ceiling 脚注 | `agent-loop/.../verify.rs` → `append_verify_ceiling_disclosure` | `prompts/loop/verify-ceiling-disclosure.md`：「自动核对已达次数上限…」 | 到顶仍交付终稿并挂说明 | **违规**：审不过 + 系统旁白 |
| 无证据 disclosure | `run_synthesis.rs` → `maybe_append_evidence_disclosure` | `evidence-missing-disclosure.md` / `*-no-attempt.md` | *Host-determined… never model-authored so it cannot be dropped* | **违规**：主动剥夺 LLM 措辞权，且 no-attempt 含「本 run 未见检索侧调用」类诊断腔 |

### 4.2 宿主代写主气泡（固定句 → MessageDelta）

| 机制 | 位置 | 文案资产 | 哲学判定 |
|------|------|----------|----------|
| 无证据 degraded | `synthesis.rs` 格式闸失败且无 evidence | `degraded-no-evidence-*.md`（含「请重试…」） | **违规**：宿主当前台；且祈使腔 |
| 合同/格式 fallback | 同文件 repair/rerender 仍挂 | `contract-violation-*.md` | **违规**：固定产品句，非馆员即兴 |
| （部分）partial  salvage | `extract_partial_synthesis_fallback` 等 | 视路径 | 需审计：是否模型生成 |

### 4.3 中间态泄漏为「答案」

| 现象 | 证据 | 哲学判定 |
|------|------|----------|
| Provider 协议原文进终答 | full149 q048（`v2_20260810-100217`）：终答为 **含 `DSML` 的 tool_calls 块**（artifact 中为双竖线类标记，形态可能随 provider 变化）+ ceiling 句；`tool_trace` 仅 1× `lexical_retrieval` Ok；该 full run 唯一 RETRIEVAL_MISS | **违规**：wire format 当答案；规则表无 `DSML` |
| 根因层级（主审补强） | 合成环吐出 DSML = 模型把合成当成 native tool-call 回合；**根因在 provider 适配 / 协议渗入 content**，终答闸只是止血 | 见 §17.4；**勿把规则表当修复终点** |
| 检索期 code / host shell 进终答 | `FINAL_ANSWER_RULES` 已拦一类；有 repair | **方向对**；失败后固定句仍偏宿主前台 |
| Template token | `template_artifact`：`</response>`、`<response>`、`<\|im_end\|>`、`<\|im_start\|>` 共 4 项 | **方向对**；集合不全（无 DSML） |

**检测注意（主审出入修正）：** 实现 P1 时匹配 **`DSML` 子串**（及同类 provider 协议残片策略），**不要**写死单一竖线字面量（文档初稿单竖线写法不完整）。

### 4.4 过程 UI 灰区

| 点 | 位置 | 说明 |
|----|------|------|
| 过程卡与主气泡分离 | `frontend_next/hooks/chat-session/*` | **方向对** |
| `progress.*` 键本地化 | `progress-i18n.ts` | **方向对** |
| Legacy free-text Activity 原样透传 | 同文件 `!isKey` 分支 | 工程句（如 `final_answer quality gate fired…`，`synthesis.rs`）可能上过程前台 |
| 过程 ≠ 答案 | — | 可接受为「环境进度」；须产品文案，禁止诊断原文 |

---

## 5. Harness → LLM（环内）：大体对齐

### 5.1 符合「准确观察、非祈使」

| 类型 | 资产 / 行为 | 评价 |
|------|-------------|------|
| 证据结构闸 | `evidence-missing.nudge.md` / `evidence-missing-no-client.nudge.md`；`content_dispatch` Continue | ✅ 第三人称环境事实 |
| 必做动作闸 | `required-action-missing-*.tmpl.md` | ✅ |
| Verify 回环 | `verify-fail-synthesis.tmpl.md` / `verify-fail-retrieve.tmpl.md`；`[verify_feedback]` | ✅ 给模型；文案写明不替代终答 |
| 沙箱 / 检索 summary | `retrieval-summary*.md`、`codegen-*.md` | ✅ |
| 合成 prose repair | `synthesis-prose-repair.tmpl.md` + `final-answer-feedback-*.md` | ✅ 格式观察；**失败出口**见 §4.2 |
| 工具执行回传 | sandbox / ToolCatalog | ✅ 环境响应 |

### 5.2 次要对模型「指令味」残留（优先级低于用户侧）

| 点 | 说明 |
|----|------|
| `synthesis-repair.nudge.md` | 「不要用 markdown 代码围栏」等 |
| `budget-exhausted-final*.md` | 对模型规定「用户可见答复为结论散文…」——环境里夹了用户话术规格 |
| 部分 skill / 历史文案 | 「应…」类；持续清理即可 |

### 5.3 环内失败消化（部分对齐）

| 路径 | 行为 | 评价 |
|------|------|------|
| verify fail → resynthesis / rereretrieve | `mod.rs` follow_up | ✅ 环内 |
| verify 到顶 `DeliverCeiling` | 旧稿 + `append_verify_ceiling_disclosure` | ❌ 应改为「最后一轮 LLM 用户收束」或等价 |
| format gate → repair → rerender | `synthesis.rs` | ✅ 环内；最终 fallback ❌ |
| L2 evidence_missing 多轮 Continue | `content_dispatch` | ✅ 直至预算；预算后见 C5 |

C5（预算尽）仍 **再给模型一轮** 写答（`budget_exhausted_messages`）——符合「人话由 LLM」；若再叠 evidence disclosure 则破坏。

---

## 6. 中间产物：可存 vs 出站

| 中间态 | 存储 | 用户可见风险 |
|--------|------|----------------|
| tool stdout / 检索块 | messages / tool_results | 合成引用 OK |
| verify advice / 旧稿 | messages（revision） | 应只给模型；✅ 注入路径 |
| knockout / ews / usage | observability | 后台 |
| 格式违规草稿 | repair 消息 | 应不进气泡 |
| DSML / 协议 token | 曾当 content | **可进气泡** ❌ |
| ceiling / evidence disclosure | **拼进 `final_answer`** | **故意运行态出站** ❌ |
| Activity / turn_metadata.progress | 过程卡 | 灰区 |

---

## 7. 与哲学条款逐条对照

### 7.1 把 LLM 当人

| 应有 | 现网 |
|------|------|
| 判断权：调不调工具、如何答 | 检索环大体有；结构闸只数 Ok ✅ |
| 措辞权：失败时怎么说 | 多条路径宿主代写 ❌ |
| 可澄清 / 追问 | 模型可以；host 脚注冲淡馆员感 ❌ |

`agent-base`（v1.8）已写：用户可见终答是普通文字；实现细节旁白 / 仿造观察壳不是终答——**对 LLM 的人格设定与哲学同向**；破坏来自 **交付路径宿主改写**。

### 7.2 Harness 是环境

| 应有 | 现网 |
|------|------|
| 每个工具/代码请求有准确回传 | ✅ |
| 结构事实用 observation 表达 | ✅ |
| 不对用户说话 | **多条路径在说** ❌ |

### 7.3 LLM 自主判断合理

| 应有 | 现网 |
|------|------|
| 继续检 vs 答 | 三环 handoff / verify 吸收意图——合理 harness |
| 质量失败环内消化 | re-entry ✅；到顶脚注 ❌ |

### 7.4 双前台 · harness 后台

**最严重越位：Harness 上用户前台。**  
`maybe_append_evidence_disclosure` 注释写明 *never model-authored so it cannot be dropped*——是 **刻意策略**（防模型丢掉「无证据」声明），不是实现疏漏。认本哲学则属 **策略废案**，不是改几个标点。

### 7.5 中间可存、客户不泄

见 §6。主缺口是 **answer 字符串被宿主追加** 与 **协议泄漏未没收**。

---

## 8. 失败路径：现状 vs 目标

### 8.1 现状（用户体感「坏了」）

```text
环内（接近目标）                      出站（偏离）
─────────────────                    ─────────────────
evidence_missing → Continue          → 终局常叠 disclosure 脚注
verify fail → re-synth / re-retrieve → ceiling：坏稿 + 系统句
format gate → repair → rerender      → fallback 固定句 MessageDelta
上游/协议异常                         → 软拒或 DSML 原文
budget C5 → 再给模型观察              → 尚可；再叠 disclosure 则坏
```

### 8.2 目标态（哲学）

```text
任何内部失败 / 到顶
  → 环内尽量消化（重试、换策略、再合成、再检索）
  → 最后仍由 LLM 对用户说人话（说明 / 澄清 / 追问，自主措辞）
  → harness 只写 telemetry / 内部 label，不改写 answer 字符串
  → 非 prose 出站：禁止；触发有界的「用户可见收束」观察（见 §17 次数阈值）
  → 灾难级兜底：仅 token 预算尽 / 模型连续格式失败超阈 等窄口；人话、单独目录、telemetry 必记
```

**出口分叉（主审要求，细节 §17.2）：**

| 到顶原因 | 用户通道 | 说明 |
|----------|----------|------|
| **轮次**到顶（仍有 token 余量） | **LLM 收束轮**（不拼脚注） | 专轮写馆员人话；可破例不计入产品轮次预算或预留 1 轮 |
| **Token** 预算已尽（`budget_forces_ceiling`） | **极小灾难兜底句**（人话、单独目录） | 不再开 LLM 收束轮；**禁止**旧脚注拼接 |
| 格式闸已原稿+repair+rerender 三败 | 可选第 4 次收束 **或** 灾难兜底 | 见 §17.3 阈值 |

---

## 9. 为何会变成现在这样（原因，非借口）

1. **评测驱动**：full149 / label 与「用户可见写清有无证据」绑定。  
2. **防模型撒谎**：宿主脚注保证「无证据」不被模型省略。  
3. **三环 + verify 成熟**：质量状态机完整，**出口策略**仍用「挂说明」而非「再请前台说一句」。  
4. **loop 双用途目录**：observation 与 user fallback 混放。  
5. **出站闸不完整**：有 template/code 类，无 provider DSML；闸失败走宿主句。  
6. **AGENTS 只管对模型声线**；**「用户通道禁宿主旁白」** 从未写成硬规则。

---

## 10. 对齐度评分（便于排期，主观）

| 维度 | 分 (1–5) | 说明 |
|------|----------|------|
| 环境观察对 LLM | 4 | 第三人称主路径好 |
| 环内失败消化 | 3.5 | 有 re-entry；到顶策略错 |
| 出站仅 LLM 人话 | 2 | 脚注 / 模板 / 泄漏 |
| 中间态不泄客户 | 2.5 | 有分路；answer 拼接破坏 |
| 过程 UI 产品化 | 3 | 有 process；文案不齐 |
| Provider 协议隔离 | 2 | DSML 类缺口 |
| 文档与哲学一致 | 3 | 对模型清；对用户规则矛盾 |

**综合：环内 ~「环境+人」；出站 ~「审计系统」。落地优先改出口策略，而非再堆 skill 文案。**

---

## 11. 改造靶心清单（勾选；实施前必读 §17）

> **门禁：** 未完成 §17.1（verify 判定面）+ §17.2（ceiling 分叉）+ §17.3（次数阈值）的设计落点前，**禁止**仅删除脚注上线。

### P0a — 制度与 verify（与删脚注**同步**，主审要求）

- [ ] `AGENTS.md`：写入 **「用户通道禁宿主旁白」**（与 observation 声线并列；防 wave 回潮）
- [ ] `prompts/clusters/verify/SKILL.md`：扩展失败形态张力——**无依据却装作有据 / 协议残片当终答 / 未用人话说明覆盖缺口** 等第三人称观察面（**非** host 语义 checklist；措辞仍由合成环人话收束）
- [ ] 三环设计文：废止「强制 user disclosure」；改为 §17 出口表

### P0b — 删除 Harness→用户脚注

- [ ] 删除/停用 `append_verify_ceiling_disclosure` 及 `verify-ceiling-disclosure.md` 用户拼接路径
- [ ] 删除/停用 `maybe_append_evidence_disclosure` 及 `evidence-missing-disclosure*.md` 用户拼接路径
- [ ] Telemetry / eval 仍可记 `ceiling` / `evidence_empty` 等 **后台** 标签

### P0c — DeliverCeiling 分叉（解决预算悖论）

- [ ] **轮次到顶** → `UserFacingCloseout`：一次 **LLM 收束轮**（观察：环境到顶事实；模型写人话；**不**拼脚注）
- [ ] **Token 到顶**（`budget_forces_ceiling`）→ **灾难兜底句**（人话、`prompts/loop/disaster/` 或等价单目录；**禁止**再开费 LLM；**禁止**旧脚注）
- [ ] 收束轮预算策略写死：轮次到顶时收束轮 **不计入** 产品 max_iterations（或预留 1）；token 到顶 **不再** 调合成 LLM

### P0d — 宿主固定句降级为灾难口

- [ ] `degraded_no_evidence_*` / `contract_violation_*` 移出常规 `MessageDelta` 主路径
- [ ] 仅在 §17.3 阈值耗尽后允许；目录与 telemetry 标明 `disaster_fallback`

### P1 — 出站消毒（止血 + 注明根因层）

- [ ] `FINAL_ANSWER_RULES`：`provider_protocol` 规则，匹配 **`DSML` 子串**（及后续同类 token 策略），**不**绑定单一竖线字面量
- [ ] 文档/注释标明：规则表 = 出站止血；根因 = provider 适配层勿让 DSML 进 content
- [ ] 闸失败：在已有 原稿→repair→rerender **之后**，按 §17.3 决定第 4 次收束 vs 灾难兜底
- [ ] 交付前断言：`answer` 不得含 host marker / `DSML` / 已知 disclosure 子串

### P2 — 过程与后台

- [ ] **可先做、不阻塞哲学：** `synthesis.rs` 等 Activity 工程英文 → `progress.*` 键（主审：纯文案 bug）
- [ ] Activity 全面键控；禁止工程英文透传过程卡
- [ ] C5 / repair 去掉「规定用户可见写法」祈使，只报环境事实
- [ ] Telemetry / eval 标签不镜像进用户句

### P3 — 目录与双用途拆分

- [ ] `prompts/loop/README.md`：model-only vs `disaster/` 用户兜底分列
- [ ] 删除或归档 user-facing disclosure 资产，避免双信道回潮

---

## 12. 应保留的资产（勿为「简单」拆环）

- 三环编排作为 **后台状态机**（retrieve / synthesis / verify）
- 对模型的 evidence_missing / verify_feedback 等 observation
- `check_final_answer` + one-shot repair 思路
- 过程卡与主气泡分离
- `agent-base`「终答是普通文字」
- Tool / sandbox 执行与 observation 回灌

改的是 **谁有权改写用户字符串**，不是拆掉环境层。

---

## 13. 个案锚点（便于审计对照 artifact）

| 题 | Run（示例） | 现象 | 哲学归类 |
|----|-------------|------|----------|
| **q048** | `v2_20260810-100217` | 终答 = **DSML tool_calls 块** + ceiling；`tool_trace` 仅 1× lex Ok；verify fail→retrieve→ceiling；全 run 唯一 RM | 协议中间态出站 + 宿主脚注；**删脚注不够，还要出站闸 + provider 层** |
| **q078**（主审强化） | DeepSeek 复跑 `v2_20260810-113647` | 模型已写出合格馆员话（无法从库中确认数量、建议查权威文档）；宿主仍叠 **no-attempt disclosure + ceiling** | **直接证伪**「脚注不可丢、否则模型不说实话」——脚注 **冗余**；诚实披露应靠模型 + verify 面，不靠拼接 |
| **q115** | 同形于 q078 复跑 | 同上：人话软拒 + 双脚注 | 同 q078 |
| q078/q115 full 基线 | `v2_20260810-100217` | 有据好答；当时 JE | 能力可达；标签/merge 是另一问题 |

评测 RM 标签规则本身可保留在 eval；**用户可见字符串**不应用同一「诚实披露」策略。

---

## 14. 一句话结论

> 哲学与 `agent-base` / 环内 observation **同向**。  
> 最大偏离是：**Harness 在失败路径上抢了用户前台**（强制 disclosure、固定 fallback、协议泄漏未没收），把后台诊断写进了主答复。  
> **环内像环境；出站像审计员。** 目标是：**环内仍像环境；出站永远只剩馆员（LLM）。**  
> 主审补充：q078 证明馆员话模型**已经会说**；脚注是噪声。P0 必须 **删脚注 + 扩 verify 面 + 分叉 ceiling**，不能只删。

---

## 15. 主审栏（已填）

| 项 | 主审意见 |
|----|----------|
| 哲学 §1 是否全文接受 | **是**（事实层与方向） |
| P0 是否同意「删除用户可见脚注」而非改写 | **是**，但必须配套 §17.1–17.3，禁止裸删 |
| 灾难级固定句是否允许保留（条件） | **允许**，极窄：token 到顶、格式三败超阈等；人话、单独目录、telemetry |
| 过程卡是否算「用户前台」须同等约束 | 产品化文案；**工程英文可先修**（不阻塞哲学） |
| 与 full149 / eval disclosure 的边界 | 标签与后台可观测保留；**不**镜像进主气泡 |
| 下一步 | 补丁 §17 入设计后开 P0；**AGENTS 与 P0 同步**，勿排到 P3 末 |

---

## 16. 文件索引（实现锚点）

| 主题 | 路径 |
|------|------|
| Ceiling 拼接 | `avrag-rs/crates/agent-loop/src/react_loop/verify.rs`（`append_verify_ceiling_disclosure` ~370；`budget_forces_ceiling` ~173） |
| Evidence disclosure 拼接 | `avrag-rs/crates/agent-loop/src/react_loop/run_synthesis.rs`（`maybe_append_evidence_disclosure` ~25；C5 后仍可能 ~283） |
| 三环入口 / DeliverCeiling | `avrag-rs/crates/agent-loop/src/react_loop/mod.rs`（~450–526） |
| 格式闸与 fallback MessageDelta | `avrag-rs/crates/agent-loop/src/react_loop/synthesis.rs`（~298–413；`emit_prose_delivery` ~453） |
| 终答规则表 | `avrag-rs/crates/agent-loop/src/react_loop/answer_contract/final_answer_rules.rs`（`FINAL_ANSWER_RULES` ~116） |
| 结构闸 Continue | `avrag-rs/crates/agent-loop/src/react_loop/iteration/content_dispatch.rs` |
| Verify skill | `avrag-rs/prompts/clusters/verify/SKILL.md` |
| Loop 文案 | `avrag-rs/prompts/loop/*.md` + `README.md`（L3 双信道说明） |
| 系统声线 | `avrag-rs/prompts/system/agent-base.md` v1.8 |
| 过程 UI | `frontend_next/hooks/chat-session/progress-i18n.ts`（`!isKey` ~40–42） |
| 三环设计 | `avrag-rs/docs/engineering/2026-08-07-retrieve-synthesis-verify-loop-design.md` |

---

## 17. 方案补丁（主审三条缺口的闭合设计）

> 本节是 **P0 可执行前置条件**。初稿 §11 只写了「删」，未写「删后谁保证诚实 / 预算尽怎么办 / 第几次放弃再请模型」。以下闭合。

### 17.1 诚实披露改由 verify 面 + 模型人话（替代脚注保证）

**问题：** 删除 `maybe_append_evidence_disclosure` 后，「无证据要如实对用户说」不能靠 host 拼句，也不能加 host 语义闸（`AGENTS.md`）。

**落点（合规）：**

1. **扩 `prompts/clusters/verify/SKILL.md` 裁决观察面**（第三人称张力，非祈使）：  
   - 终稿是否在 **无可用证据** 时仍写成既成事实；  
   - 终稿是否为 **协议/工具外壳** 而非用户可读答复；  
   - 覆盖缺口是否在终稿中有 **可读的不确定表述**（有/无由模型判断，verify 只报告张力）。  
2. Verify **fail → synthesis** 时，观察回灌合成环（已有 `[verify_feedback]` 路径），由合成 **人话改写**，host **不**拼用户脚注。  
3. **q078 证据：** tools=0 时模型已能馆员式说明——主路径可信；q048 类靠出站闸 + verify 张力 + 收束轮，不靠脚注。  
4. Eval / telemetry 继续记 evidence 空、verify fail 等；**与用户字符串解耦**。

**验收：** 无证据场景用户气泡中 **无** disclosure 子串；若模型胡说，verify 应 fail 并触发再合成（在预算内），而非宿主后置句。

### 17.2 DeliverCeiling 分叉：轮次到顶 vs token 到顶

**问题：** `budget_forces_ceiling`（token 已尽）时再开 LLM 收束轮 **无预算**。

| 触发 | 内部动作 | 用户通道 | 预算 |
|------|----------|----------|------|
| verify fail 次数到顶，**仍有 token** | 一次 `UserFacingCloseout`：注入第三人称「核对轮次已尽 / 当前稿状态」观察（**对模型**），合成/专用收束调用 | **仅** LLM 人话 | 收束轮 **不计入** 产品 `max_iterations`（或启动时预留 1）；计入 token 计量 |
| `budget_forces_ceiling == true` | **不再** 调合成 LLM | **灾难兜底句**（人话、`prompts/loop/disaster/`，单独目录） | 零额外 completion（或仅允许已缓存极短路径——默认零） |
| 任一路径 | **禁止** `append_verify_ceiling_disclosure` / evidence disclosure 拼 answer | — | — |

**与 §8.2 对齐：** 「禁止脚注」是硬约束；「灾难兜底」是 **token 尽** 的窄口，不是脚注的马甲（文案不得含「自动核对」「本 run」「调用回传」等诊断腔）。

### 17.3 格式闸：三轮之后的阈值

**现状：** `synthesis.rs` 已是 **原稿 → repair → rerender** 三轮模型机会，再败才固定句。

**原则：** 「再请 LLM 优先于固定句」成立，但 **有界**。

| 阶段 | 动作 |
|------|------|
| 第 1 次违规 | repair（现有） |
| 第 2 次仍违规且 has_evidence | rerender（现有） |
| 第 3 次仍违规 | **默认灾难兜底**（不再默认第 4 次整段合成）；telemetry `format_gate_exhausted` |
| 可选实验 | 配置开关允许 **一次** closeout（仅当 token 余量充足）；默认关 |

**不要**在无阈值情况下无限「再请一轮」。

### 17.4 DSML / provider 协议：止血 vs 根因

| 层 | 动作 |
|----|------|
| **出站止血（P1）** | `FINAL_ANSWER_RULES` 增加规则：内容含 **`DSML`**（子串）→ 违规 → 进入 17.3 链路 |
| **根因（并行/后续）** | provider 适配确保 tool 只走结构化 `tool_calls`；合成 system 已禁外壳（agent-base）——监控泄漏率，不单靠规则表 |
| **匹配** | 用 `DSML` 等稳定子串；**禁止**只匹配某一竖线字面全标记 |

### 17.5 可立即修（不阻塞 P0 设计）

- `synthesis.rs` Activity：`final_answer quality gate fired…` → `progress.*` 键 + i18n（主审：文案 bug）。

### 17.6 P0 执行顺序（补丁后）

```text
1. 写 AGENTS「用户通道禁宿主旁白」+ verify SKILL 失败形态面（P0a）
2. 实现 ceiling 分叉 + 删除两种 disclosure 拼接（P0b+P0c）
3. 固定句迁 disaster 目录与阈值（P0d + §17.3）
4. DSML 规则表止血（P1）与 provider 跟进
5. 过程卡工程句（P2 可插队）
6. loop README 拆目录（P3）
```

**完成 1–3 即满足主审「补完三点，P0 可执行」。**

---

## 18. 主审复核事实清单（存档）

以下条目经主审对照代码/artifact，**全部确认**：

| # | 指控 | 锚点 |
|---|------|------|
| 1 | ceiling 脚注拼 answer | `verify.rs:370`；文案 `verify-ceiling-disclosure.md`；`mod.rs:502-526` |
| 2 | evidence disclosure 刻意策略 | `run_synthesis.rs:24` 注释原文 *never model-authored…* |
| 3 | degraded / contract → MessageDelta | `synthesis.rs:381-413` → `emit_prose_delivery` |
| 4 | FINAL_ANSWER_RULES 仅 5 条；无 DSML | `final_answer_rules.rs:116`；仓库除本诊断外 DSML 零业务匹配 |
| 5 | loop 双信道写明 | `prompts/loop/README.md:3` |
| 6 | agent-base 终答=普通文字 | `agent-base.md` v1.8 §「用户可见终答」 |
| 7 | C5 后再叠 disclosure | `budget_exhausted_messages` + `maybe_append_evidence_disclosure` ~283 |
| 8 | 过程卡 legacy 透传 | `progress-i18n.ts:40-42`；`synthesis.rs:305` 工程句 |
| 9 | 环内正面描述属实 | host_markers、verify_feedback、content_dispatch 结构闸 |
| 10 | q048 / q078 / q115 artifact | 见 §13；q078 证伪脚注必要性 |

**出入（已修正进正文）：** DSML 匹配用子串，勿写死单竖线全标记。
