#!/usr/bin/env bash
# Full-149: Agent = OpenCode Go (deepseek-v4-flash).
# Live JUDGE_LLM is only a harness placeholder (OpenCode deepseek-v4-flash) so
# the pipeline writes artifacts; authoritative labels come from Grok Build CLI
# offline pass (scripts/cli_judge_full149.py or session agent reading artifacts).
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
RUN_TAG="full149_opencode_cli_judge_${STAMP}"
LOG="$LOG_DIR/${RUN_TAG}.log"
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

# Placeholder live judge (harness needs a working LLM seat). Final authority = CLI.
export JUDGE_LLM_BASE_URL="$OPENCODE_GO_BASE_URL"
export JUDGE_LLM_API_KEY="$OPENCODE_GO_API_KEY"
export JUDGE_LLM_MODEL="${JUDGE_LLM_PLACEHOLDER_MODEL:-deepseek-v4-flash}"
export JUDGE_LLM_TIMEOUT_MS="${JUDGE_LLM_TIMEOUT_MS:-90000}"

export E2E_MODE=nightly
export E2E_CONCURRENCY="${E2E_CONCURRENCY:-8}"
export E2E_ABORT_AFTER_CONSECUTIVE_FAILS="${E2E_ABORT_AFTER_CONSECUTIVE_FAILS:-0}"
export CARGO_BUILD_JOBS="${CARGO_BUILD_JOBS:-2}"
export RAG_EVAL_V2=1
unset E2E_FORCE_INGEST || true

SILENT_CAP="${E2E_FULL149_SILENT_CAP_SECS:-900}"
TOTAL_CAP="${E2E_FULL149_TOTAL_CAP_SECS:-14400}"

{
  echo "[run] stamp=$STAMP"
  echo "[run] agent=$AGENT_LLM_BASE_URL model=$AGENT_LLM_MODEL"
  echo "[run] live_judge_placeholder=$JUDGE_LLM_BASE_URL model=$JUDGE_LLM_MODEL"
  echo "[run] authoritative_judge=grok_build_cli (offline after artifacts)"
  echo "[run] concurrency=$E2E_CONCURRENCY breaker=$E2E_ABORT_AFTER_CONSECUTIVE_FAILS"
  echo "[run] log=$LOG"
} | tee -a "$LOG"

rc=0
timeout "$TOTAL_CAP" "$ROOT/scripts/with-watchdog.sh" "$LOG" "$SILENT_CAP" -- \
  cargo test -p app --test product_e2e realistic_corpus_full_eval \
  --features product-e2e -- --ignored --test-threads=1 --nocapture || rc=$?
echo "[run] phase_exit=$rc" | tee -a "$LOG"

# Prefer run_dir created during this session (log contains realistic_corpus lines).
RUN_DIR=""
if grep -q '\[realistic_corpus\]' "$LOG" 2>/dev/null; then
  RUN_DIR=$(ls -1dt "$ROOT/avrag-rs/crates/app/tests/e2e_output/rag_eval_v2"/v2_* 2>/dev/null | head -1 || true)
fi
echo "[run] run_dir=$RUN_DIR" | tee -a "$LOG"
if [ -z "$RUN_DIR" ] || [ ! -f "$RUN_DIR/summary.json" ]; then
  echo "[run] ERROR: no summary.json for this session (rc=$rc)" | tee -a "$LOG"
  exit "${rc:-1}"
fi
echo "$RUN_DIR" > "$STATE_DIR/run_dir.txt"
cp -a "$RUN_DIR/summary.json" "$STATE_DIR/live_summary.json"
echo "[run] ready_for_cli_judge run_dir=$RUN_DIR" | tee -a "$LOG"
echo "[run] done exit=$rc" | tee -a "$LOG"
exit 0
