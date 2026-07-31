# loop 终答契约修复完成交接（2026-07-31）

> 上游：`docs/plans/2026-07-31-loop-terminal-answer-handoff.md`（确诊+修法，其 §6 已附验收结果）。
> 本文档 = F1/F2/F3 **落地后**的窗口交接：改了什么、怎么验的、残留风险、下一个窗口从哪开工。接手不需要原会话上下文。

## 0. 一句话现状

「末轮 code 块当终答」三个修法（F1 prose repair / F2 C5 token 闸门 / F3 harness infra 归类）已全部落地、验收并提交（4 commit）；struct_query 线与 loop 终答契约线均收官。下一窗口回 `struct-query-virtual-tables.md` §13 第 6 项：**P2 = supervision loop 工具化 / fts 表内值发现 / 数值规整 / telemetry**。

## 1. 本窗口产出（4 commit，hunk 级挑拣，未混入 SaC/2b 任何行）

| commit | 内容 | 关键文件 |
|---|---|---|
| `cbae7448` | **F2**：C5 收尾观察覆盖 token 耗尽 | `run_retrieval.rs`（新 `BudgetExhaustion{rounds,tokens}`，loop 返回 tuple 第 5 元）、`mod.rs`、`run_synthesis.rs`（闸门改 `exhaustion.any()`）、`prompt_assets.rs`、`prompts/loop/budget-exhausted-final-tokens.nudge.md`（新） |
| `2e23af3a` | **F3**：harness code-only 终答记 infra | `crates/app/tests/product_e2e/llm_real/rag_quality_prod.rs`（`is_code_only_answer` + `record_infra("code_block_answer")`，紧跟 empty_answer 检查之后） |
| `eb3c36fe` | **F1**：prose_only synthesis code-only repair 轮 | `synthesis.rs`（流式核心抽 `stream_prose_to_sink`；检测插在流结束后、emit `Done` 前）、`answer_contract.rs`（`is_code_only_answer` pub(crate) + 单测）、`prompts/loop/synthesis-prose-repair.nudge.md`（新） |
| `d15eb4b7` | 交接文档 §6 验收结果 | `docs/plans/2026-07-31-loop-terminal-answer-handoff.md` |

四个已确认的决策：**D1** 严格 code-only 检测（剥掉 `<code>`/任意围栏后无正文才算；散文引用一段 SQL 不触发——不复用 `parse_llm_output` 的宽松分类）；**D2** repair 插在流结束后、emit `Done` 前（单 `Done`，`final_message`=修复后正文；code 块会先流过屏，与 JSON fallback 梯队同款 UX）；**D3** repair 仍 code-only → `contract_violation_fallback` 兜底（「无 code 块终答」是结构保证不是概率）；**D4** C5 nudge 按耗尽种类选首句（rounds 用旧文件、token-only 用新文件、双耗用 rounds 版）。

两个新 prompt 均为第三人称观察式、无 golden 实体名；`prompts/loop/README.md` 已各加一行（注意：该文件还有 SaC 未提交 hunks，见 §4）。

## 2. 验证证据

- **单测**：`cargo test -p agent-loop --lib` **272 全绿**。新增 6 例：C5 闸门 4（rounds/token-only/双耗/未耗）、检测器 2（四种 code 形态命中 + 散文引用围栏/inline `<code>`/空串不误伤）。
- **真实切片两轮**（`QUESTIONS=86,88,106`，log `/tmp/sac_e2e/fail6_20260731-154420.log` / `154826.log`）：三题两轮全 PASS，零 code 块终答、零 `code_block_answer` infra。Q86 round1 artifact 实证正文答出 LPDT-03；Q106 两轮正文 2711/2058 字符（correctness=1/0.9）。artifact 新鲜度已按 mtime 核过（失败题不覆写的老坑）。
- **graphify 已重建**（227373 nodes）。

## 3. 残留风险与观测点（下一窗口要知道的）

1. **repair 回路从未被真实 LLM 触发过**。两轮切片模型都正常收尾（故障本身是随机尾轮）。曾做强制触发烟测（临时 `if true||` 强制进 repair，已还原复测）：非流式 harness 不透出 Activity 事件、且该轮走 DirectAnswer 未进 synthesis，端到端观测未果。缓释：repair 与主路径共用 `stream_prose_to_sink`（已被 6 次切片行使）、检测器/闸门有单测、repair 失败最坏面 = degraded 兜底文案（结构有界）。**观测钩子已就位**：Activity stage `synthesis_code_answer_repair` / `synthesis_code_answer_violation` + harness `code_block_answer` infra 计数——P2 telemetry 项应把这两个 stage 接进指标，用真实触发率决定是否需要第二轮加固（如 C5 carryover 增强、预算调优）。
2. **`cargo check -p app --tests`（lib 测试目标）编译失败，非本窗口引入**：SaC 在途改动给 `contracts::ChatRequest` 加了 `capabilities`/`client_context`/`client_ip` 字段，`app/src/lib_impl/tests.rs:433` 未跟上（E0063）。集成测试目标 `cargo check -p app --test product_e2e` 正常，切片不受影响。SaC 线收尾时自行补字段。
3. **预算调优问题悬置**：28K token / 12 轮是否适配 struct 深调查，等 telemetry 数据（同 §3.1 的钩子），别凭切片单轮感觉调。

## 4. 环境/操作要点

- **脏树依旧严重**（SaC 线在途）：`agent-loop/src/react_loop/{assembler.rs, policy/*, skill_request.rs}`、`app-chat/**`、`agent-tools/**`、`guardrails/**`、`modes/*.yaml`、`prompts/clusters/**`、`prompts/loop/budget-exhausted-final.nudge.md`、`prompts/loop/README.md`（**还有未提交的 SaC hunks**，本窗口只挑走了自己的两行）等。**edit 前以当前文件内容为准；commit 只挑自己 hunk，绝不 `git add -A`**（本窗口做法：整文件 `git add` + 共享文件手写 `git apply --cached` 拆 hunk）。
- **2b 线未提交文件**（与本窗口正交，别混进后续 commit）：`crates/rag-core/src/runtime/tools/struct_query.rs`、`tests/rag_quality/src/harness_extract.rs`、`crates/storage-pg/src/lib_impl/repository_assets.rs`、`scripts/struct_query_poc/`、`docs/plans/2026-07-31-struct-query-virtual-tables.md`、`docs/plans/2026-07-31-struct-query-p1c-handoff.md`。
- 构建纪律：`CARGO_BUILD_JOBS=2`，同一时刻只跑一个 cargo。
- 验证命令：
  ```bash
  cd /home/chuan/context-osv6/avrag-rs
  CARGO_BUILD_JOBS=2 cargo test -p agent-loop --lib
  # 真实 LLM 切片（约 2.5~4 分钟/3 题 + judge）：
  cd /home/chuan/context-osv6
  CARGO_BUILD_JOBS=2 STRUCT_STORE_DIR=$PWD/avrag-rs/storage/struct_store \
    QUESTIONS=86,88,106 bash avrag-rs/scripts/sac-skill-fail6-reg.sh
  ```
- **LLM 抖动**：deepseek-v4-flash 同题不同轮路径不同（本窗口 Q86 round2 recall@15=0 但 judge 仍 PASS）；同题至少两轮再定论。artifact 读取先看 mtime（失败题不覆写 `q0NN.json`）。
- 修结构性代码后跑 `graphify update .`；`graphify-out/` 不入库。

## 5. 下一步（P2 队列，建议顺序）

回 `struct-query-virtual-tables.md` §13 第 6 项，建议按此序：

1. **telemetry**（建议先做）：把 §3.1 的两个 Activity stage + harness infra 计数接成可查指标——它是其余三项（含预算调优）的决策依据，且本窗口刚把钩子埋好。
2. **supervision loop 工具化**（P1b 的 6 工具薄 loop 产品化）。
3. **fts 表内值发现**。
4. **数值规整**。

各项验收口径以 `struct-query-virtual-tables.md` 为准。
