#!/usr/bin/env python3
"""Export B_frozen metrics for graph81 from an archived full149 run (no re-run)."""
from __future__ import annotations

import argparse
import json
import re
import statistics
from collections import Counter
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
DEFAULT_IDS = ROOT / "avrag-rs/tests/rag_quality/fixtures/graph81_question_ids.json"
DEFAULT_V2 = ROOT / "avrag-rs/crates/app/tests/e2e_output/rag_eval_v2/v2_20260803-090356"
DEFAULT_TRACE = ROOT / "avrag-rs/crates/app/tests/e2e_output/realistic_corpus_full_eval"


def main() -> None:
    ap = argparse.ArgumentParser()
    ap.add_argument("--ids", type=Path, default=DEFAULT_IDS)
    ap.add_argument("--v2", type=Path, default=DEFAULT_V2)
    ap.add_argument("--trace", type=Path, default=DEFAULT_TRACE)
    ap.add_argument(
        "--out",
        type=Path,
        default=ROOT / "docs/engineering/_reports/graph81_b_frozen.tsv",
    )
    args = ap.parse_args()

    meta = json.loads(args.ids.read_text())
    ids = meta["ids"]
    rows = []
    for n in ids:
        art = json.loads((args.v2 / f"q{n:03d}.artifact.json").read_text())
        tr_path = args.trace / f"q{n:03d}.json"
        tools = Counter()
        if tr_path.exists():
            tr = json.loads(tr_path.read_text())
            tools = Counter(t["tool"] for t in tr.get("tool_trace") or [])
        sv = art.get("score_v2") or {}
        ret = sv.get("retrieval") or {}
        rows.append(
            {
                "n": n,
                "subset": art.get("subset"),
                "label": sv.get("label"),
                "correctness": (sv.get("judge") or {})
                .get("answer_correctness", {})
                .get("score"),
                "faithfulness": (sv.get("judge") or {}).get("faithfulness", {}).get("score"),
                "recall": ret.get("recall"),
                "graph_n": tools.get("graph_retrieval", 0),
                "lexical_n": tools.get("lexical_retrieval", 0),
                "dense_n": tools.get("dense_retrieval", 0),
                "question": (art.get("question") or "")[:120],
            }
        )

    args.out.parent.mkdir(parents=True, exist_ok=True)
    cols = [
        "n",
        "subset",
        "label",
        "correctness",
        "faithfulness",
        "recall",
        "graph_n",
        "lexical_n",
        "dense_n",
        "question",
    ]
    with args.out.open("w") as f:
        f.write("\t".join(cols) + "\n")
        for r in rows:
            f.write("\t".join(str(r.get(c, "")) for c in cols) + "\n")

    labels = Counter(r["label"] for r in rows)
    recalls = [r["recall"] for r in rows if r["recall"] is not None]
    print(f"baseline=B_frozen n={len(rows)}")
    print(f"labels={dict(labels)}")
    print(f"PASS_rate={labels.get('PASS', 0) / len(rows):.4f}")
    if recalls:
        print(f"mean_recall={statistics.mean(recalls):.4f}")
    print(f"wrote {args.out}")


if __name__ == "__main__":
    main()
