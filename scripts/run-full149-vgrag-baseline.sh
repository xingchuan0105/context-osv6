#!/usr/bin/env bash
# Full-149 product baseline under VGRAG (no re-ingest).
# LLM load split:
#   - Agent / chat / e2e: OpenCode Go deepseek-v4-flash (primary)
#   - Judge: Ollama Cloud deepseek-v4-flash:0731-cloud (primary)
#   - Cross-failover: each has the other as FALLBACKS (rate-limit / 5xx)
# Concurrency backoff: 12 → 10 → 8 on non-zero exit (or panic / incomplete run).
#
# Usage:
#   bash scripts/run-full149-vgrag-baseline.sh
# Env overrides: E2E_ABORT_AFTER_CONSECUTIVE_FAILS, SILENT/TOTAL caps via test-full149.sh
set -euo pipefail
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
ENV_FILE="$ROOT/avrag-rs/.env"
if [[ -f "$ENV_FILE" ]]; then
  set -a
  # shellcheck disable=SC1090
  source "$ENV_FILE"
  set +a
fi

: "${OPENCODE_GO_API_KEY:?set OPENCODE_GO_API_KEY in avrag-rs/.env}"
: "${OPENCODE_GO_BASE_URL:=https://opencode.ai/zen/go/v1}"
: "${OLLAMA_API_KEY:?set OLLAMA_API_KEY in avrag-rs/.env}"
: "${OLLAMA_HOST:=https://ollama.com}"
OLLAMA_V1="${OLLAMA_HOST%/}/v1"
OLLAMA_FLASH="${OLLAMA_MODEL:-deepseek-v4-flash:0731-cloud}"
GO_FLASH=deepseek-v4-flash

# Product retrieval path
export DENSE_BACKEND=vgrag
export RETRIEVAL_GRAPH_AUGMENT=0
unset E2E_FORCE_INGEST 2>/dev/null || true
export E2E_MODE=nightly
export CARGO_BUILD_JOBS="${CARGO_BUILD_JOBS:-2}"
export E2E_ABORT_AFTER_CONSECUTIVE_FAILS="${E2E_ABORT_AFTER_CONSECUTIVE_FAILS:-8}"
export RAG_EVAL_V2="${RAG_EVAL_V2:-1}"

# --- Agent side: OpenCode Go Flash ---
export AGENT_LLM_BASE_URL="$OPENCODE_GO_BASE_URL"
export AGENT_LLM_API_KEY="$OPENCODE_GO_API_KEY"
export AGENT_LLM_MODEL="$GO_FLASH"
export E2E_LLM_BASE_URL="$OPENCODE_GO_BASE_URL"
export E2E_LLM_API_KEY="$OPENCODE_GO_API_KEY"
export E2E_LLM_MODEL="$GO_FLASH"
export CHAT_LLM_BASE_URL="$OPENCODE_GO_BASE_URL"
export CHAT_LLM_API_KEY="$OPENCODE_GO_API_KEY"
export CHAT_LLM_MODEL="$GO_FLASH"
export SEARCH_LLM_BASE_URL="$OPENCODE_GO_BASE_URL"
export SEARCH_LLM_API_KEY="$OPENCODE_GO_API_KEY"
export SEARCH_LLM_MODEL="$GO_FLASH"

# --- Judge side: Ollama Cloud Flash 0731 ---
export JUDGE_LLM_BASE_URL="$OLLAMA_V1"
export JUDGE_LLM_API_KEY="$OLLAMA_API_KEY"
export JUDGE_LLM_MODEL="$OLLAMA_FLASH"
export MEMORY_LLM_BASE_URL="$OLLAMA_V1"
export MEMORY_LLM_API_KEY="$OLLAMA_API_KEY"
export MEMORY_LLM_MODEL="$OLLAMA_FLASH"

# Cross-failover (JSON must be single-line for env)
export AGENT_LLM_FALLBACKS="[{\"base_url\":\"${OLLAMA_V1}\",\"api_key\":\"${OLLAMA_API_KEY}\",\"model\":\"${OLLAMA_FLASH}\"}]"
export JUDGE_LLM_FALLBACKS="[{\"base_url\":\"${OPENCODE_GO_BASE_URL}\",\"api_key\":\"${OPENCODE_GO_API_KEY}\",\"model\":\"${GO_FLASH}\"}]"
export E2E_LLM_FALLBACKS="$AGENT_LLM_FALLBACKS"
export CHAT_LLM_FALLBACKS="$AGENT_LLM_FALLBACKS"

LOG_DIR="$ROOT/output/runtime-logs"
mkdir -p "$LOG_DIR"
STAMP="$(date -u +%Y%m%d-%H%M%S)"
SUMMARY="$LOG_DIR/full149_vgrag_baseline_${STAMP}.summary.txt"

echo "[full149-vgrag] DENSE_BACKEND=vgrag no FORCE_INGEST" | tee "$SUMMARY"
echo "[full149-vgrag] agent=OpenCodeGo/$GO_FLASH judge=Ollama/$OLLAMA_FLASH" | tee -a "$SUMMARY"
echo "[full149-vgrag] concurrency ladder 12→10→8" | tee -a "$SUMMARY"

run_once() {
  local conc="$1"
  local log="$LOG_DIR/full149_vgrag_c${conc}_${STAMP}.log"
  export E2E_CONCURRENCY="$conc"
  echo "[full149-vgrag] start concurrency=$conc log=$log" | tee -a "$SUMMARY"
  set +e
  bash "$ROOT/scripts/test-full149.sh" >"$log" 2>&1
  local rc=$?
  set -e
  echo "[full149-vgrag] concurrency=$conc exit=$rc" | tee -a "$SUMMARY"
  # Hard fail signals
  if grep -qE 'panicked at|fatal runtime error' "$log" 2>/dev/null; then
    echo "[full149-vgrag] panic detected" | tee -a "$SUMMARY"
    return 1
  fi
  # Incomplete: expect 149 examples when no E2E_QUESTIONS filter
  if ! grep -qE 'Total examples:\s*149' "$log" 2>/dev/null; then
    if grep -qE 'circuit-breaker|E2E_ABORT' "$log" 2>/dev/null; then
      echo "[full149-vgrag] incomplete (circuit/abort) — backoff" | tee -a "$SUMMARY"
      return 1
    fi
    if [[ $rc -ne 0 ]]; then
      echo "[full149-vgrag] incomplete + nonzero exit — backoff" | tee -a "$SUMMARY"
      return 1
    fi
  fi
  if [[ $rc -ne 0 ]]; then
    return "$rc"
  fi
  # Capture v2 PASS line if present
  rg -n 'labels:.*PASS=|Total examples:|test result:|mean answer' "$log" | tail -20 | tee -a "$SUMMARY" || true
  echo "[full149-vgrag] SUCCESS concurrency=$conc log=$log" | tee -a "$SUMMARY"
  echo "$log" >"$LOG_DIR/full149_vgrag_baseline_latest.path"
  return 0
}

final_rc=1
for conc in 12 10 8; do
  if run_once "$conc"; then
    final_rc=0
    break
  fi
  echo "[full149-vgrag] will retry at lower concurrency if any remain" | tee -a "$SUMMARY"
done

echo "[full149-vgrag] done final_rc=$final_rc summary=$SUMMARY" | tee -a "$SUMMARY"
exit "$final_rc"
