#!/usr/bin/env python3
"""C3/WP2 验证：并发机制（plain assert，.venv/bin/python avrag149/test_wp2.py 运行）。

覆盖 M2 门的机制部分（真实并发评测一致性是成本门控的手动步骤）：
- build_worker_prompt_tree：per-worker 独立 prompt 树 + skill 注入
- run_batches_parallel：每 worker 独立评测输出目录（不竞争"最新目录"检测）
- _merge_worker_predictions：worker 子目录 predictions 合并回统一读取点
"""
from __future__ import annotations

import json
import sys
import tempfile
from pathlib import Path

_TOOLS = Path(__file__).resolve().parent.parent
if str(_TOOLS) not in sys.path:
    sys.path.insert(0, str(_TOOLS))


def test_build_worker_prompt_tree() -> None:
    from avrag149.runner import build_worker_prompt_tree

    src = Path(tempfile.mkdtemp(prefix="prompts_src_"))
    (src / "system").mkdir()
    (src / "system" / "agent-base.md").write_text("ORIGINAL", encoding="utf-8")
    (src / "loop").mkdir()
    (src / "loop" / "x.nudge.md").write_text("nudge", encoding="utf-8")
    tree_a = build_worker_prompt_tree(src, "system/agent-base.md", "SKILL_A")
    tree_b = build_worker_prompt_tree(src, "system/agent-base.md", "SKILL_B")
    try:
        assert tree_a != tree_b, "两个 worker 必须用独立 prompt 树"
        assert (tree_a / "system" / "agent-base.md").read_text() == "SKILL_A"
        assert (tree_b / "system" / "agent-base.md").read_text() == "SKILL_B"
        # 真实 prompts 树未被触碰
        assert (src / "system" / "agent-base.md").read_text() == "ORIGINAL"
        # 其余文件随树拷贝
        assert (tree_a / "loop" / "x.nudge.md").read_text() == "nudge"
    finally:
        import shutil
        shutil.rmtree(tree_a, ignore_errors=True)
        shutil.rmtree(tree_b, ignore_errors=True)
        shutil.rmtree(src, ignore_errors=True)
    print("  [OK] per-worker prompt 树独立 + skill 注入，真实树未触碰")


def test_run_batches_parallel_distinct_eval_dirs() -> None:
    import avrag149.rollout as rollout_mod
    from avrag149.rollout import run_batches_parallel

    seen: list = []
    def fake_run_batch(**kw):
        seen.append((kw["eval_out_dir_override"], kw["items"][0]["id"]))
        return [{"id": kw["items"][0]["id"]}]

    orig = rollout_mod.run_batch
    rollout_mod.run_batch = fake_run_batch
    try:
        with tempfile.TemporaryDirectory() as td:
            o0 = str(Path(td) / "o0")
            o1 = str(Path(td) / "o1")
            results = run_batches_parallel(
                [([{"id": "1"}], "s1", o0), ([{"id": "2"}], "s2", o1)],
                avrag_rs_root=str(Path(td) / "repo"),
                max_workers=2,
            )
    finally:
        rollout_mod.run_batch = orig

    assert [r[0]["id"] for r in results] == ["1", "2"]
    dirs = [str(d) for d, _ in seen]
    assert len(set(dirs)) == 2, f"每 worker 独立评测输出目录，got {dirs}"
    assert dirs[0].endswith("o0/eval_v2") and dirs[1].endswith("o1/eval_v2")
    print("  [OK] 并行分派：每 worker 独立评测输出目录（无共享写竞争）")


def test_merge_worker_predictions() -> None:
    from avrag149.adapter import _merge_worker_predictions

    with tempfile.TemporaryDirectory() as td:
        out = Path(td)
        for w, i in [("worker_0", "1"), ("worker_1", "2")]:
            p = out / w / "predictions" / i
            p.mkdir(parents=True)
            (p / "conversation.json").write_text(
                json.dumps([{"role": "user", "content": f"q{i}"}]), encoding="utf-8",
            )
        _merge_worker_predictions(out)
        assert (out / "predictions" / "1" / "conversation.json").is_file()
        assert (out / "predictions" / "2" / "conversation.json").is_file()
    print("  [OK] worker predictions 合并回统一读取点（reflect 依赖）")


if __name__ == "__main__":
    print("C3/WP2 验证（机制）：")
    test_build_worker_prompt_tree()
    test_run_batches_parallel_distinct_eval_dirs()
    test_merge_worker_predictions()
    print("全部通过。")
