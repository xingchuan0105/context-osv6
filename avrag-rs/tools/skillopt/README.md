# SkillOpt × avrag149 —— 用产品 149 题黄金集训练 agent 提示词

把 [microsoft/SkillOpt](https://github.com/microsoft/SkillOpt)（v0.2.0）接到 avrag-rs
的产品评测上：把 `prompts/` 下的单个 prompt 文件当作可训练的 skill 文档，用
**真实 nightly 评测（149 题黄金集）** 作为评分器，validation-gated 地迭代它。

> **当前状态：落地完成，未训练。** 静态自检（`--check`）已验证；训练/评测
> **等开发全部落地后**再执行（用户指定）。

## 原理

```
train_avrag149.py
  └─ ReflACTTrainer（skillopt 引擎）
       ├─ rollout   → 把 skill 文档临时写入 prompts/<prompt_target>，
       │              跑 `E2E_MODE=nightly … realistic_corpus_full_eval`
       │              解析 v2 报告（per_query.tsv）→ 每题 hard/soft
       ├─ reflect   → optimizer LLM（DeepSeek，复用 .env AGENT_LLM_*）
       │              读 rollout 轨迹，生成 add/delete/replace 编辑
       ├─ update    → 应用到 skill 文档
       └─ gate      → 候选 skill 严格提升 held-out(val) 分数才接受
                      产物：outputs/<run>/best_skill.md
```

- **优化单元**：`prompts/system/agent-base.md`（config `env.prompt_target` 可切换任意 prompts 文件）。
- **评分口径**：`hard = 1 iff label == "PASS"`（与 nightly PASS 计数一致）；
  `soft = mean(correctness, faithfulness)`。
- **数据集**：`avrag-rs/tests/rag_quality/golden_set_realistic.json`（149 题），
  按 subsets 展平顺序编号 1..149（与 `E2E_QUESTIONS` 索引一致），
  `split_ratio 7:2:1` 确定性划分 train/val/test。

## 安装（已在本机完成，可重跑）

```bash
bash tools/skillopt/scripts/setup.sh        # venv + pip install skillopt==0.2.0
```

## 使用

```bash
cd avrag-rs/tools/skillopt

# ① 静态自检（落地验证，不触发评测 / 不调 LLM）
.venv/bin/python train_avrag149.py --config configs/avrag149/default.yaml --check

# ② 正式训练（等开发全部落地后执行；跑真实评测 + LLM 反射，很贵）
.venv/bin/python train_avrag149.py --config configs/avrag149/default.yaml \
    --cfg-options train.num_epochs=2 train.batch_size=8

# ③ 评估任意 skill 文档（训练外独立验证）
.venv/bin/python eval_avrag149.py --skill outputs/skillopt_avrag149_xxx/best_skill.md
```

评测前置条件（与产品 nightly 相同）：Milvus/PG/Redis 已运行、语料已灌库
（`realistic_corpus_full_eval` 复用 workspace，不灌库）。参
`avrag-rs/docs/engineering/2026-07-30-full149-process-budget-handover.md`。

## 回填流程（训练产物 → 产品 prompts）

1. 训练产出 `best_skill.md` 后，**先人工审查**（见下方纪律），再决定是否采纳。
2. 采纳时：把内容写回 `prompts/system/agent-base.md`（保持 YAML frontmatter
   与 `version` 递增），提交并跑产品测试回归（L1 / 定向 149）。
3. 未采纳时：产物留在 `outputs/`，prompts 不动。

## 纪律（非协商）

- **golden-set 泄漏红线**（AGENTS.md）：`golden_set_realistic.json` 只用于
  评分（`ground_truth` 只进 rollout 的 `reference_text`，供 reflect 打分参考）。
  **任何 skill 文档（含 `best_skill.md`）不得包含 149 题题面或参考答案**；
  回填 prompts/ 前逐字审查。`avrag149/skills/initial.md` 是产品 prompt 的拷贝，
  其中不含任何黄金集内容。
- **不改产品代码**：本目录是独立 Python 工具；rollout 通过「临时交换
  `prompts/<prompt_target>` → 评测 → 恢复」注入（E2E 强制 `PROMPT_DIR` 指向
  真实 prompts 树，外部覆盖无效）。交换有 try/finally + 备份（`out_root/.prompt_backup/`）
  保护；若评测被强杀导致 prompts 文件残留，用
  `git -C avrag-rs checkout -- prompts/` 恢复。
- **`.env` 复用**：optimizer 凭据自动从 `avrag-rs/.env` 的 `AGENT_LLM_*`
  （DeepSeek）回填为 `QWEN_CHAT_*`；密钥不打印、不落盘。
- **训练期间勿并行跑其他 149 评测**（单 worker 环境，prompts 文件互斥）。

## 已知限制（落地期）

- 首版 conversation 轨迹为最小三件套（system/user/assistant，assistant 取
  artifact 的 `score_v2.model_answer`），不含检索中间过程；若 reflect 编辑
  质量不足，可从 artifact 的 retrieval/mode_debug 扩展。
- `eval_avrag149.py` 全量模式依赖 dataloader 的 split 划分（按 n 重排），
  子集模式直接按题号跑。
- skillopt 0.2.0（PyPI）无 `openai_compatible` backend（GitHub main 才有）；
  故 optimizer 用 `qwen_chat` 对接 DeepSeek 的 OpenAI 兼容端点。
