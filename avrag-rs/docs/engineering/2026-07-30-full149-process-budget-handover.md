# 交接文档：token 主预算落地 · 全量 149 · 非 PASS 过程诊断（2026-07-30）

> **部分过期** — 本文以 orchestrator / worker / brief / handoff 架构为当时现状叙述；该架构已于 2026-08-01 物理删除（commit `7f2d182d`），现行为单 agent SaC（见根 `docs/plans/2026-07-30-sac-sdk-single-agent-design.md`）。文中 eval v2 / token 预算等内容仍然有效。（注释添加于 2026-08-02 文档体系梳理）

| 项目 | 内容 |
|---|---|
| 类型 | 会话交接（人话优先，附路径/数字） |
| 日期 | 2026-07-30 |
| 范围 | 硬闸+记分+token 预算+表格素养 commit；q058/q088 定向复测；**不灌库全量 149**；14 题非 PASS 内容/过程根因；预算三层与 handoff/codegen 结论 |
| 分支 | 本地 `master`（solo trunk；**未 push**） |
| 前序 | `docs/engineering/2026-07-29-markitdown-hard-gate-handover.md`；token 编排计划 `docs/plans/2026-07-30-token-budget-orchestration.md` |

---

## 0. 一句话总结

本批已 commit **`0e925c1f`**（硬闸 / 记分对齐 / token 主预算 / 表格素养渐进披露）。定向复测 **q058+q088 = 双 PASS**；不灌库全量 **v2 PASS 135/149（90.6%）**，较前夜 markitdown 基线 139/149 略降。非 PASS **14 题**主因不是「token 没接上」，而是：**（1）通道预算 10 与单 brief 12 打架 → 第一趟后锁门无法补洞；（2）handoff 验收过松（代码半成品也算交货）；（3）表格计数仍去重；（4）少数检索错段/拒答极性/空检索软提示**。

---

## 1. 已落地代码（commit）

| Commit | 摘要 |
|---|---|
| **`0e925c1f`** | `fix(agent-loop,eval,prompts): 无 chunk 硬闸 + 记分对齐 + token 主预算 + 表格素养渐进披露` |

要点：

- **硬闸**：`require_evidence` 下无 answer-grade chunk 禁止进 Answer；触顶可 grace（tokens + ≥2 complete）；仍无则检索失败答复。
- **记分**：答案 correctness 已过 τ 时，不再仅凭 recall=0 优先贴 `RETRIEVAL_MISS`。
- **预算**：`BudgetConfig.max_tokens` 主停机，`max_iterations` 安全顶；`<loop_budget … tokens_*>` 注入。
- **表格**：`codegen/reference/how-to-read-tables.md` + `skill_request codegen/how-to-read-tables`（**选修**，非默认加载）。
- 文档：`2026-07-29-markitdown-hard-gate-handover.md`、`plans/2026-07-30-token-budget-orchestration.md`。

**未在本 commit 修**：通道 `CHANNEL_ITERATION_CAP` 与 `max_iterations=12` 不一致（见 §5）。

---

## 2. 评测结果

### 2.1 定向复测（先 C 后 D）

```bash
E2E_MODE=nightly E2E_QUESTIONS="58,88" cargo test -p app --test product_e2e \
  realistic_corpus_full_eval --features product-e2e -- --ignored --test-threads=1 --nocapture
```

| 题 | 先前（v2_20260729-151343） | 定向（v2_20260730-061052） |
|---|---|---|
| q058 | UNGROUNDED | **PASS**（双 ADR 有实质 cite） |
| q088 | INCORRECT 45/24 | **PASS** 59/30 |

- log：`/tmp/rerun_q058_q088_20260730.log`
- 结论：**同一 commit 下可 PASS，全量仍抖**（见下）。

### 2.2 全量 149（不灌库，reuse workspace）

```bash
# 未设 E2E_FORCE_INGEST
E2E_MODE=nightly cargo test -p app --test product_e2e realistic_corpus_full_eval \
  --features product-e2e -- --ignored --test-threads=1 --nocapture
```

| 项 | 结果 |
|---|---|
| 日志 | `/tmp/nightly_full149_20260730.log` |
| 心跳 | `/tmp/nightly_full149_20260730.hb`（每题一行） |
| 时长 | ~133 min（test ~7972s） |
| **v2 PASS** | **135 / 149（90.6%）** |
| judge | ok=149 error=0 |
| 均值 | C≈0.934 · F≈0.969 · R≈0.983 · recall≈0.90 |
| 产物 | `crates/app/tests/e2e_output/rag_eval_v2/v2_20260730-062908` |
| 对照 | 前夜 markitdown full **139/149** → 本轮 **−4** |

**v2 标签：** PASS 135 · RETRIEVAL_MISS 6 · PARTIAL 4 · UNGROUNDED 2 · REFUSAL_WRONG 2。

> 控制台 legacy 行仍可能印 `GENERATION_UNGROUNDED` 等；**以 v2 为准**（ADR-0012）。

### 2.3 14 题非 PASS 清单

| # | subset | label | 一句话 |
|---|---|---|---|
| 17 | thesis_synthesis | RM | 错章节；正确 grep 停在未执行 handoff |
| 18 | thesis_synthesis | RM | 反应策略答成另一组三条 |
| 42 | thesis_adversarial | RW | 应拒「未提访谈人数」却答 4 人（编制当访谈） |
| **58** | cross_adr | **PARTIAL** | 0009 满、0004 仅标题（定向曾 PASS） |
| 65 | consulting_factual | RM | Salesforce 23% 未命中，早拒 |
| 86 | ipd_table | RM | LPDT 第一个答成 LPDT-04 非 LPDT-03 |
| **88** | ipd_table | **UNGROUNDED** | 已数出 59/30，代码去重成 45/24（定向曾 PASS） |
| 100–107 | cross_document 等 | RM/PARTIAL | **双源半载** 为主 |
| 115 | orchestrator_paradigm | RW | 日期在证据里却过度拒答 |
| 121 | rag_search_joint | UG | web 扩写超 cite；口径与金标时间表述有张力 |

另：q106 等出现 **`eval_bridge_miss`**（store 侧缺 dense/hybrid 痕迹；真实过程多在 codegen 沙箱）。

---

## 3. 根因：内容层（说什么错了）

压缩五簇：

| 簇 | 题 | 人话 |
|---|---|---|
| **检索错位** | 17,18,65,86 | 库里有答案，命中附近错段/错文档/错行 |
| **双文档半载** | 58,100,101,105,107 | 一侧做满就交差，另一侧空或只有标题 |
| **表格去重** | 88 | 证据够，中间代码把正确行数改成「活动名去重」 |
| **拒答极性** | 42,115 | 该拒却答 / 该答却拒 |
| **桥接/联合** | 106,121 | 无 dense 终局或 web 细表压过文档纪律 |

**不是**「硬闸把系统打崩」：mean C/F 仍高；多数非 PASS 的 F 不差，是 **没搜全 / 数错 / 拒错**。

---

## 4. 根因：执行过程（怎么跑坏的）

数据：`e2e_output/realistic_corpus_full_eval/q*.json` → `mode_debug.general`。

### 4.1 过程故障统计（14 题）

| 故障 | 约占比 | 人话 |
|---|---|---|
| 第 2 次 rag dispatch 被拒（channel budget） | **8/14** | 想再派补洞，通道说锁门交差 |
| handoff 仍是未跑代码 / 半成品 | **6/14** | 交接条是「下一段 Python」不是发现 |
| code_gen_error（沙箱 Traceback） | **5/14** | 代码炸了，轮次白耗 |
| 空检索轮仍前进/收工 | ≥3 | 0 条后仍写 handoff |

### 4.2 典型坏路径

```
编排派 rag brief
  → worker 多轮 codegen（真检索在沙箱里）
  → 半成功 / 空读 / Traceback
  → 交出散文或未跑代码当 handoff（compiler 也收）
编排想第二 brief 补另一源
  → channel budget exhausted → 强制 finish
Answer 用半截材料成文 → Judge 非 PASS
```

### 4.3 q088 最干净的过程证据

1. 沙箱已打出 **`total_hits` 验证 59 / 发布 30**  
2. 下一轮代码 **主动去重** → 45 / 24  
3. handoff 写 `coverage: full`、`gaps: []`  
4. 最终答案 45/24  

→ **不是检索失败，是过程里把正确计数改坏，还自称全覆盖。**

---

## 5. 预算 / token / handoff / skill / 空检索（人话结论）

### 5.1 三本账

| 层 | 配置 | 作用 |
|---|---|---|
| rag loop YAML | **12 轮** + **28000 token** + grace 10000 | 单次 retrieve 停机（token 已实现） |
| search YAML | 8 轮 / 16000 token | 同上 |
| **通道** | `CHANNEL_ITERATION_CAP = **10**` | 每通道整轮总轮次；**第二 brief 能否派** |

Worker 单 brief：`effective = min(yaml.max_iterations, 通道剩余)`。  
当前 **yaml 12 > cap 10** → 第一 brief **一律 cap_clamped** → 结束后 **SEAL** → 第二 brief 几乎总被拒。

> 这是「半载补不了」的 **执行层主因**，比「模型懒」更硬。

Token 主单元：**loop 内已落地**；**通道 re-dispatch 仍只看轮 cap**，且与 12 冲突。

### 5.2 Handoff 为何烂

Compiler（K3）故意放宽：

- 散文、甚至 **裸 Python 代码块** → **合法 handoff**  
- 几乎只硬拦：声明 `coverage=insufficient` 且 **本轮零工具调用**（E105）  
- **不校验** `coverage=full` 是否真覆盖目标  

→ 半成品代码、假 full 都能出站。

### 5.3 Codegen error

沙箱 Traceback：签名错、id 截断、`asyncio.run` 等。  
有 `[sandbox_error]` 提示，**不强制**修好；错误轮仍耗预算。

### 5.4 Skill

- **codegen 主技能**：mandatory，写得清楚（含 `grep.total_hits`、勿瞎统计）。  
- **表格素养**：`how-to-read-tables` **选修**，全量几乎不 `skill_request`。  
- 模型仍可无视 total_hits 自己去重 → skill **文字有、默认不念、执行不钉死**。

### 5.5 空检索为何还继续

有 `[retrieval_summary] … 0 条` 和 `[no_output]`，但 no_output 文案含 **「否则可直接去 handoff」** 类怂恿；  
rag codegen **没有** search 侧那种「连续两次空强制停」的硬规则；  
硬闸只保证「整 loop 零 answer-grade chunk 不能进 Answer」，**半吊子有痕就能收工**。

---

## 6. 建议优先级（未实现，仅交接）

| 优先级 | 动作 | 预期收益 |
|---|---|---|
| **P0** | 修 **CAP 10 vs per_brief 12**：对齐数字，或 **cap_clamped 时不要 SEAL**（只截断本 brief） | 恢复第二 brief 补洞；打 B 簇半载 |
| **P0** | Handoff：**禁止未执行 code 作 final**；假 `full` 可降级/机检 | 减少半成品交接 |
| **P0** | 表格计数：默认披露 how-to-read-tables，或 **强制信任 total_hits、禁二次去重** | 稳住 q088 |
| **P1** | 空检索 / code_gen_error 连续 N 次 → 强制 gaps 或换策略 nudge（改掉 proceed-to-handoff 怂恿） | 17/106 类 |
| **P1** | 拒答极性（42/115）、factoid 错段（65/86） | 定向题 |
| **P2** | eval bridge 与 codegen 工具痕迹对齐 | 少假 RM / bridge_miss |
| **P2** | 评测松紧（q105 双读法、q121 时间表述） | 少误杀 |

**最小第一刀**：只动 `WorkerSession` 的 seal/cap 逻辑或 `CHANNEL_ITERATION_CAP`，验证 dual-doc 题是否回升。

---

## 7. 关键路径速查

| 用途 | 路径 |
|---|---|
| 本批 commit | `0e925c1f` |
| 全量 log | `/tmp/nightly_full149_20260730.log` |
| 全量 v2 产物 | `avrag-rs/crates/app/tests/e2e_output/rag_eval_v2/v2_20260730-062908` |
| 题级 dump（含 mode_debug） | `…/e2e_output/realistic_corpus_full_eval/qNNN.json` |
| 定向 58/88 | `…/v2_20260730-061052` · `/tmp/rerun_q058_q088_20260730.log` |
| 预算 YAML | `modes/rag.yaml` `modes/search.yaml` |
| 通道 cap | `crates/app-chat/src/orchestrator/worker_session.rs` → `CHANNEL_ITERATION_CAP` |
| token 停机 | `crates/agent-loop/src/react_loop/run_retrieval.rs` |
| handoff 编译 | `crates/agent-loop/src/output_compiler/handoff.rs` |
| 空轮反馈 | `crates/agent-loop/src/react_loop/iteration_codegen.rs` → `format_codegen_observation` |
| 表格选修 | `prompts/clusters/codegen/reference/how-to-read-tables.md` |

复跑全量（不灌库）：

```bash
cd avrag-rs
cargo build -p avrag-worker
E2E_MODE=nightly cargo test -p app --test product_e2e realistic_corpus_full_eval \
  --features product-e2e -- --ignored --test-threads=1 --nocapture \
  2>&1 | tee /tmp/nightly_full149_YYYYMMDD.log
```

定向：

```bash
E2E_MODE=nightly E2E_QUESTIONS="58,88,100,101,105,106,107" \
  cargo test -p app --test product_e2e realistic_corpus_full_eval \
  --features product-e2e -- --ignored --test-threads=1 --nocapture
```

---

## 8. 接手检查清单

- [ ] 读本文件 §0–§5；需要细节再下钻 §7 路径  
- [ ] `git log -1` 确认 `0e925c1f` 在 master  
- [ ] **不要**默认 `E2E_FORCE_INGEST` / 盲目 docker-compose（见 AGENTS.md）  
- [ ] 若改 cap/seal：先单测 `worker_session` + 定向 cross_document + q058/q088  
- [ ] 声称结构变更后同会话 `graphify update .`（勿提交 graphify-out）  
- [ ] Solo：本地 commit 即可；用户未要求则不 push/PR  

---

## 9. 与前序交接的关系

| 文档 | 关系 |
|---|---|
| `2026-07-29-markitdown-hard-gate-handover.md` | 硬闸/记分/10 题复测；残留 q058/q088 叙事 |
| **本文** | 硬闸已 commit；全量 149 新基线；**过程+预算三层**才是当前主矛盾 |
| `plans/2026-07-30-token-budget-orchestration.md` | token 设计意图；落地了 loop，**未同步通道 cap** |

---

*完。下一棒默认：P0 修通道 seal/cap，再定向 dual-doc + q088。*
