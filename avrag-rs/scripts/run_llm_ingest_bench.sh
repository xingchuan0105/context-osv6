#!/usr/bin/env bash
# LLM ingest benchmark: same file -> the production windowed PS+triplet stage
# (bins/worker pipeline::windowed_llm) per model. Records stage timings and
# dumps the real artifacts (summary.md / triplets.json / timings.json) per leg.
# No server, no worker subprocess, no queue, no retrieval probe; switching
# models is env-only (zero rebuild).
#
# Usage:
#   ./scripts/run_llm_ingest_bench.sh                       # default 3 models
#   BENCH_MODELS="qwen3.8-flash deepseek-v4-flash" ./scripts/run_llm_ingest_bench.sh
set -euo pipefail
cd "$(dirname "$0")/.."
# Non-login shell: markitdown lives in ~/.local/bin (parse stage spawns it).
export PATH="$HOME/.cargo/bin:$HOME/.local/bin:$PATH"
set -a
# shellcheck disable=SC1091
source .env
set +a

RD="${BENCH_RESULTS_DIR:-$PWD/crates/app/tests/e2e_output/llm_bench_$(date +%Y%m%d_%H%M%S)}"
mkdir -p "$RD"
MODELS=(${BENCH_MODELS:-qwen3.7-flash qwen3.8-flash deepseek-v4-flash})

for MODEL in "${MODELS[@]}"; do
  case "$MODEL" in
    deepseek-*|deepseek:*)
      BASE="${BENCH_OLLAMA_BASE_URL:-https://ollama.com/v1}"
      KEY="${OLLAMA_API_KEY:?OLLAMA_API_KEY required for $MODEL}"
      STYLE="openai"
      ;;
    *)
      BASE="$INGESTION_LLM_BASE_URL"
      KEY="$INGESTION_LLM_API_KEY"
      STYLE="${INGESTION_LLM_API_STYLE:-dashscope_responses}"
      ;;
  esac
  SLUG="${MODEL//[:\/.]/_}"
  OUT="$RD/$SLUG"
  mkdir -p "$OUT"
  echo "===== bench $MODEL -> $OUT ($(date +%T)) ====="
  INGESTION_LLM_BASE_URL="$BASE" \
  INGESTION_LLM_API_KEY="$KEY" \
  INGESTION_LLM_MODEL="$MODEL" \
  INGESTION_LLM_API_STYLE="$STYLE" \
  INGESTION_LLM_TIMEOUT_MS="${BENCH_LLM_TIMEOUT_MS:-300000}" \
  INGESTION_LLM_ENABLE_THINKING="${BENCH_ENABLE_THINKING:-false}" \
  INGESTION_TRIPLET_ENABLED="${BENCH_TRIPLET_ENABLED:-1}" \
  INGESTION_TRIPLET_TOKEN_BUDGET="${INGESTION_TRIPLET_TOKEN_BUDGET:-3000}" \
  BENCH_OUT_DIR="$OUT" \
  cargo test -p avrag-worker --lib llm_bench -- --ignored --nocapture \
    >"$OUT/run.log" 2>&1 || echo "bench $MODEL FAILED (see $OUT/run.log)"
  grep -o 'BENCH_RESULT=.*' "$OUT/run.log" | tail -1 | sed 's/^BENCH_RESULT=//' \
    >"$OUT/result.json" || true
  [ -s "$OUT/result.json" ] && echo "  -> $(cat "$OUT/result.json")" \
    || echo "  -> no BENCH_RESULT (FAILED)"
done

echo "===== summary ====="
python3 - "$RD" <<'PY'
import glob, json, sys
from pathlib import Path

rows = []
for f in sorted(Path(sys.argv[1]).glob("*/result.json")):
    if f.stat().st_size:
        rows.append(json.loads(f.read_text(encoding="utf-8")))
rows.sort(key=lambda r: r.get("llm_secs", 9999))
for r in rows:
    print(f"  {r['model']:26} parse={r['parse_secs']:5.1f}s  llm={r['llm_secs']:6.1f}s  "
          f"windows={r['windows']:3}  prompt={r['prompt_tokens']:7}  compl={r['completion_tokens']:6}  "
          f"triplets={r['triplet_count']:4}  summary_chars={r['summary_chars']}")
print(f"\nartifacts: {sys.argv[1]}/<model>/{{summary.md, triplets.json, timings.json}}")
PY
