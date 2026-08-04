#!/usr/bin/env bash
# G3: graph81 quality sample on RETRIEVAL_BACKEND=pgvector (vs archived Milvus D1).
#
# Full graph81 needs real LLM + corpus (nightly). Defaults to a **slice** for
# cost control; override with G3_SLICE=full for all 81 ids.
#
# Usage:
#   bash scripts/run-graph81-pgvector-g3.sh          # slice (default 12 ids)
#   G3_SLICE=full bash scripts/run-graph81-pgvector-g3.sh
#   G3_DRY=1 bash scripts/run-graph81-pgvector-g3.sh  # print env + ids only
#
# Baseline arm: product D1 (DENSE_BACKEND=vgrag, no lexical side-car).
# Report: docs/engineering/_reports/graph81_g3_pgvector_<stamp>.{log,json}
set -euo pipefail
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
IDS_JSON="$ROOT/avrag-rs/tests/rag_quality/fixtures/graph81_question_ids.json"
STAMP="$(date +%Y%m%d-%H%M%S)"
OUT_DIR="$ROOT/docs/engineering/_reports"
mkdir -p "$OUT_DIR"
LOG="$OUT_DIR/graph81_g3_pgvector_${STAMP}.log"
META="$OUT_DIR/graph81_g3_pgvector_${STAMP}.json"

[[ -f "$IDS_JSON" ]] || { echo "missing $IDS_JSON" >&2; exit 1; }

# Build E2E_QUESTIONS list (comma-separated ids).
SLICE="${G3_SLICE:-slice}"
mapfile -t ALL_IDS < <(python3 - <<PY
import json
from pathlib import Path
raw = json.load(open("$IDS_JSON"))
# support shapes: {"ids":[...]} or {"e2e_questions":"a,b"} or list
if isinstance(raw, list):
    ids = [str(x) for x in raw]
elif isinstance(raw, dict):
    if "e2e_questions" in raw:
        v = raw["e2e_questions"]
        ids = [x.strip() for x in str(v).split(",") if x.strip()]
    elif "ids" in raw:
        ids = [str(x) for x in raw["ids"]]
    elif "questions" in raw:
        ids = [str(x) for x in raw["questions"]]
    else:
        # pick first list-valued key
        ids = []
        for k, v in raw.items():
            if isinstance(v, list) and v and not isinstance(v[0], dict):
                ids = [str(x) for x in v]
                break
        if not ids:
            raise SystemExit(f"cannot parse ids from {raw.keys()}")
else:
    raise SystemExit("unexpected json")
print("\n".join(ids))
PY
)

N_ALL="${#ALL_IDS[@]}"
if [[ "$SLICE" == "full" ]]; then
  SELECTED=("${ALL_IDS[@]}")
else
  # Default: first 12 (stable, cheap). Override: G3_N=20
  N="${G3_N:-12}"
  SELECTED=("${ALL_IDS[@]:0:N}")
fi
export E2E_QUESTIONS
E2E_QUESTIONS="$(IFS=,; echo "${SELECTED[*]}")"
N_SEL="${#SELECTED[@]}"

export E2E_MODE=nightly
export E2E_CONCURRENCY="${E2E_CONCURRENCY:-4}"
export CARGO_BUILD_JOBS="${CARGO_BUILD_JOBS:-2}"
export RETRIEVAL_BACKEND=pgvector
export DENSE_BACKEND=vgrag
export RETRIEVAL_GRAPH_AUGMENT=0
export INGESTION_TRIPLET_ENABLED="${INGESTION_TRIPLET_ENABLED:-1}"

# Prefer native desktop PG if client stack is up on 5433; else monorepo DATABASE_URL.
if [[ -f "$ROOT/desktop/runtime/client.env" ]]; then
  # shellcheck disable=SC1091
  set -a
  # shellcheck disable=SC1090
  source "$ROOT/desktop/runtime/client.env"
  set +a
fi
if [[ -f "$ROOT/avrag-rs/.env" ]]; then
  set -a
  # shellcheck disable=SC1091
  source "$ROOT/avrag-rs/.env"
  set +a
fi
# Force pgvector after .env (monorepo may default milvus).
export RETRIEVAL_BACKEND=pgvector
export DENSE_BACKEND=vgrag

python3 - <<PY >"$META"
import json, os
meta = {
  "stamp": "$STAMP",
  "arm": "D1-pgvector",
  "retrieval_backend": "pgvector",
  "dense_backend": "vgrag",
  "slice": "$SLICE",
  "n_selected": $N_SEL,
  "n_graph81_total": $N_ALL,
  "e2e_questions": os.environ.get("E2E_QUESTIONS", ""),
  "milvus_d1_reference": {"pass": 78, "n": 81, "source": "docs/engineering/_reports/graph81_baselines_summary.tsv / VGRAG accept plan"},
  "pass_criteria": {
    "note": "Compare pass rate to Milvus D1 78/81 (~96%). Slice is diagnostic; full is gate.",
    "slice_soft": "manual review; flag if pass_rate < 0.75 on slice",
    "full_hard": "pass_rate >= 0.90 * milvus_d1 (≈0.87) or absolute >= 70/81",
  },
  "status": "prepared",
}
print(json.dumps(meta, indent=2, ensure_ascii=False))
PY

echo "[g3] meta -> $META"
echo "[g3] n=$N_SEL / $N_ALL backend=pgvector dense=vgrag"
echo "[g3] questions=$E2E_QUESTIONS"

if [[ "${G3_DRY:-0}" == "1" ]]; then
  echo "[g3] dry-run only"
  exit 0
fi

# Delegate to graph81 runner machinery if present; else print instructions.
if [[ -x "$ROOT/scripts/run-graph81-baseline.sh" ]] || [[ -f "$ROOT/scripts/run-graph81-baseline.sh" ]]; then
  # Inline D1 env already set; call the REST/e2e path used by baseline script.
  # Prefer existing baseline entry if it accepts env overrides.
  {
    echo "=== G3 pgvector graph81 $STAMP ==="
    echo "RETRIEVAL_BACKEND=$RETRIEVAL_BACKEND DENSE_BACKEND=$DENSE_BACKEND"
    echo "E2E_QUESTIONS=$E2E_QUESTIONS"
    # Reuse baseline D1 script body by sourcing case D1 then invoking the same
    # cargo test entry the baseline script uses.
    if rg -q 'cargo test' "$ROOT/scripts/run-graph81-baseline.sh" 2>/dev/null; then
      # Extract and run via wrapping: set env then exec remaining of D1 path
      bash "$ROOT/scripts/run-graph81-baseline.sh" D1
    else
      echo "[g3] run-graph81-baseline.sh present but no cargo test line; see script"
      bash "$ROOT/scripts/run-graph81-baseline.sh" D1
    fi
  } 2>&1 | tee "$LOG"
else
  echo "[g3] missing run-graph81-baseline.sh" | tee "$LOG"
  exit 1
fi

# Append result stub for human fill if judge output not auto-parsed
python3 - <<PY
import json
from pathlib import Path
meta = json.loads(Path("$META").read_text())
meta["status"] = "ran"
meta["log"] = "$LOG"
Path("$META").write_text(json.dumps(meta, indent=2, ensure_ascii=False) + "\n")
print("[g3] updated", "$META")
PY
