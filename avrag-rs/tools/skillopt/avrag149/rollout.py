"""avrag149 rollout：一批题目的评测执行 + SkillOpt 轨迹落盘。

SkillOpt 的默认 reflect 会读取 ``<out_dir>/predictions/<id>/conversation.json``，
因此每个 result 都要落一份 system/user/assistant 三件套（assistant 取评测
artifact 的 ``score_v2.model_answer``；取不到则该题不参与反射，只参与评分）。

NOTE（落地期简化）：首版 conversation 只含最小三件套，不含完整检索中间过程；
后续若 reflect 质量不足，可从 artifact 的 mode_debug / retrieval 字段扩展。
"""
from __future__ import annotations

import json
from pathlib import Path

from .runner import (
    SwapPromptFile,
    layer_signals,
    load_artifact_answer,
    parse_report,
    run_eval,
    score_row,
)


def run_batch(
    *,
    items: list[dict],
    skill_content: str,
    out_root: str | Path,
    avrag_rs_root: str | Path,
    prompt_target: str = "system/agent-base.md",
    eval_timeout_secs: int = 3600,
    verbose: bool = True,
) -> list[dict]:
    """跑一个 batch 的题目评测，返回 SkillOpt RolloutResult 列表。

    ``items`` 来自 dataloader（含 1-based ``id``）；``skill_content`` 是当前
    skill 文档，评测期间临时替换 ``prompts/<prompt_target>``。
    """
    out = Path(out_root)
    out.mkdir(parents=True, exist_ok=True)
    # 审计留档：本轮实际注入的 skill 内容
    (out / "skill_used.md").write_text(skill_content, encoding="utf-8")

    ids = sorted(int(it["id"]) for it in items if str(it.get("id", "")).isdigit())
    if not ids:
        raise ValueError("items 里没有可用的 1-based 题号 id")

    prompts_root = Path(avrag_rs_root) / "prompts"
    backup_dir = out / ".prompt_backup"
    with SwapPromptFile(
        prompts_root=prompts_root,
        target_rel=prompt_target,
        skill_content=skill_content,
        backup_dir=backup_dir,
    ):
        v2_dir = run_eval(
            avrag_rs_root,
            questions=ids,
            timeout_secs=eval_timeout_secs,
            verbose=verbose,
        )

    rows, meta = parse_report(v2_dir, ids=ids)
    if verbose and meta:
        counts = meta.get("label_counts") or {}
        print(
            f"  [rollout] {meta.get('total', '?')} 题, labels={counts}, "
            f"judge_ok={meta.get('judge_ok')} error={meta.get('judge_error')}"
        )

    item_by_id = {str(it.get("id")): it for it in items}
    results: list[dict] = []
    pred_dir = out / "predictions"
    pred_dir.mkdir(parents=True, exist_ok=True)

    for n in ids:
        row = rows.get(n, {})
        item = item_by_id.get(str(n), {})
        hard, soft, skipped = score_row(row, no_context=bool(item.get("no_context")))
        answer = load_artifact_answer(v2_dir, n)

        # 落轨迹（SkillOpt reflect 依赖）
        task_dir = pred_dir / str(n)
        task_dir.mkdir(parents=True, exist_ok=True)
        conversation = [
            {"role": "system", "content": skill_content},
            {"role": "user", "content": item.get("query", row.get("query", ""))},
            {"role": "assistant", "content": answer},
        ]
        (task_dir / "conversation.json").write_text(
            json.dumps(conversation, ensure_ascii=False, indent=2), encoding="utf-8",
        )

        if skipped:
            # JUDGE_ERROR（judge API 故障）不是 skill 质量问题:轨迹留档,
            # 但不进聚合/训练,避免把好 skill 按 0 分惩罚(2026-08-01 评分点修正)。
            print(f"  [rollout] {n} JUDGE_ERROR → skip (轨迹已留档)")
            continue

        # 分步代理信号（WP0：按层取信号，黄金集综合 label 不直接当训练梯度）
        signals = layer_signals(row, no_context=bool(item.get("no_context")))
        results.append({
            "id": str(n),
            "hard": hard,
            "soft": soft,
            "predicted_answer": answer,
            "question": item.get("query", ""),
            "subset": row.get("subset") or item.get("subset", ""),
            "task_type": item.get("task_type", ""),
            "label": row.get("label", ""),
            "correctness": row.get("correctness", 0.0),
            "faithfulness": row.get("faithfulness", 0.0),
            "recall": signals["recall"],
            "signals": signals,
            # C2/WP1（D6-①）：gold 答案绝不进 optimizer 视野——reference_text 会作为
            # Hidden Reference 喂给 analyst（gradient/reflect.py:160），空串断泄漏。
            # 评分（hard/soft）在宿主侧已算好，optimizer 不需要 gold 文本。
            "reference_text": "",
            "n_turns": 1,
        })

    (out / "rollouts.json").write_text(
        json.dumps(results, ensure_ascii=False, indent=2), encoding="utf-8",
    )
    return results
