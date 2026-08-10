#!/usr/bin/env bash
# Full-149: Agent+Judge = OpenCode Go deepseek-v4-flash, concurrency=6.
# Reuse corpus (no force re-ingest). Breaker off so the full slate finishes.
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
RUN_TAG="full149_opencode_dsflash_c6_${STAMP}"
LOG="$LOG_DIR/${RUN_TAG}.log"
CONSOLE="$LOG_DIR/${RUN_TAG}.console"
PIDFILE="$LOG_DIR/${RUN_TAG}.pid"
SUMMARY_OUT="$LOG_DIR/${RUN_TAG}_summary.json"

export AGENT_LLM_BASE_URL="${OPENCODE_GO_BASE_URL:?OPENCODE_GO_BASE_URL required}"
export AGENT_LLM_API_KEY="${OPENCODE_GO_API_KEY:?OPENCODE_GO_API_KEY required}"
export AGENT_LLM_MODEL="${AGENT_LLM_MODEL_OVERRIDE:-deepseek-v4-flash}"
export AGENT_LLM_API_STYLE="${AGENT_LLM_API_STYLE:-openai}"
# Force false after .env: product E2E should not burn thinking tokens on OpenCode Go flash
export AGENT_LLM_ENABLE_THINKING=false

export E2E_LLM_BASE_URL="$AGENT_LLM_BASE_URL"
export E2E_LLM_API_KEY="$AGENT_LLM_API_KEY"
export E2E_LLM_MODEL="$AGENT_LLM_MODEL"
export CHAT_LLM_BASE_URL="$AGENT_LLM_BASE_URL"
export CHAT_LLM_API_KEY="$AGENT_LLM_API_KEY"
export CHAT_LLM_MODEL="$AGENT_LLM_MODEL"

export JUDGE_LLM_BASE_URL="$OPENCODE_GO_BASE_URL"
export JUDGE_LLM_API_KEY="$OPENCODE_GO_API_KEY"
export JUDGE_LLM_MODEL="${JUDGE_LLM_MODEL_OVERRIDE:-deepseek-v4-flash}"
export JUDGE_LLM_TIMEOUT_MS="${JUDGE_LLM_TIMEOUT_MS:-120000}"

export E2E_MODE=nightly
export E2E_CONCURRENCY="${E2E_CONCURRENCY:-6}"
export E2E_ABORT_AFTER_CONSECUTIVE_FAILS="${E2E_ABORT_AFTER_CONSECUTIVE_FAILS:-0}"
export CARGO_BUILD_JOBS="${CARGO_BUILD_JOBS:-2}"
export RAG_EVAL_V2="${RAG_EVAL_V2:-1}"
if [ -z "${E2E_FORCE_INGEST+x}" ]; then
  unset E2E_FORCE_INGEST || true
fi

SILENT_CAP="${E2E_FULL149_SILENT_CAP_SECS:-900}"
TOTAL_CAP="${E2E_FULL149_TOTAL_CAP_SECS:-14400}"

{
  echo "[run] stamp=$STAMP tag=$RUN_TAG"
  echo "[run] agent=$AGENT_LLM_BASE_URL model=$AGENT_LLM_MODEL thinking=$AGENT_LLM_ENABLE_THINKING"
  echo "[run] judge=$JUDGE_LLM_BASE_URL model=$JUDGE_LLM_MODEL"
  echo "[run] concurrency=$E2E_CONCURRENCY force_ingest=${E2E_FORCE_INGEST:-0} breaker=$E2E_ABORT_AFTER_CONSECUTIVE_FAILS"
  echo "[run] silent_cap=${SILENT_CAP}s total_cap=${TOTAL_CAP}s"
  echo "[run] log=$LOG"
} | tee "$LOG" | tee "$CONSOLE"

rc=0
timeout "$TOTAL_CAP" "$ROOT/scripts/with-watchdog.sh" "$LOG" "$SILENT_CAP" -- \
  cargo test -p app --test product_e2e realistic_corpus_full_eval \
  --features product-e2e -- --ignored --test-threads=1 --nocapture || rc=$?
echo "[run] phase_exit=$rc" | tee -a "$LOG" | tee -a "$CONSOLE"

RUN_DIR=$(ls -1dt "$ROOT/avrag-rs/crates/app/tests/e2e_output/rag_eval_v2"/v2_* 2>/dev/null | head -1 || true)
echo "[run] run_dir=$RUN_DIR" | tee -a "$LOG" | tee -a "$CONSOLE"
if [ -n "$RUN_DIR" ] && [ -f "$RUN_DIR/summary.json" ]; then
  cp -a "$RUN_DIR/summary.json" "$SUMMARY_OUT"
  python3 - <<PY | tee -a "$LOG" | tee -a "$CONSOLE"
import json
from pathlib import Path
run = Path(r"""$RUN_DIR""")
s = json.loads((run / "summary.json").read_text())
summ = s.get("summary") or s
print("[run] total=", summ.get("total"))
print("[run] label_counts=", summ.get("label_counts"))
print("[run] judge_ok=", summ.get("judge_ok"), "judge_error=", summ.get("judge_error"))
print("[run] summary_copy=", r"""$SUMMARY_OUT""")
PY
else
  echo "[run] ERROR: no summary.json" | tee -a "$LOG" | tee -a "$CONSOLE"
fi
echo "[run] done exit=$rc" | tee -a "$LOG" | tee -a "$CONSOLE"
exit "$rc"
