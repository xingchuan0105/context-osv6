#!/usr/bin/env bash
# Run graph81 under one baseline after dense=VGRAG product cutover (2026-08-03).
#
# Preferred arms (product-aligned):
#   D0  pure ANN dense          (DENSE_BACKEND=ann, no side-car, no L-eval RRF)
#   D1  product VGRAG dense     (DENSE_BACKEND=vgrag default, no side-car, no L-eval RRF)
#
# Legacy arms (B0–B4) are archived comparison only — they re-enable L-eval observation
# RRF / lexical side-car and do NOT measure the product path cleanly.
#
# Usage:
#   bash scripts/run-graph81-baseline.sh D0
#   bash scripts/run-graph81-baseline.sh D1
#   bash scripts/run-graph81-baseline.sh B1   # legacy
set -euo pipefail
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
BASELINE="${1:-}"
if [[ -z "$BASELINE" ]]; then
  echo "usage: $0 D0|D1|B0|B1|B2|B3|B4" >&2
  exit 2
fi

IDS_JSON="$ROOT/avrag-rs/tests/rag_quality/fixtures/graph81_question_ids.json"
if [[ ! -f "$IDS_JSON" ]]; then
  echo "missing $IDS_JSON" >&2
  exit 1
fi
E2E_QUESTIONS="$(python3 -c "import json; print(json.load(open('$IDS_JSON'))['e2e_questions'])")"
export E2E_QUESTIONS
export E2E_MODE=nightly
export E2E_CONCURRENCY="${E2E_CONCURRENCY:-8}"
export CARGO_BUILD_JOBS="${CARGO_BUILD_JOBS:-2}"
# Reuse corpus by default. Caller: E2E_FORCE_INGEST=1 to rebuild (new ontology triplets).
if [[ -z "${E2E_FORCE_INGEST:-}" ]]; then
  unset E2E_FORCE_INGEST || true
fi

# Product defaults for all arms unless overridden.
export RETRIEVAL_GRAPH_AUGMENT=0
unset GRAPH_L_EVAL_RRF || true
unset RETRIEVAL_GRAPH_SEED || true
unset GRAPH_AUGMENT_HOPS || true
unset GRAPH_EVAL_FORCE_REQUIRED_GRAPH || true
unset GRAPH_EVAL_MODE || true
export DENSE_BACKEND=vgrag
if [[ -n "${E2E_FORCE_INGEST:-}" ]]; then
  export INGESTION_TRIPLET_ENABLED="${INGESTION_TRIPLET_ENABLED:-1}"
  export INGESTION_VLM_TRIPLET_ENABLED="${INGESTION_VLM_TRIPLET_ENABLED:-0}"
  echo "[graph81] FORCE_INGEST=1 triplet=${INGESTION_TRIPLET_ENABLED} vlm_triplet=${INGESTION_VLM_TRIPLET_ENABLED}"
fi

case "$BASELINE" in
  D0)
    # Pure dense ANN — control arm for VGRAG A/B.
    export DENSE_BACKEND=ann
    export RETRIEVAL_GRAPH_AUGMENT=0
    ;;
  D1)
    # Product path: dense = VGRAG (pool fuse + hop2), no lexical side-car, no L-eval RRF.
    export DENSE_BACKEND=vgrag
    export RETRIEVAL_GRAPH_AUGMENT=0
    ;;
  B0)
    echo "[graph81] WARN: B0 is legacy (side-car + L-eval RRF); prefer D1 for product" >&2
    export RETRIEVAL_GRAPH_AUGMENT=1
    export GRAPH_L_EVAL_RRF=1
    export GRAPH_EVAL_MODE=1
    export GRAPH_AUGMENT_HOPS=1
    export DENSE_BACKEND=ann
    ;;
  B1)
    echo "[graph81] WARN: B1 legacy graph_off ≈ D0; prefer D0" >&2
    export DENSE_BACKEND=ann
    export RETRIEVAL_GRAPH_AUGMENT=0
    export GRAPH_EVAL_MODE=1
    ;;
  B2)
    echo "[graph81] WARN: B2 obsolete (client.graph removed); maps to D0-like + L-eval" >&2
    export DENSE_BACKEND=ann
    export RETRIEVAL_GRAPH_AUGMENT=0
    export GRAPH_L_EVAL_RRF=1
    export GRAPH_EVAL_MODE=1
    ;;
  B3)
    echo "[graph81] WARN: B3 legacy hop3 side-car + L-eval; product VGRAG hop is fixed 2 inside dense" >&2
    export DENSE_BACKEND=ann
    export RETRIEVAL_GRAPH_AUGMENT=1
    export GRAPH_L_EVAL_RRF=1
    export GRAPH_EVAL_MODE=1
    export GRAPH_AUGMENT_HOPS=3
    ;;
  B4)
    echo "[graph81] WARN: B4 legacy dense_multiway seed + L-eval; product uses D1" >&2
    export DENSE_BACKEND=vgrag
    export RETRIEVAL_GRAPH_AUGMENT=0
    export GRAPH_L_EVAL_RRF=1
    export GRAPH_EVAL_MODE=1
    ;;
  B_frozen)
    echo "B_frozen is archived; run: python3 scripts/report-graph81-bfrozen.py" >&2
    exit 2
    ;;
  *)
    echo "unknown baseline: $BASELINE (use D0|D1 or legacy B0–B4)" >&2
    exit 2
    ;;
esac

LOG_DIR="$ROOT/output/runtime-logs"
mkdir -p "$LOG_DIR"
LOG="$LOG_DIR/graph81_${BASELINE}_$(date -u +%Y%m%d-%H%M%S).log"
echo "[graph81] baseline=$BASELINE questions=$E2E_QUESTIONS"
echo "[graph81] DENSE_BACKEND=${DENSE_BACKEND:-} RETRIEVAL_GRAPH_AUGMENT=${RETRIEVAL_GRAPH_AUGMENT:-} GRAPH_L_EVAL_RRF=${GRAPH_L_EVAL_RRF:-}"
echo "[graph81] log=$LOG"

cd "$ROOT"
(cd avrag-rs && CARGO_BUILD_JOBS="${CARGO_BUILD_JOBS}" cargo build -p avrag-worker 2>&1 | tail -5)

bash scripts/test-full149.sh 2>&1 | tee "$LOG"
echo "[graph81] done baseline=$BASELINE log=$LOG"
