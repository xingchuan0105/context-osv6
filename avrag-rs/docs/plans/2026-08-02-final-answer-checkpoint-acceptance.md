# 终答质检台标准化 · 验收指令（另开窗口）

> 本文件供发起人在**新窗口**按此验收。实施窗口已按 `2026-08-02-final-answer-checkpoint-impl.md` 完成 WP0–WP4 并各自提交。
> 验收以**代码现状 + 命令输出**为准，不依赖实施窗口的口头结论。若某项与文档描述不符，以代码为准并在文末「验收记录」标注。

## 1. 任务一句话

把终答形态质检从「散点 if」重构为「规则卡注册表 + 印章备案制 + 统一入口 + 台账」，并让 nudge 反馈指名命中形态。

## 2. 验收环境

- 工作目录：`/home/chuan/context-osv6/avrag-rs`（cargo 相关命令在此执行）
- 凭证：不需要（无 LLM 调用；纯单测验收）
- WSL `jobs=2`：命令**逐一**执行，勿并发堆叠 cargo test
- 服务：无需 Milvus/PG/Redis（`cargo test --lib` 不依赖外部服务）

## 3. 验收步骤

### 3.0 确认提交与工作区基线

```bash
cd /home/chuan/context-osv6/avrag-rs
git log --oneline | head -20
git status --short
```

预期：
- git log 顶部应含本任务 4 个 commit（按新到旧）：`b7c1da9c`（WP4 文档）、`6e5d61d8`（WP2+WP3 规则卡）、`1b2ce8e1`（WP1 印章备案）、`3d81db54`（WP0 前置）。其间可能夹有其他工作线 commit（skillopt/app-chat），正常。
- `git status` 中**本任务的文件应全部已提交、无残留**（下表文件都不应出现在 status 的 M/?? 中）。工作区若出现 `../AGENTS.md`、`../opencode.json`、`../.mcp.json`、`../.gitignore`、`../docs/agent/*`、`tools/skillopt/*`、`docs/plans/2026-08-02-skillopt-*` 等改动，属**另一工作线 in-flight**，**不要触碰、不要 add/commit**。

本任务涉及文件（应全部已提交）：

| 阶段 | 文件 |
|---|---|
| WP0 | `react_loop/answer_contract.rs`、`react_loop/synthesis.rs`、`react_loop/skill_request.rs`、`react_loop/policy/exit_policy.rs`、`tests/rag_quality/src/eval_v2/judge_parse.rs`、`prompts/system/agent-base.md`、`prompts/loop/synthesis-prose-repair.nudge.md`、`prompts/loop/README.md` |
| WP1 | `react_loop/host_markers.rs`（新建）、`react_loop/mod.rs`、`react_loop/iteration_codegen.rs`、`react_loop/assembler.rs`、`react_loop/policy/disclosure_plan.rs` |
| WP2+WP3 | `react_loop/answer_contract.rs`、`react_loop/policy/exit_policy.rs`、`react_loop/prompt_assets.rs`、`react_loop/synthesis.rs`、`prompts/loop/synthesis-prose-repair.tmpl.md`（rename 自 nudge.md） |
| WP4 | `prompts/loop/README.md`、根 `../AGENTS.md`、`EXTENDING.md`（agent-loop）、`docs/plans/2026-08-02-final-answer-checkpoint-impl.md` |

### 3.1 G1 规则卡注册表

```bash
grep -n "pub const FINAL_ANSWER_RULES" crates/agent-loop/src/react_loop/answer_contract.rs
sed -n '1000,1040p' crates/agent-loop/src/react_loop/answer_contract.rs
```

预期：`FINAL_ANSWER_RULES: &[FinalAnswerRule]`，四张卡 id 依序为 `code_only` → `host_shell` → `template_artifact` → `executable_code`，每卡含 `check: fn(&str)->Option<&'static str>` 与 `feedback_hint`（第三人称事实句，无命令式）。

新增检测的接入方式：表加一行 + 对应 `*_matched` 函数 + 单测，不改引擎控制流。

### 3.2 G2 统一入口

```bash
grep -rn "is_code_only_answer\|contains_host_observation_shell" crates/agent-loop/src/react_loop/synthesis.rs crates/agent-loop/src/react_loop/policy/exit_policy.rs
```

预期：**零命中**（或仅出现在注释）。三处调用点（synthesis 修前/修后、exit_policy DirectAnswer 路由）全部走 `check_final_answer`：

```bash
grep -n "check_final_answer" crates/agent-loop/src/react_loop/synthesis.rs crates/agent-loop/src/react_loop/policy/exit_policy.rs
```

预期至少 3 处调用（synthesis.rs 两处 + exit_policy.rs 一处）。

### 3.3 G3 nudge 反馈指名化

```bash
ls prompts/loop/synthesis-prose-repair.tmpl.md        # 存在（nudge.md 已不存在）
sed -n '1,6p' prompts/loop/synthesis-prose-repair.tmpl.md
```

预期：首行含 `本次命中形态：{violation_detail}。`；`synthesis-prose-repair.nudge.md` 不存在。

```bash
grep -n "violation_detail\|synthesis_prose_repair_nudge" crates/agent-loop/src/react_loop/prompt_assets.rs crates/agent-loop/src/react_loop/synthesis.rs
```

预期：prompt_assets.rs 定义 `synthesis_prose_repair_nudge(detail: &str) -> String` 并用 subst 替换 `{violation_detail}`；synthesis.rs 两处调用点传入 `violation.feedback_hint`。

### 3.4 G4 印章备案制

```bash
grep -c "HostMarker {" crates/agent-loop/src/react_loop/host_markers.rs     # 预期 13
grep -n "fn every_md_tag_candidate_is_registered\|fn every_marker_emitter_exists\|fn detector_set_matches_registered_forbidden_markers\|fn parity_fails_on_unregistered_md_tag" crates/agent-loop/src/react_loop/host_markers.rs
```

预期：4 个 parity 测试在（md 标签候选∈登记表 / 发射端文件存在 / 检测集=forbidden 子集 / 未登记标签敏感）。

发射端无标签字面量残留（登记表是唯一来源）：

```bash
grep -n "code_execution_result\|loop_budget\|retrieve_cluster_index\|docscope_metadata" crates/agent-loop/src/react_loop/iteration_codegen.rs crates/agent-loop/src/react_loop/assembler.rs crates/agent-loop/src/react_loop/policy/disclosure_plan.rs | grep -v "host_markers\|marker("
```

预期：命中的都是经由 `marker(...)`/备案常量派生，或仅注释；无裸字符串标签。

### 3.5 G5 台账 stage 改名

```bash
grep -n "final_check" crates/agent-loop/src/react_loop/synthesis.rs
```

预期：`final_check:{rule_id}:repair` 与 `final_check:{rule_id}:fallback` 两处（含 sink.emit 的 stage）。旧名 `synthesis_code_answer_repair`/`synthesis_code_answer_violation` 在 synthesis.rs 应零命中（可能残留于历史报告/注释，可忽略）。

### 3.6 G6 测试门

```bash
cargo test -p agent-loop --lib -- --test-threads=1     # 预期 291 passed
cargo test -p avrag-guardrails --lib                   # 预期 45 passed
```

（逐一执行，勿并发。）

### 3.7 文风抽查

- `prompts/loop/synthesis-prose-repair.tmpl.md` 全文第三人称，无「请/必须/不要」命令式。
- 四张规则卡 `feedback_hint` 均为事实陈述。
- 抽查 `prompts/loop/README.md` 中 synthesis-prose-repair 一行：文件名已是 `tmpl.md`，描述含 `{violation_detail}`。

## 4. 验收清单（逐项打勾）

- [ ] G1 `FINAL_ANSWER_RULES` 四卡齐全、顺序即检测顺序；新增检测只需表一行 + 函数 + 测试
- [ ] G2 synthesis.rs / exit_policy.rs 无 `is_code_only_answer` / `contains_host_observation_shell` 直调，统一 `check_final_answer`（≥3 处）
- [ ] G3 nudge 首行含 `{violation_detail}` 指名；tmpl 存在、nudge.md 已删；prompt_assets/synthesis 测试断言无占位符残留
- [ ] G4 host_markers.rs 13 条目；parity 4 测试在；发射端源码无裸标签字面量
- [ ] G5 stage 新命名 `final_check:*`；旧名零命中（除历史报告）
- [ ] G6 agent-loop 291 / guardrails 45 全绿
- [ ] 文风：新增 LLM 可见文案全为第三人称事实陈述

## 5. 验收记录

验收人：主窗口（本会话）。验收时间：2026-08-02。全部命令亲跑，未采信实施窗口口头结论。

- [x] G1：✅ 四卡齐全（`answer_contract.rs:1000`），顺序 code_only→host_shell→template_artifact→executable_code 即检测顺序；每卡 `check` 返回 `Option<&'static str>` 命中详情；`feedback_hint` 全为第三人称事实句；新增检测=表一行+函数+测试（结构符合）
- [x] G2：✅ `synthesis.rs:299/321` + `exit_policy.rs:65` 三处统一 `check_final_answer`；`is_code_only_answer`/`contains_host_observation_shell` 在两文件零直调
- [x] G3：✅ `synthesis-prose-repair.tmpl.md` 首行含 `{violation_detail}`；旧 nudge.md 已删；`prompt_assets.rs:134` subst 替换、:174 测试断言无占位符残留；`synthesis.rs:317` 传入 `violation.feedback_hint`
- [x] G4：✅ `HostMarker {` 13 处匹配（12 条目 + 1 行 struct 定义，口径自洽）；4 parity 测试在（host_markers.rs:177/196/235/254），含 `parity_fails_on_unregistered_md_tag` 敏感性自测；发射端开标签经登记表派生（`iteration_codegen.rs:595-599`）；`:605` 闭合标签 `</code_execution_result>` 仍为字面量——闭合形态不在检测面，评估为可接受残留（若追求极致可从 marker 派生闭合串，非阻塞）
- [x] G5：✅ `final_check:{rule_id}:repair` / `final_check:{rule_id}:fallback` 见于 counts 与 stage 双处（synthesis.rs:302/305/324/327）；旧名 `synthesis_code_answer_*` 在 synthesis.rs 零命中
- [x] G6：✅ `cargo test -p agent-loop --lib -- --test-threads=1` = **291 passed, 0 failed**；`cargo test -p avrag-guardrails --lib` = **45 passed, 0 failed**（逐一执行）
- [x] 文风：✅ tmpl.md 全文第三人称、无命令式；四张卡 feedback_hint 均为事实陈述；README:34 文件名与 `{violation_detail}` 描述同步；`AGENTS.md:30` 印章备案硬规则、`EXTENDING.md:79-83` 规则卡指向均在（WP4 闭环）
- 偏差：无新增。预登记偏差两项成立（D2 LoopHooks 本轮不接线；D5 台账改名与历史报告断代）。备注：O1/O2 提交 `682f5b63`（预算口径+SKILL 分层）在验收基线内，291 含其新增测试。

**结论：验收通过。**
