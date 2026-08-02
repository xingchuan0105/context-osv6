# SkillOpt 分层训练改造 · 全量落地编排（2026-08-02）

> 配套文档：`docs/plans/2026-08-02-skillopt-layered-training-impl.md`（设计契约 D1–D7 + WP0–WP7）。
> 本文档是**执行编排**：任务级拆解、依赖 DAG、并行轨道、提交序列、检查点。新窗口按此顺序落地。
> 纪律：solo trunk 本地提交、不 push、WSL `jobs=2`、不并发跑多个全量 cargo test。所有锚点已核实到文件:行号。

---

## 1. 依赖 DAG 与执行轨道

```
                        ┌─────────────────────────────────────┐
   WP0 分步评分器 ◄──────┤  WP0/WP1 无互依，可并行            │
   WP1 防泄漏地基  ◄─────┘   （都 touch runner.py，注意合并）  │
        │                        │                            │
        ▼                        ▼                            │
   WP3 轨迹归因            WP2 并发提速（Rust config.rs）      │
        │                        │                            │
        └────────┬───────────────┘                            │
                 ▼                                            │
   WP4 reflect 接缝 ◄─────── WP6 记忆化扫描器（依赖 WP1 留出） │
                 │                 │                          │
                 └────────┬────────┘                          │
                          ▼                                   │
                   WP5 三层训练 ◄────── 消费 WP0/1/2/3/4/6     │
                          ▼                                   │
                   WP7 验收回填                               │
                        └─────────────────────────────────────┘
```

**并行轨道（WSL 允许，jobs=2）**：
- **轨道 A（信号与数据地基）**：WP0 → WP1 → WP3（同域 Python，串行稳）
- **轨道 B（并发基建）**：WP2（Rust harness，与 A 无文件交集，可并行）
- **轨道 C（反射与检测）**：WP4（依赖 WP3）、WP6 扫描器（依赖 WP1，可与 WP3 并行）
- **轨道 D（训练与验收）**：WP5 → WP7（消费 A+B+C）

## 2. 执行阶段与检查点

### Phase 0 · 地基（M1：可归因的干净信号）

| WP | 任务 | 锚点 |
|---|---|---|
| **WP0** 分步评分器拆层 | `score_row` 增加按层信号：检索面用 recall、合成面用 correctness/faithfulness、代码面用 sandbox_error/no_output；返回结构扩展 | `tools/skillopt/avrag149/runner.py:224` |
| | `run_check` 增加分步信号自检 | `train_avrag149.py::run_check` |
| **WP1** 断泄漏 | rollout `reference_text` 不再填 `ground_truth`；覆盖 `build_reference_text` 返回抽象/空 | `rollout.py:111`、adapter 覆盖 `envs/base.py:62` |
| | dataloader 加 **per-family split**：subset 族内 7:2:1 + 整 subset 留出（组合 subset 标记 holdout） | `avrag149/dataloader.py` |
| | `--check` 断言 optimizer 快照无 ground_truth | `train_avrag149.py` |

**M1 门**：`--check` 绿；per-family split 计数正确；构造"泄漏"用例断言 optimizer 输入无 gold 文本。

### Phase 1 · 并发与归因（M2/M3：快 + 可归因）

| WP | 任务 | 锚点 |
|---|---|---|
| **WP2** 去互斥 | E2E 加 skillopt 豁免通道：`PROMPT_DIR` 允许 skillopt 注入（仅测试时生效） | `crates/app/tests/product_e2e/test_context/config.rs:62,215` |
| | rollout 改 per-worker 注入：env/独立目录传 skill，`SwapPromptFile` 降级为兜底 | `runner.py:54` |
| | worker 池 + `E2E_QUESTIONS` 小题集并行 | `train_avrag149.py` / adapter |
| **WP3** 轨迹归因 | conversation 扩展：披露 slices、skill_request、停点决策、sandbox_error/no_output、mode_debug | `rollout.py:85` `run_batch` |
| | 产出 L1.5 vs L2、L2.5 vs L3 分离信号 | adapter / 新解析模块 |

**M2/M3 门**：双 worker 并发结果与串行一致；轨迹字段齐全；停点覆盖度单测绿（对照 golden `source_chunks`）。

### Phase 2 · 反射与检测（M4：可训练）

| WP | 任务 | 锚点 |
|---|---|---|
| **WP4** reflect 接缝 | `reflect_backend = llm \| coding_agent` 配置项 | `default.yaml`、`train_avrag149.py` |
| | 覆盖 `Avrag149Adapter.reflect()`：llm 分支现状；coding_agent 分支整理轨迹 → 子进程 agent → RawPatch | `adapter.py`（覆盖 `envs/base.py:234`） |
| | 专用 `reflect_agent.py`：读轨迹 + 当前 skill，产结构化 edits，**工作目录无 ground_truth** | 新建 `avrag149/reflect_agent.py` |
| **WP6** 记忆化扫描器 | 扫描 gate：编辑增量 vs golden 全量（子串 + n-gram/embedding），命中拒绝进拒绝缓存 | 新建 `avrag149/memorization_scanner.py` + gate hook |
| | train-val gap 日志 + 泛化探针（`new_corpus_factual`/`option_d_*`）强制报告 | `train_avrag149.py` |
| | `analyst_*.md` 反记忆化指令 | `avrag149/prompts/*.md` |

**M4 门**：llm/coding_agent 双分支同结构 RawPatch；agent 目录无 gold（单测断言）；假编辑必拒。

### Phase 3 · 三层训练（M5：不倒退的训练）

| WP | 任务 | 锚点 |
|---|---|---|
| **WP5** 停与答（先训） | config：`prompt_target`=base 事实/不确定节 + 对应 loop nudges；信号=停点覆盖 + cited_gold | `configs/avrag149/stop-answer.yaml` |
| | 检索面：`prompt_target`=knowledge-base SKILL.md；纯 rag subset；信号=recall+sandbox | `configs/avrag149/retrieval.yaml` |
| | 引用面：引用触发措辞；ipd_table/cross_document 边缘题；信号=加载命中 | `configs/avrag149/reference.yaml` |
| | 组合 subset 恒在回归门；每层 gate 跑全量 149 | `train_avrag149.py` / adapter |

**M5 门**：每层训练后全量 149 ≥ 127/149 且组合 subset 无 delta 退化。

### Phase 4 · 验收回填（M6：出厂）

| WP | 任务 | 锚点 |
|---|---|---|
| **WP7** 回填 | best_skill.md → 扫描器 + 人工逐字审查 → 回填产品 prompts（frontmatter + version 递增）→ L1/定向 149 回归 | `prompts/system/agent-base.md` 等 |
| | `graphify update .`（结构性变更硬规则） | — |

**M6 门**：产品测试绿；回填 commit 独立。

## 3. 提交序列（solo trunk，本地）

```
C1  WP0  feat(skillopt): score_row 分步信号拆层          → M1 门 ✅ 7585af8a
C2  WP1  feat(skillopt): 断 reference_text 泄漏 + per-family split → M1 门 ✅ c59de31f
C3  WP2  feat(skillopt): per-worker prompt 注入 + worker 池  → M2 门 ✅ 02b947a5
C4  WP3  feat(skillopt): 轨迹披露/停点归因               → M3 门 ✅ 6010ed1a
C5  WP6  feat(skillopt): 记忆化扫描器 + gap 监控         → M4 门 ✅ 6ae50513
C6  WP4  feat(skillopt): reflect 接缝可插拔 + coding agent → M4 门 ✅ deceea49
C7  WP5  feat(skillopt): 三层训练 configs（停与答/检索/引用） → M5 门 ✅ b687df3b
C8  WP7  best_skill 回填 + 产品回归                     → M6 门 ⏳ 待真实训练产物
```

每 C 独立过对应验证门；C1–C7 开发已全部落地（各 WP 测试 + `--check`/`--signals`/`--scan`/`--gap` 全绿）。
**C8 是唯一依赖真实训练产物的门**：训练很贵（真实评测 + LLM/agent 反射），须先跑一轮再回填。

### C8 回填执行 runbook（best_skill.md 产出后）

1. **训"停与答"一轮**（失败覆盖 86%，优先）：
   ```bash
   cd avrag-rs/tools/skillopt
   .venv/bin/python train_avrag149.py --config configs/avrag149/stop-answer.yaml
   ```
   - 训练前自动对 seed skill 跑 val baseline；产物 `outputs/skillopt_avrag149_*/best_skill.md`。
   - 开并行/切 coding agent 反射按需：`--cfg-options env.eval_workers=2 env.reflect_backend=coding_agent`。
2. **扫描器 + 人工逐字审查**（红线，D6-③ + README 纪律）：
   ```bash
   .venv/bin/python train_avrag149.py --scan outputs/<run>/best_skill.md
   ```
   命中 → 拒绝回填；未命中 → 人工逐字核对是否含 149 题题面/参考答案。
3. **全量 149 回归**（M5 门：≥127/149 且组合 subset 不倒退）：
   ```bash
   .venv/bin/python train_avrag149.py --signals v2_<new_run>
   ```
   对比组合 subset（cross_document/ipd_table/rag_search_joint/cross_adr）delta。
4. **回填产品 prompts**：best_skill.md → `prompts/system/agent-base.md`
   （保持 YAML frontmatter + `version` 递增）→ `L1 / 定向 149` 产品测试回归。
5. **`graphify update .`**（结构性变更硬规则）+ 独立 commit。

## 4. 关键决策（执行时再确认）

1. **WP2 豁免通道的安全性**：PROMPT_DIR 豁免只对 skillopt 环境变量生效（如 `E2E_SKILLOPT_INJECT`），生产/其他测试路径不受影响——这是产品评测链路守卫，改动要过现有 product-e2e 测试。
2. **WP4 coding agent 选型**：`claude -p` CLI 是最快路径；若需可控输出 schema 用专用 `reflect_agent.py`（推荐，可挂反记忆化 + 可测试）。
3. **WP1 的 `build_reference_text`**：优先在 adapter 覆盖返回空/抽象，不改 site-packages（版本可控纪律）。
4. **WP5 训练成本**：三层各自独立训练循环；先跑"停与答"一轮看信号质量，再决定是否开检索面/引用面。

## 5. 风险与回退

| 风险 | 回退 |
|---|---|
| WP2 豁免通道破坏现有 E2E 守卫 | 环境变量开关默认关；回归 product-e2e 既有测试 |
| coding agent 成本失控 | 只开"停与答"层；简单层保持 llm |
| 三层训练后组合 subset 退化 | 组合 subset 恒在回归门，退化即停；回退到 C7 前 |
| 记忆化扫描误伤正常泛化编辑 | 扫描器只拒"编辑增量命中 golden 字面/高相似"；阈值可调；误伤进拒绝缓存可人工放行 |
| 轨迹归因字段膨胀 token | 只落最小归因集（披露 ids + 停点轮次 + sandbox flag），不落全量中间过程 |
