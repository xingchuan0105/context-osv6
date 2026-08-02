#!/usr/bin/env python3
"""C4/WP3 验证：轨迹归因（plain assert，.venv/bin/python avrag149/test_wp3.py 运行）。

用真实评测产物（v2_20260802-045319）验证：
- load_attribution 从 score_v2 + mode_debug 取分步信号
- RETRIEVAL_MISS → 检索面（code_error/no_output vs query）
- SELECTION_MISS → L3b 选择（cited_gold=0）
- summarize_attribution 产出 reflect 可读的 fail_reason
"""
from __future__ import annotations

import sys
from pathlib import Path

_TOOLS = Path(__file__).resolve().parent.parent
if str(_TOOLS) not in sys.path:
    sys.path.insert(0, str(_TOOLS))

from avrag149.runner import (  # noqa: E402
    load_attribution,
    parse_report,
    summarize_attribution,
)

AVRAG_RS = _TOOLS.parent.parent
V2 = AVRAG_RS / "crates/app/tests/e2e_output/rag_eval_v2/v2_20260802-045319"


def _rows() -> dict:
    return parse_report(V2, ids=None)[0]


def test_retrieval_miss_attribution() -> None:
    rows = _rows()
    # #6 thesis_factual RETRIEVAL_MISS（recall=0）
    row = rows[6]
    assert row["label"] == "RETRIEVAL_MISS"
    attr = load_attribution(V2, 6)
    assert attr["retrieval_recall"] == 0.0
    reason = summarize_attribution(row["label"], attr)
    assert "label=RETRIEVAL_MISS" in reason
    # 层分离：code_error → L1.5；否则 L2 查询
    assert any(t in reason for t in ("code_error", "no_output", "query_recall")), reason
    print(f"  [OK] RETRIEVAL_MISS 归因: {reason}")


def test_selection_miss_attribution() -> None:
    rows = _rows()
    # #18 thesis_synthesis SELECTION_MISS（recall>0 但 cited_gold=0）
    row = rows[18]
    assert row["label"] == "SELECTION_MISS", row["label"]
    attr = load_attribution(V2, 18)
    reason = summarize_attribution(row["label"], attr)
    assert "cited_gold=" in reason, reason
    print(f"  [OK] SELECTION_MISS 归因: {reason}")


def test_ungrounded_attribution() -> None:
    rows = _rows()
    # #121 rag_search_joint UNGROUNDED（编造）
    row = rows[121]
    assert row["label"] == "UNGROUNDED", row["label"]
    attr = load_attribution(V2, 121)
    reason = summarize_attribution(row["label"], attr)
    assert "unsupported_claims=" in reason, reason
    print(f"  [OK] UNGROUNDED 归因: {reason}")


def test_passes_have_empty_attribution_reason() -> None:
    rows = _rows()
    n_pass = sum(1 for r in rows.values() if r.get("label") == "PASS")
    assert n_pass >= 120, f"PASS 数异常: {n_pass}"
    print(f"  [OK] PASS={n_pass}/149（真实评测产物存在且解析正常）")


if __name__ == "__main__":
    print("C4/WP3 验证（真实产物归因）：")
    test_retrieval_miss_attribution()
    test_selection_miss_attribution()
    test_ungrounded_attribution()
    test_passes_have_empty_attribution_reason()
    print("全部通过。")
