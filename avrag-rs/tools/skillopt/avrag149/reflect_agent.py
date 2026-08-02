#!/usr/bin/env python3
"""Coding-agent reflect：从轨迹工作区产出结构化 skill 编辑（RawPatch）。

WP4/D7：评估/改写用 coding agent 代理。adapter 构造**无 ground_truth** 的工作区
（D6-① 红线），本脚本读工作区 → 分析失败归因 → 产编辑建议。

调用链：
    Avrag149Adapter._reflect_coding_agent
        → 本脚本 --workspace <dir> --skill <file> --trajectories <dir> --out <json>
        → 输出 {"patches": [{"patch": {"reasoning","edits"}, "source_type"}]}

后端选择：
- 设 ``REFLECT_AGENT_CMD``（如 ``claude -p``）→ 把任务 prompt 作为 stdin/stdin 参数
  交给 coding agent CLI，让它读工作区文件后返回编辑 JSON（真正"代理"形态）；
- 否则回退 ``chat_optimizer``（skillopt optimizer LLM，与 llm 分支同源）。

编辑 schema（与 analyst_error.md 一致）：
    {"op": "append"|"insert_after"|"replace", "target": "...", "content": "..."}
"""
from __future__ import annotations

import argparse
import json
import os
import subprocess
import sys
from pathlib import Path

_SYSTEM = """You are a senior coding agent optimizing an agent skill document.

You are given:
- The current skill document (the prompt file being trained).
- Per-question failure trajectories (conversation + layer attribution:
  retrieval/selection/stop/grounding signals).

Your job: identify the most common, systematic failure patterns and propose a
concise set of skill edits that fix them.

HARD RULES:
- Edits must be generalizable. NEVER write any concrete question, entity name,
  number, reference answer, or citation fragment into the skill (anti-memorization).
- If an edit depends on a specific task's fact, rewrite it as an abstract rule
  or drop it.
- Only patch gaps in the skill; do not duplicate existing content.

Respond ONLY with a valid JSON object (no markdown fences, no extra text):
{
  "reasoning": "<why these edits fix the batch's common failures>",
  "edits": [
    {"op": "append",       "content": "<markdown to add at end of skill>"},
    {"op": "insert_after", "target": "<exact heading/text>", "content": "<markdown>"},
    {"op": "replace",      "target": "<exact text>", "content": "<replacement>"}
  ]
}"""


def _fmt_trajectories(traj_dir: Path) -> str:
    parts: list[str] = []
    for tid in sorted(traj_dir.iterdir()):
        if not tid.is_dir():
            continue
        conv = tid / "conversation.json"
        attr = tid / "trajectory_attribution.json"
        if not conv.is_file():
            continue
        try:
            conversation = json.loads(conv.read_text(encoding="utf-8"))
        except json.JSONDecodeError:
            continue
        lines = [f"### Trajectory {tid.name}"]
        for m in conversation:
            role = m.get("role", "?")
            content = str(m.get("content", ""))[:2000]
            lines.append(f"[{role}] {content}")
        if attr.is_file():
            try:
                a = json.loads(attr.read_text(encoding="utf-8"))
                lines.append(f"[attribution] {json.dumps(a, ensure_ascii=False)}")
            except json.JSONDecodeError:
                pass
        parts.append("\n".join(lines))
    return "\n\n---\n\n".join(parts)


def _call_agent(user_prompt: str) -> str:
    """调 coding agent CLI（REFLECT_AGENT_CMD）或回退 chat_optimizer。"""
    cmd = os.environ.get("REFLECT_AGENT_CMD", "").strip()
    if cmd:
        r = subprocess.run(
            [*cmd.split(), user_prompt], capture_output=True, text=True, timeout=900,
        )
        if r.returncode != 0:
            raise RuntimeError(f"REFLECT_AGENT_CMD 失败 rc={r.returncode}: {r.stderr[-2000:]}")
        return r.stdout.strip()
    # 回退：skillopt optimizer LLM（与 llm 分支同源）
    from skillopt.model import chat_optimizer
    text, _meta = chat_optimizer(system=_SYSTEM, user=user_prompt, stage="reflect_agent")
    return text


def _parse_response(text: str) -> dict:
    """容错解析 LLM/agent 返回的 JSON（去 markdown fences）。"""
    t = text.strip()
    if t.startswith("```"):
        t = t.strip("`")
        t = t[t.find("{"):]
    obj = json.loads(t)
    if "patch" in obj:  # 有的 agent 直接返回完整 RawPatch
        return obj
    return {"reasoning": obj.get("reasoning", ""), "edits": obj.get("edits", [])}


def main() -> None:
    p = argparse.ArgumentParser(description="coding-agent reflect")
    p.add_argument("--workspace", required=True)
    p.add_argument("--skill", required=True)
    p.add_argument("--trajectories", required=True)
    p.add_argument("--out", required=True)
    p.add_argument("--edit-budget", type=int, default=4)
    args = p.parse_args()

    skill = Path(args.skill).read_text(encoding="utf-8")
    traj_text = _fmt_trajectories(Path(args.trajectories))
    user_prompt = (
        f"Current skill document:\n{skill}\n\n"
        f"Trajectories (with layer attribution):\n{traj_text}\n\n"
        f"Produce at most {args.edit_budget} edits."
    )
    text = _call_agent(user_prompt)
    parsed = _parse_response(text)
    patches = [{
        "patch": {"reasoning": parsed.get("reasoning", ""),
                  "edits": parsed.get("edits", [])[: args.edit_budget]},
        "source_type": "failure",
    }]
    Path(args.out).write_text(
        json.dumps({"patches": patches}, ensure_ascii=False, indent=2), encoding="utf-8",
    )
    print(f"[reflect_agent] wrote {len(patches)} patch(es) to {args.out}")


if __name__ == "__main__":
    main()
