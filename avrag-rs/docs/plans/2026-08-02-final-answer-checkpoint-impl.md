# 终答质检台标准化 · 实施文档（2026-08-02）

> 本文档是开发契约：新窗口按此实施，发起人按 §7 验收清单验收。
> 业务语言版方案见 §1；技术改动以 §4-§6 为准。所有"现状事实"条目均已核实到文件:行号；实施时若与代码不符，以代码为准并在 §8 追加偏差记录。

## 1. 背景（业务语言摘要）

黄金集两轮全量诊断（`docs/engineering/2026-08-02-golden149-llm-behavior-report.md` 及 run2 分析）证明：**教学（observation）能降低次品率，但出厂口必须有质检**——q088 伪造宿主观察、q086 目录块回声、q018 模板残片、q095/q102 过程稿出厂，都是"终答形态"类不合格品。当前质检规则写死在循环引擎的两个调用点，防伪标签清单靠人肉同步（q086 正是清单外标签漏网）。

本方案把质检升级为三个构件：

1. **质检台**：终答出厂前的标准工位，规则以"规则卡"数据驱动，加规则不动引擎控制流；
2. **印章备案制**：宿主观察标签单一事实源 + parity 对账测试，新增标签不备案则测试红；
3. **质检台账**：每次触发按规则卡记账，黄金集报告可直接汇总次品率。

并补上差距评估中确认的两个工程项：反馈指名化（质检报告写明哪项不合格）、台账系统化。

## 2. 前置状态（必读）

- **上一轮修复在工作树未提交**（本方案的地基）：`crates/agent-loop/src/react_loop/{answer_contract.rs, synthesis.rs, skill_request.rs, policy/exit_policy.rs}`、`tests/rag_quality/src/eval_v2/judge_parse.rs`、`prompts/system/agent-base.md`、`prompts/loop/synthesis-prose-repair.nudge.md`、`prompts/loop/README.md`。新窗口**先 `git status` 确认这 8 个文件在，单独提交为一个 commit**（建议信息：`fix(loop): 终答形态契约四检测器 + judge 尾逗号容忍 + skill_request 容忍解析`），再开始本方案。
- **另一窗口的 app-chat 在飞改动（profile_update 重构）与本方案无文件交集，不要触碰**；若它尚未收尾导致 app-chat 编译失败，本方案全程不需要 app-chat 编译通过（验证只跑 agent-loop / guardrails）。
- 已合并的既有修复：add8a105（伪造闸）、23d49493（DirectAnswer 路由）、ff27a293/016c254c（struct_store CWD）。

## 3. 目标与非目标

### 目标（验收口径）

- G1 规则卡注册表：四类终答检测收敛为数据驱动的规则卡表；新增一类检测 = 表内加一行 + 一个检测函数 + 一个测试。
- G2 单一质检入口：三个现有调用点（DirectAnswer 路由、synthesis 修复前、修复后复检）全部走同一入口函数，违规返回**命中的规则卡与具体标记**。
- G3 反馈指名化：修复 nudge 带 `{violation_detail}` 占位符，模型收到的反馈写明命中的是哪类形态、哪个标签。
- G4 印章备案制：`host_markers.rs` 常量表为全部宿主观察标签的唯一事实源；发射端引用常量；检测器从表派生；parity 测试扫描 `prompts/loop/*.md` 与发射端源码，未备案标签 → 测试红。
- G5 台账：质检触发的 activity stage 统一为 `final_check:<rule_id>:<repair|fallback>`，随 mode_debug 出账。
- G6 `cargo test -p agent-loop --lib`（基线 284）+ `avrag-guardrails`（45）全绿。

### 非目标

- nonce 防伪章（对抗性伪造储备升级，本轮不做）。
- LoopHooks 新增终答扩展点（见 D2 决策理由：当前规则是全模式通用的，纯函数穿线成本大于收益；若未来产品侧需要模式差异化规则，再接线）。
- 证据充分性/grounding 政策（架构红线：skill-owned，质检台只管答复形态）。
- e2e harness 改造、app-chat 任何文件。

## 4. 设计决策

### D1 规则卡注册表（`answer_contract.rs`）

现有四个检测器（`is_code_only_answer`、`contains_host_observation_shell`、`contains_template_artifact`、`contains_executable_code_form`）保持公开不变，上层加注册表：

```rust
pub struct FinalAnswerRule {
    pub id: &'static str,                 // code_only / host_shell / template_artifact / executable_code
    pub check: fn(&str) -> Option<&'static str>, // Some(命中的具体标记/说明)，None = 通过
    pub feedback_hint: &'static str,      // 该形态的一句话指名说明（进 nudge 占位符）
}

pub const FINAL_ANSWER_RULES: &[FinalAnswerRule] = &[ /* 四张卡，顺序即检测顺序 */ ];

pub struct FinalAnswerViolation {
    pub rule_id: &'static str,
    pub matched: &'static str,            // 如 "<retrieval_summary>"
    pub feedback_hint: &'static str,
}

/// 质检台唯一入口。三张调用点都用它。
pub fn check_final_answer(text: &str) -> Option<FinalAnswerViolation>;
```

- 现有 `final_answer_contract_violation(text) -> bool` 改为 `check_final_answer(text).is_some()` 的薄封装，外部签名不破坏。
- 四个检测器当前返回 `bool`，各加一个返回 `Option<&'static str>`（命中标记）的变体供规则卡使用；`is_code_only_answer` 这类无具体标记的，`matched` 返回规则级说明串。
- `feedback_hint` 是**事实陈述**（第三人称），例：`"候选答复中含有宿主观察标签 <retrieval_summary>；该标签只由宿主注入"`。不是命令。

### D2 质检台不挂 LoopHooks（与业务方案的偏差及理由）

业务方案中"LoopHooks 新增终答前扩展点"一条，落地时改为**引擎内注册表即插槽**：

- 现有调用点之一 `policy/exit_policy.rs::decide_synthesis_gate` 是纯函数（无 hooks 句柄），把 hooks 穿线进纯策略函数需要改三处签名，且当前四类规则对全部模式通用——没有产品差异化的真实需求。
- 规则卡表本身是数据驱动扩展点：加卡不动控制流，达成"插槽"的业务目的。
- 若未来出现"按模式/按产品挂载不同规则"的需求，再把 `check_final_answer` 提为 LoopHooks 方法（默认实现即注册表）。此决策记录在案，不在本轮实施。

### D3 印章备案制（新模块 `react_loop/host_markers.rs`）

```rust
pub struct HostMarker {
    pub tag: &'static str,               // 前缀形态，如 "<loop_budget"、"[retrieval_summary]"
    pub forbidden_in_final: bool,        // 终答中出现即违规
    pub emitted_at: &'static str,        // 发射端位置说明（人读）
}

pub const HOST_OBSERVATION_MARKERS: &[HostMarker] = &[ ... ];
```

**初始登记清单**（已核实的发射端）：

| 标记 | 发射端 | forbidden_in_final |
|---|---|---|
| `<code_execution_result` | `iteration_codegen.rs` `format_codegen_result_message`（:561 一带） | ✓ |
| `<loop_budget` | `assembler.rs` `build_loop_budget_hint`（:181-183） | ✓ |
| `<retrieve_cluster_index>` / `<synthesis_skill_index>` | `policy/disclosure_plan.rs` `render_cluster_index`（:333-349） | ✓ |
| `<docscope_metadata>` | `policy/disclosure_plan.rs` `inject_cluster_runtime_context`（:408-423） | ✓ |
| `[retrieval_summary]` | `prompts/loop/retrieval-summary.tmpl.md` 经 `prompt_assets::retrieval_summary` | ✓ |
| `[blocks_skipped]` / `[contract_violation]` 等 nudge 标签 | `prompts/loop/*.md` 各文件 | 逐枚核对后登记（nudge 标签出现在模型可见观察里，终答回声同样违规；默认 ✓，如发现例外在 §8 记录） |

**发射端迁移**：代码内手写的标签字面量（`format!("<loop_budget …")`、`"<retrieve_cluster_index>"` 等）改为引用常量。md 文件内的标签不动（文案归 md），但标签名必须与登记表一致。

**parity 测试**（`host_markers.rs` 测试模块，测试时用 `CARGO_MANIFEST_DIR` 相对路径读文件）：

1. 扫描 `prompts/loop/*.md`，提取行首 `[tag]` 形态与 `<tag…>` 形态的标签候选，断言每个 ∈ 登记表；
2. 对登记表每个条目，断言其发射端真实存在（在声明的 .rs 源文件中能搜到该常量引用，或对 md 条目断言文件存在且含该标签）；
3. 断言 `contains_host_observation_shell` 的检测集 = 登记表中 `forbidden_in_final = true` 的子集（检测器改为从表派生后此断言由构造保证，测试防回归）。

### D4 反馈指名化

- `prompts/loop/synthesis-prose-repair.nudge.md` 改名为 `synthesis-prose-repair.tmpl.md`，首行加占位符：`本次命中形态：{violation_detail}。`（其后保留现有的四类形态说明全文——模型可对照完整清单）。
- `prompt_assets.rs`：`synthesis_prose_repair_nudge()` 改签名 `synthesis_prose_repair_nudge(detail: &str) -> String`，用现有 `subst()` 替换。
- `synthesis.rs` 两处调用点传入 `violation.feedback_hint`。
- `prompts/loop/README.md` 表格行与占位符清单同步。

### D5 台账命名

- `synthesis.rs` 现有 `synthesis_code_answer_repair` / `synthesis_code_answer_violation` 两个 activity stage 改名为 `final_check:{rule_id}:repair` / `final_check:{rule_id}:fallback`（动态拼 rule_id），message 文案同步。
- **断代声明**：旧名在 run v2_20260802-045319 及之前的报告中使用，趋势对比时注意改名点。e2e 报告脚本若按旧名统计需小改（本轮不改 harness，只在 §8 记录新名）。

### D6 文档同步

- `prompts/loop/README.md`：D4 占位符、文件名变更。
- 根 `AGENTS.md` 的 Prompts 规则段补一行硬规则：**宿主注入的观察标签必须先登记 `host_markers.rs` 再使用**（这是新约定，AGENTS.md 是合适的位置）。
- `agent-loop/EXTENDING.md`：若其中提到契约/终答相关内容，补一行指向规则卡注册表。

## 5. 工作包（依赖序）

### WP0 前置提交

提交 §2 列出的 8 个未提交文件（一个 commit）。验证门：`cargo test -p agent-loop --lib` 284 绿、`cargo test -p rag_quality --lib` 114 绿、`cargo test -p avrag-guardrails --lib` 45 绿。

### WP1 印章备案制（D3）

新建 `host_markers.rs`（常量表 + parity 测试）→ 发射端迁移（上表 5 处代码字面量）→ 检测器派生。验证门：`cargo test -p agent-loop --lib` 全绿 + parity 测试对新标签敏感（临时在 md 里加一个未登记标签验证测试确实变红，随后还原）。

### WP2 规则卡注册表 + 反馈指名化（D1、D4）

规则卡表、`check_final_answer` 入口、检测器 `Option` 变体、nudge 改 tmpl + 占位符、两个 synthesis 调用点接线。验证门：agent-loop 测试全绿；新增单测——四类违规各自的 `rule_id`/`matched` 断言 + nudge 输出含指名说明且不含 `{violation_detail}` 残留。

### WP3 调用点统一 + 台账（D2、D5）

`exit_policy.rs` 与 `synthesis.rs` 三处调用全走 `check_final_answer`；stage 改名。验证门：agent-loop 全绿；用 q086/q088/q018/q095 四个历史样本作 fixture 的集成测试（构造 DirectAnswer/合成输出 → 断言路由或修复路径触发且 rule_id 正确）。

### WP4 文档（D6）+ 收尾

README、AGENTS.md、EXTENDING.md 同步；`graphify update .`（结构性变更硬规则）；§8 补齐。验证门：人工审 + guardrails 绿。

## 6. 提交建议

WP0、WP1、WP2+WP3、WP4 各一个本地 commit（solo trunk，不 push）。每个 commit 独立过验证门。

## 7. 验收清单（发起人验收用）

- [ ] G1 `FINAL_ANSWER_RULES` 表存在，四张卡齐全；新增第五张示例卡（如临时验证）只需一处表项
- [ ] G2 `grep -rn "is_code_only_answer\|contains_host_observation_shell" crates/agent-loop/src/react_loop/{synthesis.rs,policy/exit_policy.rs}` 仅剩 `check_final_answer` 统一入口调用
- [ ] G3 nudge 输出含具体命中标记（测试断言），md 模板无占位符残留
- [ ] G4 `host_markers.rs` 登记表覆盖 §4 D3 表全部发射端；发射端源码无标签字面量残留；parity 三断言在
- [ ] G5 activity stage 新命名 `final_check:*`；旧名仅存在于历史报告
- [ ] G6 agent-loop / guardrails 全绿；graphify 已更新
- [ ] 文风抽查：feedback_hint 与 nudge 新增文案全为第三人称事实陈述，无命令式

## 8. 实施偏差记录

（实施中任何与本文档的偏差追加在此节。）
- 预登记偏差 ①：D2——LoopHooks 扩展点本轮不接线，理由见 D2。
- 预登记偏差 ②：D5 stage 改名造成与 run2 及更早报告的计数断代，不重命名回兼容。

### WP0（2026-08-02 提交 3d81db54）

1. **前置 8 文件按 §2 原样提交**（`fix(loop): 终答形态契约四检测器 + judge 尾逗号容忍 + skill_request 容忍解析`），验证门 agent-loop 284 / rag_quality 114 / guardrails 45 全绿，与计划一致。

### WP1（2026-08-02 提交 1b2ce8e1）

2. **host_markers 备案表 13 条目**（D3 表全部发射端 + nudge 标签逐枚核对）：`<code_execution_result>`（闭合，教学引用）/`<code_execution_result `（空格，iteration_codegen.rs format_codegen_result_message）/[no_output]/[sandbox_error]/<loop_budget/[retrieval_summary]/<retrieval_summary>（角括号仿造变体）/synthesis-prose-repair.tmpl.md/[blocks_skipped]/[format_hint]/<retrieve_cluster_index>/<synthesis_skill_index>/<docscope_metadata>，全部 forbidden_in_final=true。parity 4 测试覆盖 md 扫描/发射端存在/检测集匹配/未登记标签敏感（临时写 probe 文件验证变红后删除）。
3. **发射端迁移 4 处代码字面量**：iteration_codegen.rs/assembler.rs/disclosure_plan.rs（marker() helper + render_cluster_index + inject_cluster_runtime_context）；answer_contract.rs 检测器从 `forbidden_in_final_tags()` 派生。
4. **parity 扫描排除规则**：闭合形态 `[/tag]`、`[web:n]` 引用形态、`</name>`、`<|`、`<code` 不作为标签候选；registered 前缀匹配（候选以登记 tag 为前缀或登记 tag 去 `>` 后前缀命中）。

### WP2+WP3（2026-08-02 提交 6e5d61d8）

5. **规则卡注册表** `FINAL_ANSWER_RULES` 四卡顺序 = 检测顺序（code_only/host_shell/template_artifact/executable_code）；`check_final_answer` 唯一入口；`final_answer_contract_violation` 薄封装保留（外部兼容）。四检测器加 `*_matched` Option 变体（is_code_only_answer 无标记，matched 用规则级说明串）。
6. **调用点统一**：synthesis.rs 两处 + exit_policy.rs DirectAnswer 路由（:65 `if check_final_answer(answer).is_none()`）三处全走 `check_final_answer`；grep 验证 synthesis.rs/exit_policy.rs 无 `is_code_only_answer`/`contains_host_observation_shell` 直接引用残留。
7. **台账 stage 改名**：`synthesis_code_answer_repair/violation` → `final_check:{rule_id}:repair/fallback`（D5，断代声明成立）。
8. **nudge 反馈指名化**：`git mv synthesis-prose-repair.nudge.md → synthesis-prose-repair.tmpl.md`；首行 `本次命中形态：{violation_detail}。`；`synthesis_prose_repair_nudge(detail: &str) -> String`；host_markers 备案表 `<code_execution_result>` 条目 emitted_at 同步更新为 tmpl.md（否则 WP1 parity 测试红——发现并修复）。
9. **验证门**：agent-loop 291 passed（基线 284 + WP1 parity 4 + WP2 新单测 3）。

### WP4（2026-08-02 提交（本次））

10. **AGENTS.md 硬规则行**：根 AGENTS.md Prompts 节加「Host-observation markers must be registered first」段（必须先登记 `react_loop/host_markers.rs` 再使用）。该行与另一工作线 in-flight 的 graphify→code-review-graph 改名改动同文件共存，**提交时 AGENTS.md 单独处理（不与其混入本 commit 或按文件 hunk 隔离）**，以实际提交为准。
11. **EXTENDING.md** Product contract 节补 final-answer quality gate（FINAL_ANSWER_RULES + check_final_answer + host_markers 登记）指向。
12. **prompts/loop/README.md**：Files 表 synthesis-prose-repair 文件名 nudge.md→tmpl.md + 描述补 `{violation_detail}` 指名；占位符清单补 `{violation_detail}`。
13. **graphify update .** 待 WP4 验证门后执行（结构性变更硬规则）。

## 9. 环境纪律（摘自 AGENTS.md，全文有效）

- prompts-in-md：LLM 可见文案只住 `avrag-rs/prompts/**/*.md`；代码只做加载与占位符替换。
- 第三人称观察式：反馈话术陈述事实，不写命令。
- WSL：`jobs=2`；不并发跑多个全量 cargo test。
- 不 push、不 PR；本地 trunk 提交。
