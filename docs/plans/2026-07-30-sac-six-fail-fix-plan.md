# SaC 黄金集 6 错题修复计划（待评审）

| 项 | 内容 |
|---|---|
| 日期 | 2026-07-30（WP-S1 写法 2026-07-31 纠正） |
| 状态 | **待负责人检查**（未开工；S1 体裁已按反馈改写） |
| 范围 | SaC 单 agent 黄金集仍非 PASS 的 6 题 + 横切 cite/skill/完备性 |
| 数据 | 三阶段 E2E：`/tmp/sac_e2e/REPORT.md`；产物 `v2_20260730-144717|144944|145227`；过程 `realistic_corpus_full_eval/q0{18,86,88,105,106,121}.json` |
| 对照 | 全量基线 135/149（`v2_20260730-062908`）；本轮 14 题 8 PASS / 6 仍挂 |
| 约束 | A1–A8 不破；单 agent；solo 本地 `master`；不扩前端 capability；**LLM 提示词禁止硬编码在 Rust**（一律 `prompts/**/*.md`，loop nudge 见 `prompts/loop/` + `prompt_assets`）；**禁止**黄金集/语料字符串进产品代码与 skill few-shot |

---

## 0. 一句话

6 题不是六种互不相关的 bug，而是 **三条横切缝** 叠在不同题型上：

1. **cite 断链**（代码）：答案写了 `SELECTED: #n`，产品侧 `citations=[]` → F=0 / SELECTION_MISS / UNGROUNDED  
2. **表格素养**（skill + 读表策略）：错行序、漏单元格、邻域未读全  
3. **证据完备性**（loop 出口）：有任意证据即可 DirectAnswer，多 claim / 多源半载不停、不回退

本计划按 **P0 横切 → P1 skill → P2 完备性软回退 → 验证** 排期；每项写清打哪几题、改哪里、怎么验。

---

## 1. 现状快照（6 题）

| q | subset | v2 标签 | 答案侧现象 | `citations` | 主因（讨论结论） |
|---|---|---|---|---|---|
| **18** | thesis_synthesis | UG | 三条反应策略 + `SELECTED: #5,#8,#7` | **[]** | cite 断 + 内容是否正确仍靠 F 后二次看 |
| **86** | ipd_table | SELECTION_MISS | 答 **LPDT-04**（应为 **LPDT-03** 序） | **[]** | 表序/编号理解错 + cite 断 |
| **88** | ipd_table | UG | 数字 **59/30 正确** + `SELECTED: #1` | **[]** | **几乎纯 cite**（计数策略已对） |
| **105** | cross_document | PARTIAL | 相似度表写满 + SELECTED 多 alias | **[]** | 半载/缺 caveats 类 rubric + cite |
| **106** | multi-source | RM | **370 对**；638 答「文档未给出」 | **[]** | **未扫到/未识别**表内 `638个业务对象（L3）` + 完备性未拦 |
| **121** | rag_search_joint | SELECTION_MISS | web 长答，**无** `[[web:n]]` / SELECTED | **[]** | joint 引用协议 + 口径；cite 断 |

**横切铁证**：6 题产物 `citations` 全是 `[]`；至少 18/88/105/106 答案正文已写 `SELECTED:`。  
→ **P0 修 cite 后，88 有很大概率单独变 PASS；18/105 也可能从 UG/PARTIAL 挪档**，但 86/106/121 仍要 skill/完备性。

---

## 2. 根因分层（不是「题号清单」）

```text
                    ┌──────────────────────────────┐
                    │  P0  SELECTED → cite 水合    │  打：全 6 题 F/cite
                    └──────────────┬───────────────┘
                                   │
          ┌────────────────────────┼────────────────────────┐
          ▼                        ▼                        ▼
   P1 表格素养              P1 多源/联合纪律           P2 完备性软回退
   86 序、106 单元格        105 caveats、121 [[web]]   105/106 半 claim
   88 巩固 total_hits       18 合成圈选                DirectAnswer 前
```

| 层 | 性质 | 现有行为 | 缺口 |
|---|---|---|---|
| **证据存在性** | 代码硬闸 | `require_evidence`：无 chunk 禁止答 | 已有，不动 |
| **证据完备性** | 无 | 有 chunk → 散文即 `DirectAnswer` | 多实体/多源不校验、不可回退 |
| **证据可引用** | 半残 | SELECTED 写在答案里 | 单 agent 路径未 hydrate → `citations=[]` |
| **读表** | skill 弱 | codegen 有 grep 一句；`how-to-read-tables` 选修 | 默认不加载；序/单元格未教透 |

与 [pi-book](https://zhanghandong.github.io/pi-book/)：单 agent + 工具循环已对齐；**「停前自验 + observation 续轮」**仍缺 → 用 P2 补，不新开 agent。

---

## 3. 工作包（WP）

### WP-C0 — SELECTED / alias 水合到产品 citations（P0，代码）

**目标**：单 agent DirectAnswer 路径上，答案中的 `SELECTED: #n`（及 alias）解析为真实 chunk_id，进入 `citations` 过滤链路；web 侧 `[[web:n]]` 可落地。

**为何必须先做**：

- q088 内容已对，F=0 仅因无 cite  
- 6 题 `citations=[]` 使 judge 的 faithfulness / selection 系统性失真，**后续 skill 改动会被噪声淹没**

**改动方向（实现时再定精确文件，验收时对照）**：

| 项 | 说明 |
|---|---|
| 水合挂点 | 现 `hydrate_selected` / SELECTED 解析多绑定 worker/orchestrator；**产品 `run_general_mode` 单 loop 出口必须调用等价逻辑** |
| 输入 | bridge capture 的 tool_results（内部仍可映射 dense_retrieval 等 telemetry 名）+ 答案字符串中的 `#n` / SELECTED 行 |
| 输出 | 与 ADR-0008 一致的 `[[cite:chunk_id]]` 或 citations 列表；无 SELECTED 时不强造 cite |
| web | search/joint：正文需 `[[web:n]]`（skill 教写）；过滤侧已有 web 协议则只接单 agent 答案 |

**不做**：改 judge；恢复 orchestrator 仅为 cite。

**验证**：

```text
门禁：
- 单测：SELECTED #1,#2 → citations 非空且 chunk_id 对齐 bridge
- 定向 E2E：至少 q088（期望 C≈1 且 F 回升 / v2 PASS）
- 回归：q018/105 若 SELECTED 已写，citations 不再 []
```

**打题**：直接 **88**；间接 **18,105,106**（以及 86 若补 SELECTED）；121 还依赖 web 标记协议。

---

### WP-S1 — 表格素养 skill（P0/P1，提示词）

**目标**：rag 路径能读懂 markitdown 管道表；覆盖 86（序）、88（行计数语义）、106（单元格与表头对齐）。载体：`codegen/SKILL.md` 极短挂钩，或 `reference/how-to-read-tables.md` 默认 disclose（D3：仍非前端 mode；守 A8 token）。

#### 写法约束（负责人纠正，强制）

表格 skill **只允许下面两种体裁，二选一或拼段；禁止第三种**。

| 允许 | 形态 | 目的 |
|---|---|---|
| **A. 是什么** | 陈述表格/管道行/列/命中数在文档与 observation 里**是什么** | 给 LLM 世界模型，不下令 |
| **B. few-shot** | 短输入→输出样例（问题 + observation 片段 + 正确读法/答法） | 示范推理，不列规矩 |

| 禁止 | 例子（计划里曾误写的风格，**不得进 skill 正文**） |
|---|---|
| 应该 / 不应该 / 请 / 禁止 / 必须 / 不要 | 「禁止 dedupe」「应先 grep 再答」「不能对活动号 min()」 |
| 步骤清单式纪律 | 「1. 先… 2. 再… 3. 只信…」当主文（与 A/B 冲突时删） |

> 实现侧硬约束（SDK 无 count/dedupe、`total_hits` 语义）属于 **API/产品代码**，不靠 skill 用「禁止写 set」重复宣讲。Skill 只负责让模型**看见表的结构**。

#### 体裁 A 草稿——「表是什么」（实现时润色，非最终文案）

用定义句，不用祈使句。需覆盖的**概念**（验收对照 86/88/106，不是写成 do/don't）：

| 概念 | 「是什么」说法方向 |
|---|---|
| **管道行** | 库内表经 markitdown 后常为 `\| c1 \| c2 \| … \|`；**一行 = 一条记录**，记录是各列的并置，不是某一列单独成对象。 |
| **列角色** | 表头（或首行）给列命名；单元格里的 token 只有贴在**同行 + 该列表头**下才是该属性的值。裸数字（如 `638`）本身不是完整事实，完整事实是「表头/邻列语义 + 该格」。 |
| **重复** | 重复 = **整行各列相同**；单列（如名称）相同而其它列不同 = 多条记录。 |
| **total_hits** | `grep` 返回的 `total_hits` 是服务端对**命中行数**的计数（行级规模），不是活动名去重后的集合大小。 |
| **表序 vs 编号** | 活动号（如 LPDT-04）是标签字符串；**「第一个」若指流程/表中的先后，对应的是表中出现顺序或显式序号列**，不是活动号字符串的字典序/数值 min。 |
| **列过滤形态** | 阶段名等类型词既可出现在类型列，也可出现在描述句；管道单元格的形态是两侧 `\|` 夹住的字段（observation 里常见 `\|\s*验证阶段\s*\|` 这类片段）。 |
| **邻域** | `grep` 的 `before`/`after`（context）是同行邻列与邻行文本；单元格语义常分布在邻域里而不只在匹配行的单一 token。 |

现有 `how-to-read-tables.md` **大体已是体裁 A**，实现时：删掉「读表时 1.2.3.」祈使小节；把 86/106 缺的「表序 vs 编号」「单元格=表头+格」补成定义句；**不要**加「禁止 min()/禁止 dedupe」段。

#### 体裁 B 草稿——few-shot 骨架（**禁止**写 realistic corpus / 黄金集题面或金标）

每个 shot：**虚构题意 → 虚构表/observation → 读出的事实**。不写「正确做法是」。
验收题号只出现在计划「打题」表，**不得**进入 skill 正文或 agent-loop 源码/单测字符串。

**Shot 1（行计数 · 概念）** — 虚构「在库」货位行数 + `total_hits`  
**Shot 2（表序 · 概念）** — 虚构角色 R / `STEP-03` 先于 `STEP-04`  
**Shot 3（表头+单元格 · 概念）** — 虚构「42个配置项（L2）」与表头对齐

#### 放置与默认加载

- **优先**：扩写 `reference/how-to-read-tables.md` 为纯 A，或 A 后附 2–3 个 B shot；codegen 主 skill 只留一句挂钩（「管道表结构见 how-to-read-tables」）。  
- **加载**：改为 rag **默认 disclose**（选修 → 默认），避免模型从不 skill_request。  
- **token**：A 宜短；B 每 shot 极短 observation，总数仍压 A8。

**验证**：

```text
- 定向：q086（LPDT-03）、q088（59/30）、q106（638 写入答案）
- skill 正文 grep：无「禁止|必须|不要|应该|不应」类祈使（中英 do/don't）
```

**打题**：**86, 88, 106**。

---

### WP-S2 — 多源 / joint / 合成纪律（P1，提示词）

**目标**：跨文档与 rag+search 联合时，**claim 对账 + 引用协议**写进 capability skill。

| 主题 | 写入位置 | 要点 | 打题 |
|---|---|---|---|
| 多源 fan-out | `capability-rag.md` | 多实体/多文档题同块或连续块覆盖；一侧满一侧空不得收工 | 105, 106 |
| Caveats / 边界 | capability-rag 或短 cross-doc 段 | 对比题写「相似」须同时写 **文档未支持的差异/边界**（rubric PARTIAL 来源） | **105** |
| 联合 web | `capability-search.md` + joint 路径 | 事实须 `[[web:n]]`；勿用「资料来源：网络搜索」散文代替协议标记 | **121** |
| 合成圈选 | capability-rag | 最终 SELECTED 覆盖答案中每个事实句；禁止只 SELECTED 目录块 | **18**（与 C0 联动） |

**验证**：定向 105（PARTIAL→升）、106（638 侧）、121（expect_citations doc+web）、18（内容+cite）。

---

### WP-L1 — Stop 决策 = 模型 + skill（对齐 pi；**无**宿主语义启发式）

**目标**：有 answer-grade 证据后，**DirectAnswer / stop 由模型在 skill 语境下决定**；宿主不扫草稿做多实体/软拒答/web 标记完备性 gate。

| 层 | 动作 | 谁 |
|---|---|---|
| **Skill / capability** | 第三人称描述覆盖是什么、半覆盖是什么、few-shot 对照（见 capability-rag） | 模型读后自决 Continue（再 code）或 stop |
| **Host 仅保留** | 零证据 `require_evidence`；worker `compile_feedback`（结构） | 宿主二元/结构门 |
| **禁止** | `answer_looks_incomplete`、completeness Continue×1、claim checklist 进 exit_policy | — |

**术语（与 `AGENTS.md` / `STATE_MACHINE.md` 一致）**：Continue · DirectAnswer/stop · observation · compile_feedback · require_evidence。不用 host「完备性软门」命名。

**验证**：单测无宿主完备性路径；定向 105/106 靠 skill + 检索质量，非 loop 启发式。

---

### WP-V — 验证矩阵与推进门槛

| 阶段 | 题集 | 通过标准 |
|---|---|---|
| V0 单测 | cite 水合 + completeness 续轮 | `cargo test -p agent-loop --lib` 相关；`app-chat` 若挂 assemble |
| V1 定向 6 | `E2E_QUESTIONS=18,86,88,105,106,121` | **目标 ≥5/6 v2 PASS**；88 必须 PASS；citations 非空（web 题有 web cite） |
| V2 黄金 14 | 原 A+B+C 14 题 | **≥12/14** 且无「原 PASS 变挂」 |
| V3 全量 149 | 不灌库 nightly | **≥135/149**（业务门槛）；仅 V2 过后再跑 |

**日志约定**：`/tmp/sac_e2e/six_fix_*.log`；产物目录记入本计划附录。

---

## 4. 逐题「修复映射」表（评审用）

| q | 期望正确行为 | WP 映射 | 成功信号 |
|---|---|---|---|
| **88** | 59/30 + 可解析 cite | **C0**（主）+ S1 巩固 | v2 PASS；F 回升；citations≥1 |
| **18** | 正确三条策略 + cite | **C0** + S2 圈选；内容若仍错再查检索段 | citations 非空；C/F 过 τ |
| **86** | LPDT-**03** 为概念阶段 LPDT 第一个（按表序/题意） | **S1** 序规则 + C0 | 答案含 LPDT-03；非 04 |
| **105** | 双源对比完整 + 边界/caveat + cite | **S2** + **L1** + C0 | PARTIAL→PASS |
| **106** | 370 **且** 638；禁止假「未给出」 | **S1** 单元格 + **L1** 多 claim + C0 | 答案含 638；RM→PASS |
| **121** | 时间口径合理 + `[[web:n]]`（及 doc 若用） | **S2** joint + C0（web 过滤） | SELECTION_MISS 消；cit 协议过 |

---

## 5. 建议实施顺序

```text
1. WP-C0  cite 水合          ← 先解 F=0 系统性噪声
2. WP-S1  表格 skill         ← 86/106/88
3. WP-S2  多源/joint skill   ← 105/121/18
4. WP-L1  完备性软回退       ← 钉死 105/106 半载与假拒
5. WP-V1  定向 6 → V2 14 →（可选）V3 149
```

**依赖**：S1/S2/L1 不依赖 C0 才能写，但 **评测解读依赖 C0**；故顺序上 C0 第一。  
**L1 可与 S1/S2 并行开发，合并后一起 V1。**

---

## 6. 明确不做（本计划外）

| 不做 | 原因 |
|---|---|
| 恢复 orchestrator / worker / handoff | 违背 A2；黄金 8 题回升已证明单 agent 方向 |
| 新前端 capability（table/cross_doc） | D3：仅 skill |
| 聚合原语 count/dedupe | A4；q088 靠 total_hits |
| 全量 149 抢跑 | 先 V1/V2 |
| push / PR / CI 剧场 | solo trunk |
| 删 orchestrator 死代码大扫除 | 可另开；不挡 6 题 |

---

## 7. 风险与回滚

| 风险 | 缓解 |
|---|---|
| 水合过宽把未采用 chunk 塞进 citations | 仅 SELECTED / 答案内 `[[cite:]]`/`[[web:]]` 出现的 id |
| 完备性续轮烧预算 | cap=1；已完备不触发；与 token/round 取 min |
| skill 变长破 A8 | 表格用短「是什么」+ 极少 shot；不写 do/don't 清单撑长度 |
| skill 又写成纪律文 | 实现门禁：正文禁「禁止/必须/不要/应该」；评审对照体裁 A/B |
| L1 规则误伤简单题 | 仅多 claim / 假「未找到」/ joint 无 web 标记时触发 |
| 86 金标「第一个」歧义 | 以题面+表序为准；实现前可对金标句再读一眼 |

回滚：C0/L1 用 feature 或 mode 开关可关；skill 可 git revert 单文件。

---

## 8. 验收检查清单（你签收用）

- [ ] 同意 **三条横切** 诊断（cite / 表格 / 完备性）  
- [ ] 同意 **C0 → S1 → S2 → L1 → V** 顺序  
- [ ] 同意 V1 门槛：**6 题 ≥5 PASS 且 88 必过**  
- [ ] 同意 L1 为 **软 Continue×1**，不新 agent  
- [ ] 同意本批 **不跑全量 149**，除非 V2 过  
- [ ] 同意 **表格 skill 仅「是什么」或 few-shot**，禁止 do/don't 清单  
- [ ] 需补充/删减的题或 WP：____________  

---

## 9. 附录：关键路径

| 用途 | 路径 |
|---|---|
| 设计锚点 | `docs/plans/2026-07-30-sac-sdk-single-agent-design.md` |
| SaC 开发计划 | `docs/plans/2026-07-30-sac-sdk-single-agent-dev-plan.md` |
| 循环状态机 | `avrag-rs/crates/agent-loop/src/react_loop/STATE_MACHINE.md` |
| content 出口 | `.../iteration/content_dispatch.rs` |
| 证据硬闸 | `.../policy/exit_policy.rs` |
| codegen skill | `avrag-rs/prompts/clusters/codegen/SKILL.md` |
| rag/search 能力 | `avrag-rs/prompts/orchestrators/capability-rag.md` / `capability-search.md` |
| 三阶段报告 | `/tmp/sac_e2e/REPORT.md` |

---

**请检查**：范围是否过宽/过窄、P0 是否同意以 cite 为第一刀、L1 是否接受进本批还是只做 L0 prompt。确认后按 §5 开工。
