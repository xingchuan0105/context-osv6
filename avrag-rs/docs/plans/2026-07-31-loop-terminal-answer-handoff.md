# loop 终答契约（P2 首件）交接文档（2026-07-31）

> 上游：`docs/plans/2026-07-31-struct-query-virtual-tables.md`（附录 B/C 记录了本问题的发现过程）。
> 本文档 = 「末轮 code 块当答案」故障的**确诊证据 + 机理链 + 修法方向 + 验证手册**。接手不需要原会话上下文。

## 0. 一句话现状

struct_query 线（P1c+P1d+2b）已全部落地；切片验收中暴露的**独立故障**——react loop 预算耗尽后 synthesis 收尾轮仍输出 code 块并被零校验地当作最终答案——已确诊到代码行，**修复未动手**。本文档交接该修复。

## 1. 现象与证据（真实 LLM 切片，`QUESTIONS=86,88,106`）

| 运行 | 日志 | Q86（表序） | Q88（阶段计数） | Q106（双数跨 doc） |
|------|------|------------|----------------|-------------------|
| run1 13:24 | `fail6_20260731-132409.log` | ✅ 正文正确（LPDT-03） | ✅ PASS | ❌ answer=code 块 |
| run2 14:41 | `fail6_20260731-144126.log` | ❌ `empty_answer`（INFRA） | ✅ PASS | ❌ answer=code 块（6 次调用后预算尽，末轮 code **未执行**） |
| run3 14:45（仅 86） | `fail6_20260731-144555.log` | ❌ answer=code 块（7 次调用全 Ok、末轮 struct_query **已执行**，recall=100%） | — | — |

共同点：**终答是一次 codegen 回合的 code 块原文，不是正文合成**。judge 因此判 correctness=0（「答案仅包含查询代码」）。
关键排除项：struct_query 改动无关——run1 的 Q106 在证据 chunk 落地前就已是此模式；run3 的 struct 结果只有 1 行，负载增量百字节级。

artifact 读取坑：**失败题不会覆写** `crates/app/tests/e2e_output/realistic_corpus_full_eval/q0NN.json`——读 artifact 先看 mtime（run2 的 q086.json 就是 run1 的残留）。

## 2. 确诊（机理链，全部 文件:行号 已核）

1. **code 块 ≠ DirectAnswer**：`agent-loop/src/react_loop/parse.rs:10-34` `parse_llm_output`——含 `<code>` 或 ```python 围栏 → `CodeBlocks` → 沙箱执行 → Continue；纯正文 → `DirectAnswer`。
2. **预算检查在 retrieve 每轮开头**：`run_retrieval.rs:45-66`；`iteration >= max_iters` 或 `tokens_used >= max_tokens` → `check_loop_budget_exhausted`（:156-162，只发 telemetry）→ break。末轮是 code 块时 `direct_answer = None`。
3. **进入 synthesis**：`policy/exit_policy.rs:44-63`（无 direct_answer → EnterSynthesis）；`run_synthesis.rs:204` 追加 C5 收尾消息后走 `SynthesisPhase::run`；rag 模式 contract = `prose_only`（`modes/rag.yaml:41`）→ `run_prose_stream`（`synthesis.rs:266-357`）。
4. **洞 1（主洞）**：`run_prose_stream` 对流式输出**零校验、零重试、零 fallback**——模型在 12 轮 code 块上下文中收尾时仍可能输出 code 块，原文逐字成为最终 answer。（JSON contract 路径才有 repair/contract_violation 兜底，synthesis.rs:85-263；`run_synthesis.rs:198-203` 注释已自认此风险。）
5. **洞 2（次要）**：C5 nudge（`prompts/loop/budget-exhausted-final.nudge.md`，经 `prompt_assets.rs:60-62` 加载）的注入闸门 `run_synthesis.rs:324-352` **只看轮次耗尽**（`:330` `iteration < max_iterations → None`）；token 先耗尽时不追加任何收尾观察，synthesis 在「输出 code」的框架里裸跑。
6. **harness 无识别**：`crates/app/tests/product_e2e/llm_real/rag_quality_prod.rs:1318-1330` 只查 `answer.trim().is_empty()`（empty_answer），code 块答案直接进入评分被当作普通错误答案，污染指标。

预算数值：`modes/rag.yaml:17-28`（基础 12 轮 / 28000 token；free 8/16000、pro 12/28000、enterprise 16/40000），解析在 `policy/config/config_types.rs:135-164`；无环境变量覆盖；E2E 可用 `request.metadata["assembled_mode_config"]` 注入 ModeConfig 或 `metadata["user_tier"]` 改预算（见 `app-chat/src/agents/unified/mod.rs:354-370`）。

## 3. 修法方向（建议按序）

- **F1（主修，host 结构校验）**：`prose_only` synthesis 输出若整体仍是 code 块（复用 `parse_llm_output` 的判定），做一次 repair 轮。先例：JSON 路径的 `synthesis-repair.nudge.md` + `contract-violation-*.md`。新增 prompt 落 `prompts/loop/`（**prompts-in-md 铁律**），**第三人称观察式**（陈述「上一轮回传仍是 code 块、用户可见答案需要正文」这类事实，不写命令/步骤；voice 规则见根 AGENTS.md）。repair 仍属 host 结构性纠正，不违反「stop 决策归 model+skill」——别加语义覆盖检查。
- **F2（小修）**：C5 闸门补 token 耗尽——把 `tokens_exhausted` 传进 `budget_exhausted_messages`（`run_synthesis.rs:324-352`），token 先尽时也注入收尾观察。
- **F3（测量卫生，harness）**：`crates/app/tests/product_e2e/llm_real/rag_quality_prod.rs` 把「纯 code 块 answer」记为 infra failure（类 `empty_answer`），不再当普通错误答案计 correctness=0。判定可复用简单的围栏/`<code>` 检测。
- 验证顺序：先单测（F1：mock 末轮 code + synthesis 仍出 code → 断言 repair 触发；F2：token 耗尽 → 断言 C5 消息注入），再真实 LLM 切片。

## 4. 环境/操作要点

- **脏树严重，且 agent-loop 正被其他在途工作改**：`assembler.rs`、`policy/*`、`skill_request.rs`、`app-chat/src/agents/unified/mod.rs`、`modes/*.yaml`、`prompts/loop/*` 等均有未提交修改（SaC 线）。**edit 前以当前文件内容为准；提交只挑自己改的 hunk，绝不 `git add -A`**。
- 本会话遗留的未提交文件（struct_query 2b 线，与本次修复正交，别混进同一 commit）：`crates/rag-core/src/runtime/tools/struct_query.rs`、`tests/rag_quality/src/harness_extract.rs`、`crates/storage-pg/src/lib_impl/repository_assets.rs`、`scripts/struct_query_poc/pipeline.py`、`scripts/struct_query_poc/load_evidence_chunks.py`（新）、`docs/plans/2026-07-31-struct-query-virtual-tables.md`、`docs/plans/2026-07-31-struct-query-p1c-handoff.md`。已提交：`24befd59`（P1c+P1d，12 文件）。
- 构建纪律：`CARGO_BUILD_JOBS=2`，同一时刻只跑一个 cargo；libduckdb-sys 已编译过，增量大。
- 验证命令：
  ```bash
  cd /home/chuan/context-osv6/avrag-rs
  CARGO_BUILD_JOBS=2 cargo test -p agent-loop --lib
  # 真实 LLM 切片（需 services + avrag-rs/.env；约 3 分钟/3 题 + judge）：
  cd /home/chuan/context-osv6
  CARGO_BUILD_JOBS=2 STRUCT_STORE_DIR=$PWD/avrag-rs/storage/struct_store \
    QUESTIONS=86,88,106 bash avrag-rs/scripts/sac-skill-fail6-reg.sh
  ```
- LLM 抖动事实：deepseek-v4-flash 同一题不同轮路径不同；**同一题至少跑两轮再定论**，别拿单轮结果当回归/修复证据。
- 日志判读：v2 label 在日志行 `v2: label=…`；judge 明细在 `crates/app/tests/e2e_output/rag_eval_v2/<run_id>/q0NN.judge.json`。
- 修完结构性代码后跑 `graphify update .`（仓库规则）；`graphify-out/` 不入库。

## 5. 验收标准与下一步

验收（全部满足才算完）：
1. 新增单测覆盖 F1/F2 触发路径；`cargo test -p agent-loop --lib` 全绿。
2. 切片复跑：86 正文答出 LPDT-03、88 保持 PASS（59/30）、**106 至少产出正文答案**（允许部分覆盖），三题均无 code 块终答；同题两轮稳定。
3. prompts 全部落盘 `prompts/loop/` 且为观察式；无 golden 实体名。

后续（不在本交接范围，回 `struct-query-virtual-tables.md` §13 P2 列表）：supervision loop 工具化、fts 表内值发现、数值规整、telemetry；loop 预算本身的调优（28K/12 是否适配 struct 深调查）可随 telemetry 数据再议。

## 6. 验收结果（2026-07-31 当日落地，本节后追加）

**已全部落地并验收。** Commit：`cbae7448`（F2）、`2e23af3a`（F3）、`eb3c36fe`（F1）。

- **单测**：`cargo test -p agent-loop --lib` 272 全绿。新增：C5 闸门 token/rounds/双耗/未耗 4 例（run_synthesis.rs）、code-only 检测器 2 例（answer_contract.rs：四种 code 形态命中 + 五种 prose/空串不误伤）。
- **切片两轮**（`QUESTIONS=86,88,106`，log `fail6_20260731-154420` / `154826`）：三题两轮全 PASS，无 code 块终答、无 `code_block_answer` infra。Q86 正文答出 LPDT-03（round1 artifact 实证）；Q106 两轮正文 2711/2058 字符（允许部分覆盖，实际 correctness=1/0.9）。
- **残留风险（如实记录）**：repair 路径两轮切片均未被真实触发（模型两轮都正常收尾，故障本身是随机尾轮）。曾做强制触发烟测（临时 `if true ||` 强制进 repair，已还原）：因非流式 harness 不透出 Activity 事件且该轮走了 DirectAnswer 未进 synthesis，未能端到端观测 repair 回路。兜底保证：repair 编排与主流式路径共用 `stream_prose_to_sink`（已被 6 次切片行使），检测器与闸门有单测；repair 真触发时的最坏面 = 落 degraded 兜底文案（结构有界）。
- **落地的四个决策**（与属主确认）：D1 严格 code-only 检测（非 parse_llm_output 分类）；D2 repair 插在流结束后、emit Done 前（单 Done，final_message=修复后正文）；D3 repair 再失败落 `contract_violation_fallback` 正文；D4 C5 nudge 按耗尽种类选首句（新增 `budget-exhausted-final-tokens.nudge.md`）。
- **单测基建事实**：`SynthesisPhase` 原零单测，`LlmClient` 为具体 HTTP client、agent-loop 内无 mock（mock server 在 app 集成层）。本轮按「纯函数单测 + 真实切片验证编排」执行，未新建 HTTP mock。
