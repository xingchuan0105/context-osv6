# SkillOpt × avrag149 落地验收指示

> 本文件由落地方生成，供**另一个 agent 窗口**独立验收。验收者没有本会话上下文，
> 请按下列步骤逐项执行；每项给出 PASS/FAIL 与证据，最后汇总结论。
>
> **验收边界（用户明确要求）**：**不运行真实评测、不启动训练**（`realistic_corpus_full_eval`
> 与 `train_avrag149.py` 正式训练均属"落地后先不跑"范围）。验收以**静态检查 +
> 代码审查 + 纪律合规**为主。如你认为必须跑真实评测，先征得用户同意。

## 0. 验收对象

- **提交**：`bb147c04` — `tools(skillopt): 落地 SkillOpt × avrag149（149 题黄金集提示词优化集成）`
- **目的**：把 [microsoft/SkillOpt](https://github.com/microsoft/SkillOpt) v0.2.0 接到
  avrag-rs 产品评测：把 `prompts/` 单个 prompt 文件当可训练 skill，用 149 题黄金集
  nightly 评测作评分器，validation-gated 迭代。
- **当前状态声明**（README）：落地完成、**未训练**。
- **相关纪律**：仓库 `AGENTS.md`（prompts-in-md、golden-set 不泄漏、solo 本地 trunk、
  `.env` 复用、graphify update 规则）。

## 1. 提交完整性检查

```bash
cd /home/chuan/context-osv6
git log --oneline -1 bb147c04
git show --name-only --format= bb147c04 | wc -l
git show --name-only --format= bb147c04 | grep -v '^avrag-rs/tools/skillopt/' || echo "无越界文件"
```

**预期**：`git log` 显示 `tools(skillopt): 落地 SkillOpt × avrag149…`；文件数 = **14**；
第三条**无输出**（全部文件在 `avrag-rs/tools/skillopt/` 下，未混入任何产品代码/开发中改动）。

**PASS 条件**：14 个文件，路径全部在 `avrag-rs/tools/skillopt/` 内。

## 2. 产品代码与 prompts 未被触碰

```bash
cd /home/chuan/context-osv6
git diff bb147c04^ bb147c04 --stat -- avrag-rs/prompts/ avrag-rs/crates/ avrag-rs/bins/ avrag-rs/tests/ | wc -l
```

**预期**：`0`（提交不触碰 prompts 与任何产品代码）。
注意：工作区可能还有**既有的未提交开发改动**（`git status` 会显示大量 M 文件）——
那是用户正在进行的开发批次，与本次验收无关；验收者只需确认 **bb147c04 本身**不含它们。

## 3. 静态自检（不触发评测）

```bash
cd /home/chuan/context-osv6/avrag-rs/tools/skillopt
bash scripts/check.sh
```

**预期输出要点**（全部 `[OK]`，退出码 0）：

```
skillopt: 0.2.0
[OK] avrag_rs_root: …/avrag-rs
[OK] prompts 树 / 目标 prompt 文件 / 黄金集 / skill_init / cargo
[..] 实例化 adapter + 加载 splits …
train=104 val=30 test=15   （合计 149）
task_types: 21 个
[OK] 配置解析：env=avrag149 optimizer_backend=qwen_chat …
落地验证通过。训练/评测尚未执行——等开发全部落地后运行：…
```

**PASS 条件**：所有检查 `[OK]`、splits 104/30/15、末尾明确"训练/评测尚未执行"。
若任一 `[FAIL]`：定位原因并报告（不要自行修改）。

## 4. Python 编译与数据加载断言

```bash
cd /home/chuan/context-osv6/avrag-rs/tools/skillopt
.venv/bin/python -m py_compile train_avrag149.py eval_avrag149.py avrag149/*.py && echo "py_compile OK"
.venv/bin/python - <<'EOF'
import sys; sys.path.insert(0, '.')
from avrag149.dataloader import Avrag149DataLoader
path = '../../tests/rag_quality/golden_set_realistic.json'
items = Avrag149DataLoader(data_path=path).load_raw_items(path)
assert len(items) == 149
assert [it['id'] for it in items] == [str(i) for i in range(1, 150)]
import json
flat = [ex for s in json.load(open(path))['subsets'] for ex in s['examples']]
assert items[0]['query'] == flat[0]['query'] and items[-1]['query'] == flat[-1]['query']
print("dataloader OK: 149 题, id=1..149, 展平顺序与 golden 集一致")
EOF
```

**预期**：`py_compile OK` + `dataloader OK: 149 题, id=1..149, 展平顺序与 golden 集一致`。

## 5. seed skill 一致性

```bash
cd /home/chuan/context-osv6/avrag-rs
diff prompts/system/agent-base.md tools/skillopt/avrag149/skills/initial.md && echo "seed 与产品 prompt 一致"
```

**预期**：无差异输出，`seed 与产品 prompt 一致`。
（seed 是训练起点 = 当前 `agent-base.md` 的拷贝；训练产物 `best_skill.md` 回填前需人工审查。）

## 6. 纪律合规——golden-set 不泄漏

```bash
cd /home/chuan/context-osv6/avrag-rs
python3 - <<'EOF'
import json, pathlib, re
gold = json.load(open('tests/rag_quality/golden_set_realistic.json'))
samples = []
for s in gold['subsets']:
    for ex in s['examples'][:2]:          # 每个 subset 取前 2 条做抽样
        samples.append(ex['query'])
        samples.append(ex.get('expected_answer', ''))
hits = []
for f in pathlib.Path('tools/skillopt').rglob('*'):
    if f.is_file() and f.suffix in {'.md', '.py', '.yaml', '.txt'}:
        text = f.read_text(encoding='utf-8', errors='ignore')
        for q in samples:
            if q and len(q) >= 8 and q in text:
                hits.append((str(f), q[:30]))
print('命中:', hits if hits else '无 —— golden 题面/答案未进入 skillopt 目录')
EOF
```

**预期**：`命中: 无`（抽样 42 条 golden 内容未出现在 `tools/skillopt/` 任何 md/py/yaml/txt 文件）。
注意 `ground_truth` 只在 `dataloader.py`/`rollout.py` 中作为**评分字段名**出现，
属于评分链路，不属于 skill 文档内容——这不构成泄漏。

## 7. 纪律合规——密钥不泄漏

```bash
cd /home/chuan/context-osv6
git grep -nE '(API_KEY|SECRET|PASSWORD|TOKEN)[=:][^ "]{8,}' bb147c04 -- 'avrag-rs/tools/skillopt/**' || echo "提交内无明文密钥"
```

**预期**：无输出（`|| echo` 打印"提交内无明文密钥"）。
另抽查：`configs/avrag149/default.yaml` 只出现**环境变量名**（`QWEN_CHAT_*` 等），无真实值；
`runner.py` 的 `load_env_file` 只把 `.env` 值注入子进程环境、不打印。

## 8. 代码审查要点（逐文件过一遍）

| 文件 | 审查要点 |
|---|---|
| `avrag149/dataloader.py` | 展平顺序与评测 runner 的 `E2E_QUESTIONS`（1-based）语义一致；`ground_truth` 仅评分用 |
| `avrag149/runner.py` | `SwapPromptFile` 是否 try/finally 恢复 + 备份（`out_root/.prompt_backup/`）；评测命令与 `docs/engineering/2026-07-30-full149-process-budget-handover.md` 全量/定向命令一致；`.env` 值不打印 |
| `avrag149/rollout.py` | 每题 hard/soft 计算（`hard = label=="PASS"`）；`conversation.json` 落盘路径符合 skillopt 默认 reflect 约定（`out_dir/predictions/<id>/`） |
| `avrag149/adapter.py` | `EnvAdapter` 四抽象方法（`build_train_env`/`build_eval_env`/`rollout`/`get_task_types`）齐全；默认 `avrag_rs_root` 推断 |
| `train_avrag149.py` | `--check` 模式**不触发评测/不调 LLM**（只读 JSON + 路径检查 + cargo 存在性）；`sync_optimizer_env` 复用 `.env` 的 `AGENT_LLM_*` → `QWEN_CHAT_*` |
| `eval_avrag149.py` | 独立评估入口，不依赖训练产物 |
| `configs/avrag149/default.yaml` | 无密钥；`split_ratio 7:2:1`；`skill_init` 指向 seed；`prompt_target: system/agent-base.md` |
| `README.md` | 回填流程（best_skill.md → 人工审查 → 写回 prompts + version 递增）；golden-set 泄漏红线；崩溃恢复（`git checkout -- prompts/`）；状态声明"落地完成未训练" |
| `avrag149/skills/initial.md` | 与 `prompts/system/agent-base.md` 逐字一致（见 §5），不含任何黄金集内容 |

## 9. 已知限制（验收时对照 README，不视为缺陷）

- 首版 reflect 轨迹为最小三件套（system/user/assistant），不含检索中间过程。
- `graphify update` 因全仓库索引超时未完成（本次为新增独立 Python 目录，未改既有
  Rust/TS 结构；`graphify-out/` 无脏改动）——待开发批次收尾一并跑。
- 评测前置条件（Milvus/PG/Redis 运行、语料已灌库）只在真正训练/评估时需要，静态验收不涉及。

## 10. 验收结论模板

```markdown
## 验收结论

- §1 提交完整性：PASS/FAIL（证据：…）
- §2 产品代码未触碰：PASS/FAIL（证据：…）
- §3 静态自检：PASS/FAIL（证据：…）
- §4 编译与数据断言：PASS/FAIL（证据：…）
- §5 seed 一致性：PASS/FAIL（证据：…）
- §6 golden-set 不泄漏：PASS/FAIL（证据：…）
- §7 密钥不泄漏：PASS/FAIL（证据：…）
- §8 代码审查：PASS/FAIL（逐文件发现：…）
- §9 已知限制：已对照 / 有新增

总评：通过 / 有条件通过（列出条件） / 不通过（列出阻断项）
```

验收者只报告，不修改代码；发现问题列出文件:行号与原因即可。
