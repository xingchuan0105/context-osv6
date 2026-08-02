# 交接文档：markitdown 全量基线 · 三层诊断 · 记分对齐 · 无 chunk 硬闸（2026-07-29 → 07-30）

> **部分过期** — 本文以 orchestrator / worker / brief / handoff 架构为当时现状叙述；该架构已于 2026-08-01 物理删除（commit `7f2d182d`），现行为单 agent SaC（见根 `docs/plans/2026-07-30-sac-sdk-single-agent-design.md`）。文中 eval v2 / token 预算等内容仍然有效。（注释添加于 2026-08-02 文档体系梳理）
>
> **部分被取代（2026-08-02）** — 本文「markitdown 唯一文档解析器」决策已由按格式分工取代：PDF→liteparse、Office→office-direct 直读、markitdown 仅文本/代码兜底。见 [`../plans/2026-08-02-parser-pipeline-direct-readers.md`](../plans/2026-08-02-parser-pipeline-direct-readers.md)。本文的硬闸/记分/无 chunk 部分仍有效。

| 项目 | 内容 |
|---|---|
| 类型 | 会话交接（Claude 限额后由 Grok 接手续作） |
| 日期 | 2026-07-29 / 30 |
| 范围 | markitdown 换血后全量 nightly；10 题非 PASS 根因；提示词去重约束删除；eval 标签对齐；无 answer-grade chunk 硬闸；定向复测 |
| 分支 | 本地 `master`（solo trunk；**本批改动尚未 commit**，见 §6） |
| 前序 | `docs/engineering/2026-07-28-handover.md`；Claude 会话 `81642788` / `39f5bf11`（API 限流） |

---

## 0. 一句话总结

Claude 侧完成 **grep/read_lines 替代 doc_scan**、**markitdown 教学 + 静态校验 v1**、**全 10 篇 markitdown 换血** 后启动全量 nightly，限额中断。接手后跑完 nightly 基线 **139/149**；三层诊断后落地三件事：（1）删掉计数去重重约束；（2）**correctness 已过关时不再因 recall=0 贴 RETRIEVAL_MISS**；（3）**无 answer-grade chunk 不得进 Answer**（拦截 + 预算 +2 + 仍无则检索失败答复）。10 题定向复测 **8/10 PASS**；残留 **q088 去重口径**、**q058 Answer 越权补全**。

---

## 1. 接手时的状态（Claude 已交付）

### 1.1 已提交（本地 master）

| Commit | 内容 |
|---|---|
| `5204fc76` | `doc_grep` / `doc_read_lines` 替代 `doc_scan`；词级 0-hit hint；证据平面 chunk 回接；引用覆盖军规相关 |
| `67df00c7` | markitdown 输出契约教学（SKILL）+ codegen 静态格式校验器 v1 |
| 更早 | markitdown 五题换血 PASS、Wave A/B/C agent-loop 重构等（见 git log） |

### 1.2 进行中任务（Claude todos）

| ID | 主题 | 状态（接手时） |
|---|---|---|
| 10–12 | 词级 hint / grep 替代 doc_scan / SKILL 教学 | completed |
| 13 | 全量 markitdown 重灌 + 149 基线 | **in_progress**（nightly 在跑） |
| 14 | markitdown 教学 + 静态校验 | completed（已 commit） |

### 1.3 全量 nightly（markitdown 换血后）

| 项 | 结果 |
|---|---|
| 日志 | `/tmp/nightly_markitdown_20260729.log` |
| 时长 | ~2h49m（10145s） |
| **PASS** | **139 / 149（93.3%）** |
| vs 旧基线 144/149 | **−5** |
| 标签 | PASS 139 · RETRIEVAL_MISS 5 · UNGROUNDED 3 · PARTIAL 1 · INCORRECT 1 |
| 产物 | `e2e_output/rag_eval_v2/v2_20260729-100948` |

**10 题非 PASS（全量时）：**  
q058 PARTIAL · q063/q081 RETRIEVAL_MISS（+`eval_bridge_miss`）· q078/q088 UNGROUNDED · q084 INCORRECT · q091/q099/q116 RETRIEVAL_MISS · q120 UNGROUNDED。

---

## 2. 三层根因诊断（全量 10 题，先理解不改）

详见会话内分析；压缩如下。

### 2.1 范式

- **计数**：模型见多角色同行 → 自建「活动名去重」；Skill 曾写「去重例外 / 两数并陈」反而**合法化去重路径**（后已删）。
- **活动号**：表头 `编号 | … | 活动号=PAC-*`；模型有时把 **编号列** 当活动号（q084 曾报 351）。
- **空/假检索**：零工具或仅 profile 仍写完整答案（q063/q081 全量时）。

### 2.2 上下文工程

- grep 主路径与 **eval RETRIEVAL_TOOLS**（dense/lexical/graph/index）脱节 → 答对仍可能 recall=0。
- SELECTED/★ 采用不稳；web 证据进 cited 不稳（q120 全量时）。
- 双文档 / multi-brief 错误继承。

### 2.3 基础设施

- markitdown PDF 表把 `11大主题域分组` 拆成多 cell → golden 子串失效（q091 等假阴）。
- `eval_bridge_miss`：finalize 后 `tool_results` 缺 dense/hybrid 才报（桥口径窄 + 部分路径未落 store）。
- **`allow_content_early_stop=true` 使 `require_evidence` 形同虚设** → 无 chunk 可 `direct_content` + `skip_synthesis_direct` 绕过 Answer 禁编造条文。

### 2.4 记分不合理点

`label_for` 原逻辑：`recall==0` **优先** RETRIEVAL_MISS，**不看** correctness。  
→ q091/q099/q116 答案与 faith 全对仍 RM（尺子未对齐 markitdown/grep 时代）。

---

## 3. 本批已落地改动（未 commit）

### 3.1 删除计数去重重约束（不加纠正文案）

| 文件 | 变更 |
|---|---|
| `prompts/clusters/codegen/SKILL.md` | 删除「计数语义（去重/两数并陈）」「计数题范式（编号互验/不得自行裁决）」 |
| `prompts/orchestrators/capability-rag.md` | 去掉「编号连续性互验」 |

保留：`total_hits` 是服务端精确数、可用 grep 计数（API 事实，非去重训诫）。

### 3.2 记分对齐

| 文件 | 变更 |
|---|---|
| `tests/rag_quality/src/eval_v2/aggregate.rs` | `recall==0` 仅当答案 **未** 达 τ_c（且非 Correct）时打 RETRIEVAL_MISS；答案已过关则落入 faith/PASS |

单测：`pass_when_answer_correct_despite_recall_zero` 等 25 条 `eval_v2::aggregate::tests` 全绿。

### 3.3 无 answer-grade chunk 硬闸

| 文件 | 变更 |
|---|---|
| `agent-loop/.../exit_policy.rs` | answer-grade 工具集（dense/lexical/graph/index/doc_summary/doc_grep/doc_read_lines）；**排除 doc_profile**；`should_block` 忽略 `allow_content_early_stop`；无 chunk 禁止 `SkipSynthesisUseDirect`；中文 nudge / grace / 失败终局常量 |
| `.../content_dispatch.rs` | 无 chunk → Continue + `NO_CHUNK_CONTINUE_NUDGE` |
| `.../run_retrieval.rs` | 预算打满且无 chunk → **一次性 +2 轮**（`NO_CHUNK_BUDGET_GRACE_ROUNDS`） |
| `.../run_fallback.rs` | 仍无 → 一轮 LLM 写「检索失败请重试」；失败则固定中文拒答 |
| `modes/rag.yaml` / `search.yaml` | `allow_content_early_stop: false` |
| `app-chat/mode_assemble.rs` | worker handoff 同步 `allow_content_early_stop=false` |
| 相关单测 | agent-loop 248 全绿；app-chat mode_assemble / host 相关全绿 |

**硬闸流程：**

```text
想结束 → 有 answer-grade chunk？
  否 → 拦截 + 提示继续检索（不进 Answer）
预算尽且仍无 → +2 轮（仅一次）
+2 后仍无 → fallback → 仍无 → LLM 写检索失败 / 固定拒答
```

**算 chunk：** dense/lexical/graph/index 非空、doc_grep total_hits>0、read_lines 有行、code_execution 带 UUID。  
**不算：** 仅 doc_profile、空 dense、散文 stdout 无 UUID。

---

## 4. 定向复测（10 题）

```bash
# 已执行（worker 先 cargo build -p avrag-worker）
E2E_MODE=nightly E2E_QUESTIONS="58,63,78,81,84,88,91,99,116,120" \
  cargo test -p app --test product_e2e realistic_corpus_full_eval \
  --features product-e2e -- --ignored --test-threads=1 --nocapture
```

| 项 | 结果 |
|---|---|
| 日志 | `/tmp/rerun_nonpass_20260729.log` |
| 产物 | `e2e_output/rag_eval_v2/v2_20260729-151343` |
| 时长 | ~763s |
| **结果** | **8 PASS / 2 非 PASS** |

| 题 | 全量时 | 复测 | 备注 |
|---|---|---|---|
| q063 | RM | **PASS** | 70% 有据 |
| q078 | UNGROUNDED | **PASS** | 81 项 |
| q081 | RM | **PASS** | PAC-20 |
| q084 | INCORRECT | **PASS** | 含 PAC-90 |
| q091 | RM | **PASS** | 11/100/638；recall=0 记分已对齐 |
| q099 | RM | **PASS** | |
| q116 | RM | **PASS** | |
| q120 | UNGROUNDED | **PASS** | |
| **q088** | UNGROUNDED | **INCORRECT** | 仍去重 45/24，真值 59/30 |
| **q058** | PARTIAL | **UNGROUNDED** | corr 0.9，cited 几乎仅标题 |

---

## 5. 残留两题推理诊断（深入）

### 5.1 q088 — 机制对数，口径选错（INCORRECT）

**时间线：** profile → 裸 grep（63 含误命中）→ **表行 grep 得 59/30 且 truncated=false** → read_lines 读到计划阶段（浪费）→ budget 耗尽 → handoff `unique_activity_count: 45/24`。

**关键事实：**

- Observation **已有** `验证阶段表格行 hits: 59` / `发布 30`。
- Handoff/Answer **主数字**仍是去重名；括号附带行数。
- Judge：correctness=0（golden=59/30）；faithfulness=1（自洽说明去重）。

**结论：** 不是检索失败。是 **表语义没建模对**（把名称列相同当成重复）。修法见 §5.4：讲清表格世界模型，不堆「禁止去重」。

### 5.2 q058 — 半残检索 + Answer 越权（UNGROUNDED）

**时间线：** profile 得两 doc_id → **doc_summary 撑满 ADR-0004** → lexical 多词 AND 对 0009 **0 命中** → 仅 profile 标题 → handoff **诚实写 0009 不足** → **Answer 却写出 socket/fd3 等完整 0009 故事**。

**关键事实：**

- 最终 **1 条 citation** = 0009 标题；无 SELECTED。
- Worker handoff 对 0009 承认缺口；Answer **违背「缺口勿用常识补文档事实」**。
- correctness 0.9（内容像对，且与 ADR 原文大体一致）+ faith 0 → **无出处的正确猜测**。
- 已有 0009 section `chunk_id` 时 **轮次预算打尽**，没 fetch 正文 → 见 §5.3。

**结论：** 装载失败 = **轮次预算范式** + 检索策略脆；合成层另有缺口纪律问题。硬闸因有 tool 结果正确放行。

### 5.3 预算范式：应以 token 为主单位（过程结论）

| 维度 | 轮次预算（现状） | Token 预算（更合理） |
|---|---|---|
| 主要约束 | 串行深度 / 槽位占用 | **成本 + 上下文体积** |
| 对 LLM 计费 | 间接 | **直接** |
| 典型失败 | 双文档第二篇装一半（q058） | 单轮塞爆 / 工具 stdout 过长 |
| 角色建议 | 软上限（防死循环） | **主硬预算** |

**v1 已实现**：`max_tokens` 为主停机条件；`max_iterations` 抬高作安全顶；无 chunk grace = `no_chunk_grace_tokens` + 至少 +2 complete。详见 `docs/plans/2026-07-30-token-budget-orchestration.md`。

### 5.4 表格素养：讲世界模型，不讲禁令（过程结论）

q088 在 obs 已有 59/30 后仍主答去重 = 把「中间名称列相同」当成重复。

原则（**2026-07-30 渐进披露**）：

- 正文：`prompts/clusters/codegen/reference/how-to-read-tables.md`  
- 主 SKILL 只留路由表 + `{"skill_request":["codegen/how-to-read-tables"]}`  
- 内容：整行=一条数据；左粗右细；仅全列相同才叫重复（**不讲禁令**）  

---

## 6. 仓库与验证状态

### 6.1 未提交文件（接手时 working tree 在下列改动上）

```
avrag-rs/crates/agent-loop/src/react_loop/iteration/content_dispatch.rs
avrag-rs/crates/agent-loop/src/react_loop/iteration/tests.rs
avrag-rs/crates/agent-loop/src/react_loop/policy/exit_policy.rs
avrag-rs/crates/agent-loop/src/react_loop/run_fallback.rs
avrag-rs/crates/agent-loop/src/react_loop/run_retrieval.rs
avrag-rs/crates/app-chat/src/mode_assemble.rs
avrag-rs/crates/app-chat/src/orchestrator/host.rs
avrag-rs/modes/rag.yaml
avrag-rs/modes/search.yaml
avrag-rs/prompts/clusters/codegen/SKILL.md
avrag-rs/prompts/orchestrators/capability-rag.md
avrag-rs/tests/rag_quality/src/eval_v2/aggregate.rs
```

（+ 本交接文档若一并纳入。）

### 6.2 已跑验证

| 范围 | 结果 |
|---|---|
| `cargo test -p agent-loop --lib` | 248 全绿 |
| `cargo test -p app-chat --lib mode_assemble` / host 相关 | 全绿 |
| `cargo test -p rag_quality --lib eval_v2::aggregate::tests` | 25 全绿 |
| 10 题 nightly 定向 | 8/10 PASS |
| 全量 149 在硬闸后 | **未重跑** |

### 6.3 建议提交信息（供接手人选用）

```
fix(agent-loop,eval,prompts): 无 chunk 硬闸 + 记分对齐 + 去掉计数去重重约束

- require_evidence 下无 answer-grade chunk 禁止进 Answer；预算尽 +2 轮；仍无则检索失败答复
- RETRIEVAL_MISS 不再在答案已过 τ_c 时仅凭 recall=0 优先贴标
- 删除 codegen SKILL 去重/互验训诫（避免合法化去重路径）
- 表格素养渐进披露：`codegen/reference/how-to-read-tables.md` + skill_request `codegen/slug`
```

---

## 7. 遗留与建议优先级

| 优先级 | 项 | 说明 |
|---|---|---|
| **P0** | Commit 本批硬闸/记分/SKILL  diff | 当前仅工作区 |
| **P0** | 全量 nightly 复跑 149 | 硬闸后基线未知；目标对照旧 144/139 |
| **P1** | **预算单位：轮次 → token** | **v1 已落地**（`2026-07-30-token-budget-orchestration.md`）：`max_tokens` 主预算，`max_iterations` 安全顶；无 chunk grace 追加 tokens + 至少 2 complete |
| **P1** | q088 表格素养（渐进披露） | 正文在 `codegen/reference/how-to-read-tables.md`；主 SKILL 仅路由。按需 `{"skill_request":["codegen/how-to-read-tables"]}` |
| **P1** | q058 Answer 缺口门闩 | handoff gaps/insufficient 时禁止为该子题写具体文档事实 |
| **P1** | q058 双文档装载 | 有 section chunk_id 必 fetch；避免多词 AND 致死 |
| **P2** | golden 子串对齐 markitdown 表形态 | 降低 recall 假阴噪声（记分已部分缓解） |
| **P2** | 生产管道 markitdown 全替代 | Claude 原计划步骤 3–4；TableIr 退役边界需单独设计 |
| **P2** | eval_bridge_miss / RETRIEVAL_TOOLS 含 grep | 桥与评测工具集与产品路径对齐 |
| 保持 | 遗留 2 PDF 真表结构（Docling）· 遗留 6 生产重灌 | 见 07-28 交接 |

---

## 8. 关键路径与命令

| 用途 | 路径 / 命令 |
|---|---|
| 本交接 | `avrag-rs/docs/engineering/2026-07-29-markitdown-hard-gate-handover.md` |
| 前序交接 | `avrag-rs/docs/engineering/2026-07-28-handover.md` |
| markitdown/grep 设计 | `avrag-rs/docs/plans/2026-07-29-markitdown-grep-toolcall-spec.md` |
| 硬闸核心 | `crates/agent-loop/src/react_loop/policy/exit_policy.rs` |
| 记分 | `tests/rag_quality/src/eval_v2/aggregate.rs` |
| 全量 nightly 日志 | `/tmp/nightly_markitdown_20260729.log` |
| 10 题复测日志 | `/tmp/rerun_nonpass_20260729.log` |
| 复测产物 | `crates/app/tests/e2e_output/rag_eval_v2/v2_20260729-151343/` |
| e2e 题过滤 | `E2E_QUESTIONS="58,88" … realistic_corpus_full_eval` |
| worker 血泪 | e2e 前必须 `cargo build -p avrag-worker` |

---

## 9. 给接手人的最短路径

1. 读本文件 §0–§5；需要细节时翻 `v2_20260729-151343` 下 q088/q058 的 `mode_debug` / judge。  
2. **先 commit** §6 工作区（或按用户意图拆 commit）。  
3. `cargo build -p avrag-worker` 后 **全量 nightly** 拿硬闸后基线。  
4. 若攻 q088/q058：优先产品契约 / Answer 缺口门闩，**不要**再加长篇「禁止去重」skill。  
5. 生产 markitdown 替换与 TableIr 退役：**单独设计**，勿与评测修复搅在一批。

---

*文档写于 2026-07-30；事实锚点为本地 master `67df00c7` + 上表未提交 diff + 两份 nightly 产物目录。*
