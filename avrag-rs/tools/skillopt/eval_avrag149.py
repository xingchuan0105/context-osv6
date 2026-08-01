#!/usr/bin/env python3
"""avrag149 SkillOpt 评估入口：用指定 skill 文档跑产品评测并打印得分。

落地期不执行；等开发全部落地后，用于验证 best_skill.md / 任意候选 skill：

    .venv/bin/python eval_avrag149.py --skill outputs/xxx/best_skill.md
    .venv/bin/python eval_avrag149.py --skill avrag149/skills/initial.md --questions 1,2,3
"""
from __future__ import annotations

import argparse
import os
import sys
from pathlib import Path

_TOOLS_ROOT = Path(__file__).resolve().parent
if str(_TOOLS_ROOT) not in sys.path:
    sys.path.insert(0, str(_TOOLS_ROOT))

from avrag149.adapter import Avrag149Adapter  # noqa: E402
from avrag149.runner import parse_report, run_eval, score_row  # noqa: E402


def main() -> None:
    p = argparse.ArgumentParser(description="avrag149 skill 评估")
    p.add_argument("--skill", required=True, help="skill 文档（Markdown）路径")
    p.add_argument("--questions", default="", help="逗号分隔的 1-based 题号；空 = 全量 149")
    p.add_argument("--avrag-rs-root", default="", help="avrag-rs 根目录（默认自动推断）")
    p.add_argument("--prompt-target", default="system/agent-base.md", help="prompts 内被替换的文件")
    p.add_argument("--out-root", default="", help="产物目录（默认 outputs/eval_<ts>）")
    args = p.parse_args()

    skill_path = Path(args.skill)
    if not skill_path.is_file():
        print(f"skill 文件不存在: {skill_path}", file=sys.stderr)
        sys.exit(1)
    skill_content = skill_path.read_text(encoding="utf-8")

    adapter = Avrag149Adapter(
        avrag_rs_root=args.avrag_rs_root,
        prompt_target=args.prompt_target,
    )
    out_root = args.out_root or str(_TOOLS_ROOT / "outputs" / f"eval_{Path(args.skill).stem}")
    os.makedirs(out_root, exist_ok=True)

    questions: list[int] | None = None
    if args.questions.strip():
        questions = [int(q) for q in args.questions.split(",") if q.strip()]

    # 复用 rollout 的注入逻辑（临时交换 prompts 目标文件 + 评测 + 恢复）
    from avrag149.rollout import run_batch
    items = [{"id": str(q), "query": "", "subset": "", "task_type": ""} for q in questions] if questions else None
    if items is None:
        # 全量：直接从 dataloader 拿 149 个 item（不 shuffle，保持 1..149）
        adapter.setup({"out_root": out_root, "split_mode": "ratio", "data_path": ""})
        dl = adapter.get_dataloader()
        items = [dict(it) for it in dl.train_items + dl.val_items + dl.test_items]
        # ratio 划分已 shuffle，重排回 n 序
        items.sort(key=lambda it: int(it["id"]))

    results = run_batch(
        items=items,
        skill_content=skill_content,
        out_root=out_root,
        avrag_rs_root=adapter.avrag_rs_root,
        prompt_target=args.prompt_target,
    )

    total = len(results)
    hard = sum(r["hard"] for r in results)
    mean_c = sum(r["correctness"] for r in results) / total if total else 0.0
    mean_f = sum(r["faithfulness"] for r in results) / total if total else 0.0
    from collections import Counter
    labels = Counter(r["label"] for r in results)
    print(f"\n  PASS {hard}/{total} ({hard / total:.1%})")
    print(f"  mean correctness={mean_c:.3f} faithfulness={mean_f:.3f}")
    print(f"  labels={dict(labels)}")
    print(f"  产物: {out_root}")


if __name__ == "__main__":
    main()
