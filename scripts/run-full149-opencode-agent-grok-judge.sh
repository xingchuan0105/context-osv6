#!/usr/bin/env bash
# Full-149 realistic_corpus_full_eval:
#   Agent  = OpenCode Go (deepseek-v4-flash)
#   Judge  = Grok (OpenCode Go model grok-4.5)
# No force re-ingest by default. Merges summary path under output/runtime-logs.
set -uo pipefail
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT/avrag-rs"
set -a
# shellcheck disable=SC1091
source "$ROOT/avrag-rs/.env"
set +a

LOG_DIR="$ROOT/output/runtime-logs"
mkdir -p "$LOG_DIR"
STAMP="$(date -u +%Y%m%d-%H%M%S)"
RUN_TAG="full149_opencode_grok_${STAMP}"
LOG="$LOG_DIR/${RUN_TAG}.log"
SUMMARY_OUT="$LOG_DIR/${RUN_TAG}_summary.json"
STATE_DIR="$LOG_DIR/${RUN_TAG}_state"
mkdir -p "$STATE_DIR"

export AGENT_LLM_BASE_URL="${OPENCODE_GO_BASE_URL:?OPENCODE_GO_BASE_URL required}"
export AGENT_LLM_API_KEY="${OPENCODE_GO_API_KEY:?OPENCODE_GO_API_KEY required}"
export AGENT_LLM_MODEL="${AGENT_LLM_MODEL_OVERRIDE:-deepseek-v4-flash}"
export AGENT_LLM_API_STYLE="${AGENT_LLM_API_STYLE:-openai}"
export AGENT_LLM_ENABLE_THINKING="${AGENT_LLM_ENABLE_THINKING:-false}"

export E2E_LLM_BASE_URL="$AGENT_LLM_BASE_URL"
export E2E_LLM_API_KEY="$AGENT_LLM_API_KEY"
export E2E_LLM_MODEL="$AGENT_LLM_MODEL"
export CHAT_LLM_BASE_URL="$AGENT_LLM_BASE_URL"
export CHAT_LLM_API_KEY="$AGENT_LLM_API_KEY"
export CHAT_LLM_MODEL="$AGENT_LLM_MODEL"

# Batch judge: Grok via OpenCode Go catalog (id: grok-4.5)
export JUDGE_LLM_BASE_URL="$OPENCODE_GO_BASE_URL"
export JUDGE_LLM_API_KEY="$OPENCODE_GO_API_KEY"
export JUDGE_LLM_MODEL="${JUDGE_LLM_MODEL_OVERRIDE:-grok-4.5}"
export JUDGE_LLM_TIMEOUT_MS="${JUDGE_LLM_TIMEOUT_MS:-120000}"

export E2E_MODE=nightly
export E2E_CONCURRENCY="${E2E_CONCURRENCY:-8}"
export E2E_ABORT_AFTER_CONSECUTIVE_FAILS="${E2E_ABORT_AFTER_CONSECUTIVE_FAILS:-0}"
export CARGO_BUILD_JOBS="${CARGO_BUILD_JOBS:-2}"
export RAG_EVAL_V2="${RAG_EVAL_V2:-1}"
# Reuse corpus unless caller sets E2E_FORCE_INGEST=1
if [ -z "${E2E_FORCE_INGEST+x}" ]; then
  unset E2E_FORCE_INGEST || true
fi

SILENT_CAP="${E2E_FULL149_SILENT_CAP_SECS:-900}"
TOTAL_CAP="${E2E_FULL149_TOTAL_CAP_SECS:-14400}"

{
  echo "[run] stamp=$STAMP"
  echo "[run] agent=$AGENT_LLM_BASE_URL model=$AGENT_LLM_MODEL"
  echo "[run] judge=$JUDGE_LLM_BASE_URL model=$JUDGE_LLM_MODEL"
  echo "[run] concurrency=$E2E_CONCURRENCY force_ingest=${E2E_FORCE_INGEST:-0} breaker=$E2E_ABORT_AFTER_CONSECUTIVE_FAILS"
  echo "[run] log=$LOG"
} | tee -a "$LOG"

rc=0
timeout "$TOTAL_CAP" "$ROOT/scripts/with-watchdog.sh" "$LOG" "$SILENT_CAP" -- \
  cargo test -p app --test product_e2e realistic_corpus_full_eval \
  --features product-e2e -- --ignored --test-threads=1 --nocapture || rc=$?
echo "[run] phase_exit=$rc" | tee -a "$LOG"

RUN_DIR=$(ls -1dt "$ROOT/avrag-rs/crates/app/tests/e2e_output/rag_eval_v2"/v2_* 2>/dev/null | head -1 || true)
echo "[run] run_dir=$RUN_DIR" | tee -a "$LOG"
if [ -n "$RUN_DIR" ] && [ -f "$RUN_DIR/summary.json" ]; then
  cp -a "$RUN_DIR/summary.json" "$SUMMARY_OUT"
  echo "$RUN_DIR" > "$STATE_DIR/run_dir.txt"
  python3 - <<'PY' "$RUN_DIR" "$SUMMARY_OUT" | tee -a "$LOG"
import json, sys
from pathlib import Path
run, out = Path(sys.argv[1]), Path(sys.argv[2])
s = json.loads((run / "summary.json").read_text())
summ = s.get("summary") or s
print("[run] total=", summ.get("total"))
print("[run] label_counts=", summ.get("label_counts"))
print("[run] judge_ok=", summ.get("judge_ok"), "judge_error=", summ.get("judge_error"))
print("[run] summary_copy=", out)
PY
else
  echo "[run] ERROR: no summary.json" | tee -a "$LOG"
fi

# Optional post-pass: rejudge any remaining JUDGE_ERROR with same Grok seat
if [ "${E2E_REJUDGE_AFTER:-1}" = "1" ] && [ -n "$RUN_DIR" ]; then
  echo "[run] rejudge JUDGE_ERROR with $JUDGE_LLM_MODEL …" | tee -a "$LOG"
  cargo run -p rag_quality --bin rejudge -- "$RUN_DIR" >>"$LOG" 2>&1 || true
  if [ -f "$RUN_DIR/summary.json" ]; then
    cp -a "$RUN_DIR/summary.json" "$SUMMARY_OUT"
  fi
  # re-aggregate labels from artifacts after rejudge
  python3 - <<'PY' "$RUN_DIR" "$LOG_DIR/${RUN_TAG}_after_rejudge.json" | tee -a "$LOG"
import json, sys
from pathlib import Path
from collections import Counter
run, out = Path(sys.argv[1]), Path(sys.argv[2])
c = Counter()
for p in run.glob("q*.artifact.json"):
    a = json.loads(p.read_text())
    lab = (a.get("score_v2") or {}).get("label") or "?"
    c[str(lab).upper()] += 1
pass_n = c.get("PASS", 0)
total = sum(c.values())
result = {
    "run_dir": str(run),
    "merged_total_labeled": total,
    "merged_pass": pass_n,
    "merged_pass_rate": (pass_n / total) if total else None,
    "merged_label_counts": dict(c),
}
out.write_text(json.dumps(result, ensure_ascii=False, indent=2))
print("[run] after_rejudge", json.dumps(result, ensure_ascii=False))
PY
fi

echo "[run] done exit=$rc" | tee -a "$LOG"
exit 0
