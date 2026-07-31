# struct-query W1 观察窗口记录（2026-07-31）

> 上游：`docs/plans/2026-07-31-struct-query-post-p2-dev-plan.md` W1。纯观察窗口，零代码改动。
> 切片：`QUESTIONS=86,106,113`（86=fts 英文 token 触发候选 / 106=telemetry 延续 / 113=纯中文对抗值查），两轮（LLM 抖动纪律）。
> artifact：`crates/app/tests/e2e_output/rag_eval_v2/v2_20260731-124421`（轮1）/ `v2_20260731-125233`（轮2）；harness：`realistic_corpus_full_eval/q0NN.json`；log：`/tmp/sac_e2e/fail6_20260731-204405.log` 及轮 2 同式。

## 0. 一句话

repair 回路首次被真实行使且有效（106 从 sticky「code 块当答案」→ 两轮 0.9 PASS）；`match_bm25` 两轮 6 题次仍零触发（fts 残留 #1 维持，但 catalog `fts:true` 每题必见）；预算耗尽慢性但不致命（3/6 题次，终答全对）；**W5 中文 fts 维持关闭**（113 纯中文值查走 grep 两轮全对，无真实短板证据）。

## 1. 两轮结果

| 题 | 轮1 | 轮2 | struct 调用（轮1/轮2） | match_bm25 |
|---|---|---|---|---|
| 86 LPDT 首活动 | PASS 1.0 | PASS 1.0 | catalog×1+query×2 / catalog×1+query×2 | 否/否 |
| 106 双数跨 doc | PARTIAL 0.9 | PASS 0.9 | catalog×1+query×1 / catalog×2+**query×5** | 否/否 |
| 113 对抗值查 | PASS 1.0 | PASS 1.0 | catalog×1（无 query）/ catalog×1 | 否/否 |

## 2. 关键观察

### 2.1 repair 首次真实触发（telemetry 核心收获）

- 轮1 Q106 `activity_counts`: `synthesis_code_answer_repair=1` + `sandbox_error=1`——**repair 回路投产以来首次被真实行使**（此前四轮三题全 0）。效果：106 从历史 sticky「末轮 code 块当答案」（correctness=0）转为 PARTIAL 0.9。
- 轮2 Q106 未触发 repair（路径不同：struct_query×5 重取证 + budget_exhausted=1），仍 PASS 0.9。
- 结论：终答契约 F1/F2/F3 在真实路径有效；触发率 1/6 题次，样本仍小，继续积累，**不动 C5 carryover**。

### 2.2 fts 残留 #1：match_bm25 两轮零触发

- 三题模型**每轮都先叫 struct_catalog**（`fts: true` 与 SKILL.md 语法条均可见），但无一使用 match_bm25：
  - 86：普通 SQL（`阶段='概念阶段' AND 角色='LPDT'`）即够——fts 对该题本就非必要；
  - 113：纯中文值查，fts 物理无效（D3）——模型走 grep×7 证伪，答案明确给出「'网络效应''70%'字面均无命中」的缺席证据，**D3 互补设计在真实路径实演成功**。
- 判读：fts 谓词的适格题型（空格分隔 token 的表内值发现）未出现在本题组；残留 #1 维持开放，但已排除「catalog/skill 不可见」成因。后续观察窗口若仍零触发，评估是否在 skill 中给 fts 更明确的适格场景描述（而非加命令式条款）。

### 2.3 预算慢性压力（28K/12 轮）

- `budget_exhausted`：86 两轮皆中（累计 4 连）、106 轮2 中——3/6 题次触顶，**但终答全部正确**（C5 闸门兜底有效）。
- 数据点累计：86/106 是预算敏感题；暂不调 28K/12（纪律：不凭单轮感觉调，继续积累）。

### 2.4 Q106 质变

- 取证模式从「半覆盖」→ struct_query 主取证（轮2 ×5：370 口径 COUNT + 638 grep 交叉），答案结构完整双主张；两轮 0.9 稳定（judge 扣分在「体系化共同特点」阐述深度，非取证缺失）。

## 3. 附带观察（非本线，不动）

- `app-chat/src/chat/mod.rs:32` 编译警告：activity_counts 三函数 unused-import——但遥测数据实际正常透出（另一条路径），疑似 SaC 在途线重构残留，归 SaC 线自查。

## 4. 结论与后续

- **W1 gate 达成**：≥2 轮切片，tool_trace / activity_counts / budget 数据在案。
- **W5 维持关闭**：113 两轮证明 grep 对中文值查无短板（符合「不过度归整」steer）。
- 下一窗口：W2 S4 ingestion 挂接（硬前置：生产 parser markitdown 化另案状态确认）或 W3 A5 补测（小，可先插）。
