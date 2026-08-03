# Agent 输出契约三层架构：灰度表达 · 输出编译器 · 质检边界

> **SUPERSEDED** — 本文描述的 orchestrator / worker / brief / handoff 多 agent 架构已被取代：2026-07-30 起产品路径改为单 agent（SaC 设计，见根 `docs/plans/2026-07-30-sac-sdk-single-agent-design.md`），orchestrator 代码已于 2026-08-01 物理删除（commit `7f2d182d`）。本文仅作历史记录。（横幅添加于 2026-08-02 文档体系梳理）

| 项目 | 内容 |
|---|---|
| 状态 | **设计待评审** |
| 日期 | 2026-07-27 |
| 关联 | ADR-0012（评测侧 judge-first）；诊断依据 `rag_eval_v2/v2_20260727-022503` 全量与 `v2_20260727-153953` 44 题复测 |
| 范围 | 生产 chat 链路（worker / brain / Answer 的输出契约与校验）；不改评测管道 |

---

## 0. 一句话

模型经常**知道真相**，但交付链条没有"我不知道 / 这是推断 / 题目错了"的合法位置；报错只有"空"与"丢弃"两种哑巴形态；质检把结构与内容混为一谈。本设计建立三层：**灰度表达**（质疑与不确定性的合法出口）、**输出编译器**（rustc 式诊断 + 一次重发）、**质检边界**（确定性只管结构，内容交给 LLM 角色）。

---

## 1. 证据（为什么必须做）

44 题复测后残留失败的解剖（详见两次诊断报告）：

| 题 | 模型的思考（原文引用） | 落笔结果 | 死因 |
|---|---|---|---|
| q042 | 「未明确说明实际访谈了多少名……但从上下文推断应覆盖」 | 推断写进 summary 事实位，Answer 断言"4 名" | coverage 无 inferred 档；想标注无处标注 |
| q045 | 「文中未写明其总部所在城市」（**正确**） | 自创 `{"task_result": …}` 包装 → 整单被丢弃 → Answer 用世界知识补洞 | 解析全有或全无；无诊断、无重发 |
| q087 | 「只有这一条直接匹配」（实际有两行） | 无 key_facts 的 `{"handoff": true, "summary": …}` 被放行，合并相邻表格行 | 解析过宽（见 `summary` 即收）；校验零对象 |
| q114 | 「文档核心框架是 4R 而非 4P，需**推测提取**」（worker 看见了真相） | premise 错误只能写成散文备注；Answer 把警告当脚注、把 4P 表当正文 | schema 无 premise_mismatch 槽；brain brief 直接采纳用户错误前提 |
| q087 附 | — | 全角 `【E:n】` 引用标记漏出到用户答案 | finalize 只认半角 `[[E:n]]` |

共性：**思考→落笔的转写步失真**；**报错无信息量**；**校验只看证件（指针）不对内容**。

---

## 2. 设计原则

1. **每个合法语义都必须有合法槽位**——推断、查无、前提错误、主体错位，先给位置，再谈纪律。没有位置的语义一定会从非法通道泄漏。
2. **诊断优于丢弃**——任何拒收必须附带"哪里错、怎么改"的机器可读诊断，并给一次重发机会（编译器哲学：agent 看到好报错一次就会修）。
3. **硬闸门只管结构**——确定性质检（代码）只验证 schema、指针真实性、标记完整性；**不做**内容语义判断。
4. **内容忠于原文是 LLM 角色的活**——claim 与 observation 的语义核对交给轻量 LLM 校验者或 Answer 层在可见信号下完成；正则/重叠率只能当辅助信号，不当闸门。
5. **加法兼容**——schema 扩展全部 serde-additive，旧产出、旧 prompt 不炸。

---

## 3. 层一：灰度表达

### 3.1 Handoff schema 扩展（`internal_worker_handoff_v1` 加法升级，不改名）

```jsonc
{
  "schema_version": "internal_worker_handoff_v1",
  "summary": "…",
  "key_facts": [
    {
      "claim": "Y公司营销人员编制为 4 人",
      "evidence": ["chunk-id-…"],
      "basis": "observed"            // 新增：observed | inferred（默认 observed）
    },
    {
      "claim": "访谈可能覆盖了全部 4 名营销人员",
      "evidence": [],
      "basis": "inferred"            // 推断合法存在，但必须带标签
    }
  ],
  "coverage": "full | partial | insufficient",
  "gaps": ["…"],
  "premise_mismatch": {              // 新增，可选；worker 的"质疑按钮"
    "kind": "entity | frame | scope",
    "detail": "问题预设的 4P 拆解属于竞争对手南通四方",
    "actual_subject": "Y公司策略为 4R 框架（关联/反应/关系/回报）"
  }
}
```

- **`basis: inferred`**：合法携带推断；渲染与软层校验都按标签区别对待（推断不进引用、不进 Answer 事实位）。
- **`premise_mismatch`**：worker 发现"题目前提/主体/框架与证据不符"时的结构化否决权。q114 的 worker 由此从"写备注"变成"发信号"。
- **查无即成功**：文档化——`coverage=insufficient + key_facts=[] + gaps=[查无说明]` 是 should-refuse 类问题的**满分交付**，不是失败。

### 3.2 渲染层（Answer 上下文）

`render_synthesize_context`（chat_exit.rs）升级：

- `basis=inferred` 的 fact 渲染为 `（推断）…`，并附一行"推断内容不得作为事实引用"；
- `premise_mismatch` 渲染为 **⚠ 前提质疑块**（位置在 Channel outcomes 之前，显眼）；
- worker 声明与 Evidence 原文冲突时**并列展示** + "冲突时以 Evidence 原文为准"（替代静默二选一）；
- `handoff_degraded` 继续标注，措辞改为"worker 输出未通过编译（诊断码见日志），按未覆盖处理"（本措辞 supersede 2026-07-27 P3 切片刚落的"未通过校验"文案）。

### 3.3 上游前提核对（brain 与 brief）

首次 brief 发生在任何派发之前，brain 手里没有证据，**无法预验证**。前提核对落在两处：

1. **brief 作为给 worker 的核查指令**：`orchestrator-base.md` 的 brief 格式增加 `[premise/归属核对]` 一节——要求 worker 把"验证问题框架/主体归属是否与证据一致"列为调查任务之一，发现错位时通过 `premise_mismatch` 上报（§3.1）；
2. **finish / 重派时**：brain 收尾前对照已入库证据复核各 worker 的 `premise_mismatch` 信号，存在时在 `### Orchestrator instruction` 里写明纠正后的口径（点名真正主体）。

### 3.4 Answer 前提纠正规则（product-answer-base.md）

新增一条（与评测侧实质拒答契约对齐）：

> 若问题预设的框架/归属与证据不符：先纠正前提（点名真正主体/真正框架），再决定拒答或按纠正后口径作答；不得为满足问题结构把其他主体的内容归入所问主体。实质性声明"语料未按该框架记载"即算正确拒答（形式不限）。

### 3.5 附带修复

全角 `【E:n】` 引用标记纳入 `finalize_answer_evidence` 的改写/剥离范围。

---

## 4. 层二：Agent 输出编译器

### 4.1 定位与形态

在 `agent-loop` 新增 `output_compiler` 模块：统一所有 agent 产出物（worker handoff JSON、skill_request JSON、codegen 块、Answer 契约输出）的**解析 + 结构校验 + 诊断生成 + 一次重发**。类比 rustc：错误码 + 定位 + 修复建议。现有 `parse.rs`、`skill_request.rs`、`answer_contract.rs`、`workers::parse_worker_handoff` 的校验逻辑逐步收敛为编译器的规则表（迁移，非并行两套）。

```rust
pub struct Diagnostic {
    pub code: &'static str,      // "E101"
    pub severity: Severity,      // Error | Warning
    pub field: Option<String>,   // "key_facts[2].evidence"
    pub message: String,         // 哪里错
    pub suggestion: String,      // 怎么改（给模型看的自然语言）
}

pub struct CompileOutcome<T> {
    pub value: Option<T>,        // 解析成功（可带 warning）
    pub diagnostics: Vec<Diagnostic>,
}
```

### 4.2 诊断码（初版；编译器 v1 只服务 worker handoff，其余输出类型按需接入）

| 码 | 含义 | 建议文案要点 |
|---|---|---|
| E101 | handoff 缺 schema_version / 非契约外壳（如 `task_result` 包装） | 给出完整契约骨架 |
| E102 | 循环内 tool_results 非空但 key_facts 缺失/为空（编译器不可见 EvidenceStore，以循环状态为准） | 列出现有 evidence 指针，要求逐条归纳 |
| E103 | key_facts[].evidence 指针不存在于真实观察 | 列出合法指针集合 |
| E104 | 检出 `<code_execution_result>` 伪造块 | 声明该块一律剥离 |

**Warning 级**（不拒收，仅标注）：推断词命中（推断/推测/大概率/可能/未明确）→ 建议改为 `basis: inferred` 而非断言（提示而非硬剥离）；围栏包裹 JSON（C3/C4 已容忍，仅提示推荐无围栏）。

**v1 明确不接**：skill_request（解析发生在循环进行中的披露路径，集成方式不同）、answer_contract（自有成熟契约机）。两类产出后续按需接入编译器，不在本设计首战范围。

### 4.3 重发机制（核心行为变更）

**挂点在循环内的输出决策点，不是循环结束后。** 现状：`parse_worker_handoff` 在 worker 的 ReAct 循环**退出之后**（`workers.rs worker_handoff_from_run`）才执行，那时"回灌重发"无处可挂。正确形态：

1. 在 ReAct 循环的 `direct_content`（产出最终内容）决策点，先过编译器；
2. 编译失败（Error 级）→ 该输出**不视为最终输出**，诊断列表渲染成一条紧凑反馈消息进入下一轮 observation（"编译失败：E101 …；请按契约重新输出，不要新检索、不要代码块"），循环自然继续——这就是 rustc 反馈的天然用法，复用现有循环 machinery，**不发明新的特殊轮**；
3. 防循环：同一 worker 最多因编译失败续行一次（计数器），超出或预算耗尽则走现有 `degraded_unparsable` 兜底，诊断码记入渲染文案；
4. **与 C5 收官轮是同一通道**：C5 的预算耗尽交接轮（`run_synthesis.rs BUDGET_EXHAUSTED_FINAL_TURN`）产出的 handoff 同样过编译器——一套编译通道，两类触发点（中途 direct_content / 收官轮），不另立机制。

q045 场景（正确结论 + 错误包装）由此被挽救；q087 场景（无 key_facts）被 E102 续行补强或合法降级。

### 4.4 与既有修复的关系

- C3（只执行 python 围栏）→ 成为编译器 codegen 分支的一条规则；
- C4（sanitize）→ 其结构校验（schema、指针真实性、伪造块剥离）迁移进编译器（E101–E104）。注意 C4 原本就只做结构判断、**没有语义判断**，所以这里是"迁移"而非"移出"；语义核对是本设计层三**新增**的能力（§5），不是从 C4 搬家；
- C5（收官交接轮）→ 产出过同一编译通道（§4.3 第 4 点）；
- 共享剥离器（C6）→ 编译器的前置步骤。

---

## 5. 层三：质检边界——内容核对作为 LLM 角色

### 5.1 分工

| 层 | 职责 | 手段 |
|---|---|---|
| 编译器（硬闸门） | schema、指针真实性、标记完整性、伪造块剥离 | 代码，确定性 |
| 软层校验（可选开启） | claim 是否被 observation 原文支持 | 轻量 LLM（复用 JUDGE_LLM_* 配置）或 Answer 自查 |
| Answer 渲染 | 冲突并列、推断标注、以原文为准 | 渲染规则 + prompt |

### 5.2 软层校验者（可选，env 开关）

worker handoff 通过编译后、入库前（先结构后内容：编译器的指针真实性检查通过后，才逐条核对存活 claim）：

- 输入：key_facts[].claim + 其 evidence 指针指向的 chunk 原文（编译器已收集好这一对）；
- 逐条问 cheap 模型：「该 claim 是否被原文严格支持？observed / inferred / unsupported」。**配置走产品侧 `MEMORY_LLM_*` 链路（temp 0），或新增 `VERIFY_LLM_*` 覆盖；不得引用评测侧的 `JUDGE_LLM_*`**；
- 结果**只标注不改写**：`basis` 被确认/改写，`unsupported` 的 claim 标 `⚠` 并移入 gaps（不删——保留审计痕迹，渲染层决定如何呈现）；
- 失败/超时：跳过校验，不阻塞主链路（与评测 judge 的 JUDGE_ERROR 哲学一致）。开关名建议 `WORKER_FACT_VERIFY=1`，默认关。

注意：这与 W2（bigram 重叠）的关系——重叠率可以作为一个廉价的预筛信号喂给校验者，但**不作独立闸门**。

### 5.3 不做的事

- 不在编译器里加任何语义判断规则（防止硬闸门重新长出软判断）；
- 不为单题/单报错串写特判（所有规则必须通用）；
- 不改变评测侧 judge（它已经按实质契约打分，与本设计自洽）。

---

## 6. 改造点清单（实现索引）

| # | 位置 | 变更 |
|---|---|---|
| S1 | `agent-loop/src/output_compiler/`（新） | Diagnostic/CompileOutcome、规则表、重发消息渲染 |
| S2 | `agent-loop` 循环决策点 + `app-chat/src/orchestrator/workers.rs` | 编译器挂接 direct_content 决策点（失败续行一次）；parse_worker_handoff 结构校验迁移进编译器；degraded 带诊断码 |
| S3 | handoff schema（`orchestrator/types.rs`）+ `host.rs` task brief | basis / premise_mismatch 字段与文档 |
| S4 | `chat_exit.rs` + `invariant.rs` | 灰度渲染（inferred 标注、premise_mismatch 块、冲突并列、诊断文案） |
| S5 | `workers.rs` / store 入库前 | 软层校验者插桩（env 开关，默认关） |
| S6 | prompts：orchestrator-base（brief 前提核对）、product-answer-base（前提纠正规则）、capability 手册与 codegen SKILL（新字段教学、查无即成功、表内精确匹配纪律） | 文案 |
| S7 | `finalize_answer_evidence` | 全角【E:n】纳入 |
| S8 | 评测侧联动（仅必要处） | handoff_degraded 渲染文案变更对 eval 无侵入；确认 harness 对 premise_mismatch 块的兼容（无需改 judge） |

## 7. 切片与验证

| 切片 | 内容 | 验证门 |
|---|---|---|
| S1+S2 | 编译器骨架 + handoff 迁移 + 重发 | `cargo test -p agent-loop -p app-chat`；q045 型单测（错误包装→诊断→重发成功） |
| S3+S4+S6 | schema 灰度 + 渲染 + prompt 教学 | 单测 + q114 型渲染快照 |
| S7 | 全角标记 | 单测 |
| S5 | 软层校验者（默认关） | mock LLM 单测；开启后 q042 型 claim 被标 inferred/unsupported |
| 验收 | E2E_QUESTIONS=42,45,87,114 复跑 | 四题行为变化逐题核对；空转轮次不升 |

纪律同前：T5 行为保持切片、每片一个 commit、WSL 串行 cargo、graphify 收尾。

## 8. 成功标准

1. q045 型（正确结论+错误包装）：重发后结论保留，不再被整单丢弃。
2. q042 型（推断）：推断以 `basis: inferred` 合法存在，Answer 事实位不出现未标注推断。
3. q087 型（无 key_facts）：E102 打回；仍补不上则合法 degraded，Answer 以证据原文为准。
4. q114 型（前提错位）：worker 发 `premise_mismatch`；Answer 先纠正前提（点名南通四方）再作答或实质拒答。
5. worker 轮次浪费率（现 ~34%）不升；degraded 渲染含诊断码。

## 9. 风险

| 风险 | 缓解 |
|---|---|
| 重发增加延迟（每 worker 最坏 +1 轮） | 仅编译失败触发，且仅一次 |
| 软层校验成本 | 默认关闭；仅 cheap 模型；claim 级小 prompt |
| schema 加法后旧 worker 不写新字段 | 全部 serde default；旧产出按 observed 处理 |
| prompt 变长 | 灰度教学并入现有 brief/手册，替换而非叠加 |

---

**下一步**：本文档评审通过后，按 §7 切片开工（S1+S2 先行）。
