#!/usr/bin/env python3
"""C6/WP4 验证：reflect 接缝可插拔（plain assert，.venv/bin/python avrag149/test_wp4.py 运行）。

覆盖 M4 门：
- backend=llm → 走 skillopt 默认 run_minibatch_reflect（dispatch 正确）
- backend=coding_agent → 走 reflect_agent.py；工作区（agent 输入）不含 ground_truth
- coding_agent 输出 RawPatch 结构正确（{"patch": {reasoning, edits}, source_type}）
"""
from __future__ import annotations

import json
import os
import sys
import tempfile
from pathlib import Path

_TOOLS = Path(__file__).resolve().parent.parent
if str(_TOOLS) not in sys.path:
    sys.path.insert(0, str(_TOOLS))

from avrag149.adapter import Avrag149Adapter  # noqa: E402


def test_llm_branch_dispatches_to_default() -> None:
    import skillopt.gradient.reflect as reflect_mod

    sentinel = [{"patch": {"reasoning": "x", "edits": []}, "source_type": "failure"}]
    orig = reflect_mod.run_minibatch_reflect
    reflect_mod.run_minibatch_reflect = lambda **kw: sentinel
    try:
        adapter = Avrag149Adapter(avrag_rs_root=str(_TOOLS.parent.parent),
                                  reflect_backend="llm")
        out = adapter.reflect([], "# skill", str(Path(tempfile.mkdtemp())))
    finally:
        reflect_mod.run_minibatch_reflect = orig
    assert out is sentinel, "llm 分支必须走 run_minibatch_reflect"
    print("  [OK] backend=llm → run_minibatch_reflect（默认路径）")


def test_coding_agent_branch_no_gold_and_rawpatch() -> None:
    with tempfile.TemporaryDirectory() as td:
        td = Path(td)
        # 1. 假 predictions（含一个轨迹，无 gold）
        out_dir = td / "out"
        pred = out_dir / "predictions" / "1"
        pred.mkdir(parents=True)
        (pred / "conversation.json").write_text(
            json.dumps([{"role": "user", "content": "某公司哪年建厂？"}]),
            encoding="utf-8",
        )
        (pred / "trajectory_attribution.json").write_text(
            json.dumps({"retrieval_recall": 0.0, "stop_class": "premature"}),
            encoding="utf-8",
        )

        # 2. stub coding agent：把收到的 prompt 落盘 + 返回 fixture RawPatch
        gold = "2019年于大连市投资建厂"  # 模拟 gold（不应出现在 agent 输入）
        (out_dir / "gold_probe.txt").write_text(gold, encoding="utf-8")
        prompt_capture = td / "agent_prompt.txt"
        stub = td / "stub_agent.py"
        stub.write_text(
            "import json, sys\n"
            f"open({str(prompt_capture)!r}, 'w').write(sys.argv[-1])\n"
            "print(json.dumps({'reasoning': 'r', 'edits': ["
            "{'op': 'append', 'content': '## 检索策略\\n- 先 catalog 再收窄'}]}))\n",
            encoding="utf-8",
        )
        os.environ["REFLECT_AGENT_CMD"] = f"{sys.executable} {stub}"

        # 3. coding_agent 分支
        adapter = Avrag149Adapter(avrag_rs_root=str(_TOOLS.parent.parent),
                                  reflect_backend="coding_agent")
        patches = adapter.reflect([], "# skill", str(out_dir))
        # 4. 断言：工作区（agent 输入）无 ground_truth（D6-①）
        prompt = prompt_capture.read_text(encoding="utf-8")
        assert gold not in prompt, f"agent 输入泄漏 gold: {gold!r}"

    assert isinstance(patches, list) and patches, "必须产出 RawPatch 列表"
    p = patches[0]
    assert set(p) == {"patch", "source_type"}, p
    assert p["source_type"] == "failure"
    assert isinstance(p["patch"].get("edits"), list)
    assert p["patch"]["edits"][0]["op"] == "append"
    print("  [OK] coding_agent 分支：RawPatch 结构正确 + 工作区无 gold 泄漏")


if __name__ == "__main__":
    print("C6/WP4 验证（reflect 接缝）：")
    test_llm_branch_dispatches_to_default()
    test_coding_agent_branch_no_gold_and_rawpatch()
    print("全部通过。")
