# SkillOpt 分层训练改造 · 实施方案（2026-08-02）

> 本文档是开发契约：新窗口按此实施，发起人按 §7 验收清单验收。
> 背景是「多 Prompt 组合 + 渐进式披露」架构的 SkillOpt 训练改造。所有"现状事实"均已核实到文件:行号；实施时若与代码不符，以代码为准并在 §8 追加偏差记录。
> 目标产物：`tools/skillopt` 从"单文件全局训练"升级为「分层训练 + 分步代理信号 + 高并发 + 防过拟合 + 双模型分离」的训练系统，验收口径不变（黄金集 149 PASS）。

---

## 1. 背景（业务语言摘要）

avrag-rs 是「多 Prompt 组合 + 渐进式披露」架构：运行时由 `agent-base.md`（系统基座）+ 能力/集群 SKILL.md + `loop/*.md` 观察注入 + 终答质检台（`check_final_answer` 规则卡）拼装而成，披露由 `DisclosurePlanner`（代码驱动）按阶段决定。

当前 SkillOpt（`tools/skillopt`）把 **`prompts/system/agent-base.md` 单文件当可训练 skill**，用黄金集 149 全量真实评测当评分器。存在四个问题：

1. **训练梯度 ≠ 验收指标**：黄金集是"综合结果"验收集，一次 PASS 是检索/代码/停点/合成全部做对的求和，信号无法归因，且单 worker 全栈评测一轮迭代几分钟。
2. **组合系统被当单文件训**：每题实际拼装 2~5 个 skill 文件，reflect 却只能编辑一个文件，组合失败无法归因。
3. **行为层（停/继续/降级）不可训**：失败大头是"该停没停/不该停早停/编造"，架构已把停决策交给 skill（`exit_policy.rs`：`stop/grounding is model+skill owned`），却没有训练它的机制。
4. **过拟合无防线**：optimizer LLM 通过 `gradient/reflect.py:160` 的 `#### Hidden Reference` 直接看到黄金集参考答案，存在把答案写进 skill 的泄漏通道。

**实证（层优先级依据）**：
- 三次全量 PASS：122 → 123 → 127（`v2_20260801-082506`/`094826` 旧标准，`v2_20260802-045319` 新标准）。
- run3 的 14 个真实失败按 `label_for`（`tests/rag_quality/src/eval_v2/aggregate.rs:81-152`）判定归因：
  - **L2.5 停/grounding**（UNGROUNDED 编造 + PARTIAL 答不全 + REFUSAL_WRONG 拒答）= **8/14 (57%)**
  - **L3 选择/合成**（SELECTION_MISS，recall>0 但 cited_gold=0）= **4/14 (29%)**
  - **L2 检索**（RETRIEVAL_MISS，recall=0）= **2/14 (14%)**
- 仓库行为报告（`docs/engineering/2026-08-02-golden149-llm-behavior-report.md`）的 A–E 五类行为与之互证：A/B=代码层（零检索直答、代码块即终答、方法名幻觉 `client.docsummary` vs `doc_summary`）、C=loop 控制+选择（表类题检索过载 119.8 次/题 vs PASS 37.8、预算烧尽、recall=1 但 cite miss）、D=路由披露缺口（纯 chat 三件套无 prompt 教导）、E=基础设施（judge 尾逗号，已修复）。

**结论**：训练优先序 = **L2.5（停与答）> L3（选择/合成）> L2（检索）> L1.5/L1**。检索层最健康，行为层最大金矿。

---

## 2. 核心设计决策

### D1 层栈（7 层 + 1 层不训）

| 层 | 名称 | 可训对象 | 训练目标 | 判定信号 |
|---|---|---|---|---|
| L0 | 组合粘合层 | `prompts/system/agent-base.md` | 每题在场的编排/基座 | 全局聚合 |
| L1 | 路由 | SKILL description + base 的 skill_request 引导 | 该触发/不该触发 | 探针对 precision/recall（需轨迹） |
| L1.5 | 代码层（SAC 特有） | base 沙箱基座节 + `loop/codegen-*.nudge.md` | 写对代码：能跑、API 对、合契约 | sandbox error / no_output / 方法表合规 |
| L2 | 查询/动作 | 能力 SKILL.md（knowledge-base/web） | 写对查询：召回 source_chunks | `RETRIEVAL_MISS`（recall=0）+ recall 列 |
| **L2.5** | **LOOP 控制（行为）** | base"事实与不确定"节 + `loop/degraded-no-evidence-*`/`partial-evidence-insufficient`/`retrieval-failed-final`/`budget-exhausted-*` | **停/继续/降级/budget 收尾——控行为不控结果** | UNGROUNDED/PARTIAL/REFUSAL_WRONG + 停点证据覆盖度 |
| L3 | 合成（内容） | 写作/格式 cluster SKILL + 终答合成逻辑 | 答对 + 引用对证据 | SELECTION_MISS（cited_gold=0）+ correctness/faithfulness |
| L3b | 引用 | SKILL.md 引用触发措辞 | 对时刻读对 reference | 引用加载命中率 + 端到端 |
| （形态） | 终答形态 | **不训** | — | 代码质检台（`check_final_answer` 规则卡）已接管 |

**形态不训的依据**：终答形态（code_only/host_shell/template_artifact/executable_code）是确定性检查，WP1–WP3 已数据驱动化（规则卡 + `host_markers.rs` 印章备案制）。训它既白费又危险——形态归代码质检，grounding/停点归 skill（`docs/plans/2026-08-02-final-answer-checkpoint-impl.md` §3 非目标"证据充分性 skill-owned"）。

### D2 训练目标合并（不是 6 个平行循环）

| 训练目标 | 合并的层 | 失败覆盖 | 信号 |
|---|---|---|---|
| **停与答**（先训） | L2.5 + L3 内容 | 86% | UNGROUNDED/PARTIAL/SELECTION_MISS + 停点覆盖 + cited_gold |
| **检索面** | L1.5 + L2 | 14% + 代码类 | recall + sandbox error（代码/查询分离需轨迹） |
| **引用面** | L3b | 表/跨文档类 | reference 加载命中 |

L1 路由最后训（代码驱动 mandatory + 失败最少，仅 skill_request 路径可训）。

### D3 测试分层（测试题与测试标准都分层，但沿两个不同轴）

三个测试角色不可混淆：

| 角色 | 用途 | 放什么题 | 标准 |
|---|---|---|---|
| ① 训练信号 | in-loop 评分 | 只隔离该层参数的纯 subset | 该层专用指标 |
| ② 层回归门 | 训完看"没改坏自己" | 同层 subset 族 held-out（per-family split） | 同层指标 |
| ③ 组合验收 | 产品级契约 | 组合 subset（`cross_document`/`ipd_table`/`rag_search_joint`/`cross_adr`）+ 全量 149 | **现有 hard/soft / PASS 口径，不变** |

- **测试题沿"参数轴"分**：纯 subset 训、组合 subset 留 ③、L1 探针对外挂、L3b 边缘题。
- **标准沿"指标轴"分**：hard/soft 在 L2 和组合层同一定义；L1 用路由 precision/recall；L3b 用加载命中率。**三个指标分开报，永不合并**（"没触发" vs "触发了但做不好"是两个故障）。
- **验收口径不动**：分层是开发期诊断，不是替代验收。

### D4 双评价对象：黄金集 = 验收，分步代理信号 = 训练梯度

黄金集是"综合结果"，不适合当训练梯度（慢 + 复合）。但 `per_query.tsv` 已产出分步信号，且 `label_for` 判定已编码层信息：

- **recall=0** → L2/L1.5 检索面
- **recall>0 且 cited_gold=0** → L3 纯选择失败（`SELECTION_MISS` 判定自带）
- **faithfulness<τ 且有编造** → L2.5 grounding（`UNGROUNDED`）
- **sandbox_error / no_output** → L1.5 代码层（确定性、无需 judge）

**不需要先做轨迹就能分层归因**；轨迹只做更细的第二刀（过早停 vs 合成不全、代码错 vs 查询错）。

### D5 高并发快速迭代架构

三个杠杆（按性价比）：

1. **训练评分器用分步列 + 并行化现有评测**：`E2E_QUESTIONS` 已支持任意子集，一次 rollout 只跑该层相关题。
2. **去掉 prompts 共享树互斥（串行根因）**：当前 rollout 靠"swap 共享 prompts 文件 → 评测 → 恢复"（`runner.py::SwapPromptFile`），只能单 worker。改 **per-worker 进程通过环境变量/独立目录注入 skill 内容**（E2E 强制 `PROMPT_DIR` 指向真实树需加 skillopt 豁免通道）。并发瓶颈从"prompts 互斥"变"Milvus/LLM 吞吐"。
3. **冻结非训练层**：训 L2.5/L3 时缓存每题检索结果（一次全量检索后缓存 chunks），只重放 LLM loop；训代码层用 mock 环境（`mock_rag_codegen.rs`/`mock_llm_server.rs` 已存在），完全不碰 Milvus。

### D6 防过拟合五道防线（D6 为红线，不可协商）

| # | 防线 | 机制 | 类型 |
|---|---|---|---|
| ① | 切断 ground truth → optimizer 通道 | rollout 的 `reference_text` 不再填 `ground_truth`（评分在宿主侧完成，optimizer 只需 query+answer+score+检索指标）；同时清 `envs/base.py::build_reference_text` | 硬机制 |
| ② | 结构性 holdout | 整 subset 跨语料留出（`new_corpus_factual`/`baiyao_pdf`/`cross_document`）——留出题的答案永不进训练 → 记忆化无处命中；随机 7:2:1 同分布会骗过 gate | 硬机制 |
| ③ | 记忆化扫描器进 gate | 对候选 skill 的**编辑增量**（相对 `skill_init` diff）扫描 golden 全量 `expected_answers`+queries+`source_chunks`：精确子串（低阈值拒）+ 模糊匹配（n-gram/embedding，高阈值拒）；命中写拒绝缓存当负反馈。机制化 README 的人工"逐字审查"红线 | 检测器 |
| ④ | train-val gap + 泛化探针 | 每 epoch 记 train/val hard，gap 拉大报警；训练后强制对全量 149（含留出语料）跑 best_skill，按 subset 报 delta | 评估信号 |
| ⑤ | optimizer prompt 反记忆化指令 | `avrag149/prompts/analyst_*.md` 加硬规则："提取跨题可复用通用规则；禁止写入具体题目、实体名、数字、参考答案、引文片段；依赖具体事实的编辑改写为抽象规则或丢弃" | 软约束（须配 ①） |

**记忆化风险最高在"停与答"（L2.5+L3）**——gold 答案/期望内容与这两层最相关；检索面风险低但仍过同一检测器。检测器扫 diff 而非整 skill（初始是已知干净的产品 prompt）。

### D7 双模型分离：答题 DeepSeek（生产一致）/ 评估改写 coding agent（可插拔）

**答题（rollout）已满足**：rollout 跑生产评测链（`runner.py` `cargo test realistic_corpus_full_eval`，继承 `.env`），target = `E2E_LLM_*`/`AGENT_LLM_*` = `deepseek-v4-flash`。训练即生产环境，零改动。

**打分（judge）保持 DeepSeek**：hard/soft 由 rollout 的 DeepSeek judge 判定，决定 PASS，必须与生产一致。

**评估/改写（reflect）换 coding agent 代理**：skillopt 官方扩展点 `EnvAdapter.reflect()`（`envs/base.py:234` 注释 "override only if your environment needs custom reflection logic"）。覆盖 `Avrag149Adapter.reflect()`，把"失败归因 + 编辑生成"换成 coding agent：

1. 整理 minibatch 轨迹（conversation.json + rollouts.json）进工作目录；
2. 子进程调 coding agent（`claude -p` 或专用 `reflect_agent.py`），任务 = 读轨迹 → 归因失败 → 产出 skill 编辑建议；
3. 解析 agent 输出 → 转 `RawPatch` 结构（`{"patch": {"reasoning","edits":[...]}, "source_type": "failure"|"success"}`，`reflect.py:356`）→ 下游 apply/gate 不变。

配置：`reflect_backend = llm | coding_agent`（默认 llm，逐层切换）。复杂层（停与答）用 coding agent，简单层（检索面）保持裸 LLM。

约束：
- coding agent 产出**编辑建议**不直接写文件（写文件归 `apply_patch_with_report`：合并/去重/edit_budget 截断）。
- **反记忆化防线对 coding agent 同样生效且更重要**——`ground_truth` 必须彻底排除在 agent 工作目录外（D6-①）；记忆化扫描器照挂（D6-③）。agent 比裸 LLM 更能读库、更能抄答案。
- coding agent 子进程天然可并行（契合 WP2 高并发），注意成本：一次 agentic run 比单次 LLM 调用贵一个数量级。

---

## 3. 工作包（依赖序）

### WP0 前置：分步评分器拆层（快赢，零新数据）

- `tools/skillopt/avrag149/runner.py::score_row` 支持按层取信号：检索层用 recall、合成层用 correctness/faithfulness、代码层用 sandbox_error/no_output。
- `train_avrag149.py::run_check` 增加分步信号自检。
- **验证门**：`--check` 绿；`score_row` 单测覆盖三组信号。

### WP1 防泄漏地基（D6-①、D6-②）

- 堵 `reference_text` 通道：`rollout.py` 不再把 `ground_truth` 写入 result 的 `reference_text`；确认 `envs/base.py::build_reference_text` 返回空或抽象摘要。
- dataloader 增加 **per-family split 模式**：在每个 subset 族内部分 7:2:1，支持"整 subset 留出"；组合 subset 标记为 `holdout` 永不进训练 split。
- **验证门**：`--check` 显示 per-family split 数；单测断言 optimizer prompt 快照无 `ground_truth` 泄漏。

### WP2 并发提速（D5）

- 移除 prompts 共享树互斥：rollout 改 per-worker 注入 skill 内容（env 或独立目录），E2E 加 skillopt 豁免通道。
- worker 池 + `E2E_QUESTIONS` 小题集并行；训练评分器用分步列。
- **验证门**：两个 worker 并发跑同一批题不冲突；并发 vs 串行结果一致。

### WP3 轨迹归因（D3/D4 的"第二刀"）

- `conversation.json` 扩展：记录每题实际披露的 slices（`disclosed_skill_ids` + reference slug）、模型发的 `skill_request`、停点决策（第几轮、证据覆盖度@停点）、sandbox_error/no_output、mode_debug。
- 产出 L1.5 vs L2（代码 vs 查询）和 L2.5 vs L3（过早停 vs 合成不全）的分离信号。
- **验证门**：轨迹字段齐全；L2.5 停点覆盖度信号单测（对照 golden `source_chunks`）。

### WP4 reflect 接缝可插拔（D7，coding agent 代理）

- `reflect_backend` 配置项（`llm` | `coding_agent`），默认 `llm`。
- 覆盖 `Avrag149Adapter.reflect()`：`llm` 分支走 `run_minibatch_reflect`（现状不变）；`coding_agent` 分支整理轨迹 → 子进程 agent → 解析 RawPatch。
- 专用 `reflect_agent.py`：读轨迹目录 + 当前 skill，产结构化 edits；**agent 工作目录不含 `ground_truth`**（D6-①）。
- **验证门**：`llm` 与 `coding_agent` 两分支对同一批轨迹产出同结构 RawPatch；agent 工作目录无 ground_truth（单测断言）；在"停与答"层开 coding_agent 跑通一轮。

### WP5 三层训练（D2）

- **停与答**（先训）：`prompt_target` = base 事实/不确定节 + 对应 loop nudges；训练集 = PARTIAL/UNGROUNDED/SELECTION_MISS 集中 subset；信号 = 停点覆盖 + cited_gold。
- **检索面**：`prompt_target` = knowledge-base SKILL.md；纯 rag subset 池；信号 = recall + sandbox error。
- **引用面**：`prompt_target` = 引用触发措辞；ipd_table/cross_document 边缘题；信号 = 加载命中。
- 每层独立 config + `include_ids_file` 白名单 + per-family split；组合 subset 恒在 ③ 回归门。
- **验证门**：每层训练后全量 149 不倒退（组合 subset 无 delta 退化）。

### WP6 过拟合检测器（D6-③④⑤）

- 记忆化扫描器挂进 gate：扫描编辑增量 vs golden 全量，命中拒绝。
- train-val gap 日志 + 泛化探针（`new_corpus_factual`/`option_d_*` 强制报告）。
- `analyst_*.md` 反记忆化指令。
- **验证门**：构造"把答案写进 skill"的假编辑，扫描器必拒；泛化探针 subset delta 进训练报告。

### WP7 验收与回填

- 训练产出 `best_skill.md` → 人工审（含扫描器 + 逐字审查）→ 回填产品 prompts（YAML frontmatter + version 递增）→ L1 / 定向 149 回归。
- `graphify update .`（结构性变更硬规则）。
- **验证门**：全量 149 PASS 不低于当前 127/149 且组合 subset 不倒退；回填后产品测试绿。

---

## 4. 验证门汇总

| WP | 门 |
|---|---|
| WP0 | `--check` 绿；score_row 三组信号单测 |
| WP1 | per-family split 数正确；optimizer 快照无 ground_truth 泄漏 |
| WP2 | 双 worker 并发一致；无 prompts 冲突 |
| WP3 | 轨迹字段齐全；停点覆盖单测绿 |
| WP4 | llm/coding_agent 两分支同结构 RawPatch；agent 目录无 ground_truth；停与答层跑通一轮 |
| WP5 | 每层训练后 149 不倒退 |
| WP6 | 记忆化假编辑必拒；探针 delta 进报告 |
| WP7 | 149 PASS ≥ 127 且组合不倒退；回填后产品绿 |

## 5. 验收清单（发起人验收用）

- [ ] 分层评分器落地：`score_row` 按层取信号，三组单测绿
- [ ] `reference_text` 通道已断：optimizer 看不到 golden 答案
- [ ] per-family split 支持整 subset 跨语料留出；组合 subset 恒留 ③
- [ ] 并发 rollout 无 prompts 互斥，双 worker 一致性验证过
- [ ] 轨迹含披露/skill_request/停点/sandbox 归因字段
- [ ] reflect 接缝可插拔：`reflect_backend` 双分支（llm/coding_agent）同结构；agent 分支工作目录无 ground_truth
- [ ] 三层训练（停与答/检索面/引用面）各出 `best_skill.md`，全量 149 不倒退
- [ ] 记忆化扫描器进 gate，假编辑测试必拒
- [ ] 泛化探针（new_corpus_factual/option_d_*）delta 进训练报告
- [ ] 回填纪律执行：人工审 + 逐字审查 + version 递增 + 产品回归绿
- [ ] 终答形态**未**作为训练目标（代码质检台已接管）

## 6. 环境纪律（摘自 AGENTS.md，全文有效）

- prompts-in-md：LLM 可见文案只住 `avrag-rs/prompts/**/*.md`；代码只做加载与占位符替换。
- 第三人称观察式：反馈话术陈述事实，不写命令。
- WSL：`jobs=2`；不并发跑多个全量 cargo test（WP2 并发是 worker 内并行，不是多全量）。
- 不 push、不 PR；本地 trunk 提交。
- golden-set 泄漏红线：任何 skill 文档不得含 149 题题面或参考答案；回填前逐字审查。

## 7. 已知风险与对策

| 风险 | 对策 |
|---|---|
| 分步代理目标被 hack（只优化 recall 崩 faithfulness） | 训练循环内常驻小组合 canary；每层 gate 一起看 |
| 代理信号与真实目标不相关 | 先跑一轮验证"recall 高 ≈ PASS 高"再信任；不相关换代理 |
| JUDGE/INFRA 噪音占评测预算（run3 8/149） | 确认是 judge 故障还是模型输出格式坏（后者是 L3 信号）；已有 skip 逻辑 |
| 分层训练掩盖组合失败 | 组合 subset 从第一天冻结在 ③，每层 gate 都跑 |
| 记忆化骗过随机 split gate | 结构性整语料留出（D6-②）——记忆 train 答案对留出语料无效 |

## 8. 实施偏差记录

- **偏差 ①（WP5 落地时发现）**：per-layer 的 `include_ids_file` 白名单**必须包含
  holdout subset 的题**——holdout 题要进 test 作组合回归门禁，若白名单不含它们，
  `--check` 的"holdout 完整进 test"断言会红（retrieval/reference 初次 --check 即暴露）。
  已修正：`ids_retrieval.json`（134 题）/`ids_reference.json`（59 题）含各自 holdout。
- **偏差 ②（C8 状态）**：C8（验收回填）依赖真实训练产物 `best_skill.md`，而训练
  （真实评测 + LLM/agent 反射）成本高、未在落地期执行。C1–C7 开发全部落地并验证；
  C8 执行 runbook 见 `2026-08-02-skillopt-layered-training-landing.md` §3。

