#!/usr/bin/env bash
# Benchmark triplet extraction on huawei_ipd_370_activities.txt across LLM providers.
#
# Model spec format: provider:model_id:token_budget
#   provider = dashscope | gemini | deepseek
#
# Examples:
#   ./scripts/benchmark_triplet_models.sh
#   BENCHMARK_MODELS="gemini:gemini-3.1-flash-lite:3000 deepseek:deepseek-v4-flash:3000" ./scripts/benchmark_triplet_models.sh
#   MERGE_SUMMARY=/tmp/prior/summary.jsonl ./scripts/benchmark_triplet_models.sh
set -euo pipefail

cd "$(dirname "$0")/.."

if [[ -f .env ]]; then
  set -a
  # shellcheck disable=SC1091
  source .env
  set +a
fi

RESULTS_DIR="${RESULTS_DIR:-/tmp/triplet_benchmark_$(date +%Y%m%d_%H%M%S)}"
mkdir -p "$RESULTS_DIR"
SUMMARY="$RESULTS_DIR/summary.jsonl"
: >"$SUMMARY"

# Default: full cross-provider sweep (override with BENCHMARK_MODELS)
if [[ -n "${BENCHMARK_MODELS:-}" ]]; then
  # shellcheck disable=SC2206
  MODELS=($BENCHMARK_MODELS)
else
  MODELS=(
    "dashscope:qwen3.6-flash:3000"
    "dashscope:qwen3.5-flash:3000"
    "dashscope:qwen-doc-turbo:200000"
    "gemini:gemini-3.1-flash-lite:3000"
    "deepseek:deepseek-v4-flash:3000"
  )
fi

DEEPSEEK_TRIPLET_API_KEY="${TRIPLET_LLM_API_KEY:-}"

configure_triplet_provider() {
  local provider="$1"
  case "$provider" in
    dashscope|qwen)
      : "${DASHSCOPE_API_KEY:?DASHSCOPE_API_KEY required for dashscope models}"
      export TRIPLET_LLM_BASE_URL="https://dashscope.aliyuncs.com/compatible-mode/v1"
      export TRIPLET_LLM_API_KEY="$DASHSCOPE_API_KEY"
      ;;
    gemini|google)
      : "${GEMINI_API_KEY:?GEMINI_API_KEY required for gemini models (see ai.google.dev/gemini-api/docs/api-key)}"
      export TRIPLET_LLM_BASE_URL="https://generativelanguage.googleapis.com/v1beta/openai"
      export TRIPLET_LLM_API_KEY="$GEMINI_API_KEY"
      ;;
    deepseek)
      : "${DEEPSEEK_TRIPLET_API_KEY:?TRIPLET_LLM_API_KEY required in .env for deepseek}"
      export TRIPLET_LLM_BASE_URL="https://api.deepseek.com"
      export TRIPLET_LLM_API_KEY="$DEEPSEEK_TRIPLET_API_KEY"
      ;;
    *)
      echo "unknown provider: $provider (use dashscope|gemini|deepseek)" >&2
      return 1
      ;;
  esac
  : "${TRIPLET_LLM_ENABLE_THINKING:=false}"
  export TRIPLET_LLM_ENABLE_THINKING
}

run_model() {
  local spec="$1"
  local provider model budget
  IFS=: read -r provider model budget <<<"$spec"
  local log="$RESULTS_DIR/${provider}_${model}.log"

  echo "=== Benchmark: provider=$provider model=$model token_budget=$budget ==="
  configure_triplet_provider "$provider"

  export E2E_MODE=nightly
  export TRIPLET_BENCHMARK_PROVIDER="$provider"
  export TRIPLET_BENCHMARK_MODEL="$model"
  export RAG_SMOKE_SINGLE_DOC=huawei_ipd_370_activities.txt
  export RAG_QUALITY_SMOKE_FORCE_INGEST=1
  export RAG_QUALITY_SMOKE_TRIPLET_ENABLED=1
  export INGESTION_TRIPLET_TOKEN_BUDGET="$budget"
  export TRIPLET_LLM_MODEL="$model"
  export TRIPLET_LLM_TIMEOUT_MS="${TRIPLET_LLM_TIMEOUT_MS:-180000}"

  set +e
  cargo test -p app --test product_e2e triplet_benchmark_huawei_ipd \
    --features product-e2e -- --ignored --test-threads=1 --nocapture 2>&1 | tee "$log"
  local status=${PIPESTATUS[0]}
  set -e

  local result
  result=$(grep -o 'TRIPLET_BENCHMARK_RESULT=.*' "$log" | tail -1 | sed 's/^TRIPLET_BENCHMARK_RESULT=//')
  if [[ -n "$result" ]]; then
    echo "$result" >>"$SUMMARY"
    echo "  -> $result"
  else
    echo "{\"provider\":\"$provider\",\"model\":\"$model\",\"error\":\"test_exit_$status\"}" >>"$SUMMARY"
    echo "  -> FAILED (exit $status, no TRIPLET_BENCHMARK_RESULT)"
  fi

  return "$status"
}

failures=0
echo "Building avrag-worker (triplet LLM client) ..."
cargo build -p avrag-worker -q

for spec in "${MODELS[@]}"; do
  if ! run_model "$spec"; then
    failures=$((failures + 1))
  fi
  echo
done

echo "========================================="
echo "Triplet benchmark complete"
echo "  results: $RESULTS_DIR"
echo "  summary: $SUMMARY"
echo "  failures: $failures / ${#MODELS[@]}"
echo "========================================="

COMBINED="$RESULTS_DIR/combined_summary.jsonl"
cat "$SUMMARY" >"$COMBINED"
if [[ -n "${MERGE_SUMMARY:-}" && -f "$MERGE_SUMMARY" ]]; then
  cat "$MERGE_SUMMARY" >>"$COMBINED"
fi

if [[ -s "$COMBINED" ]]; then
  echo
  echo "Model comparison (ingest_secs | graph | entities | recall | label):"
  python3 - <<PY "$COMBINED"
import json, sys
from pathlib import Path

rows = []
seen = set()
for line in Path(sys.argv[1]).read_text().splitlines():
    if not line.strip():
        continue
    r = json.loads(line)
    key = r.get("model", "?")
    if key in seen and "error" not in r:
        continue
    seen.add(key)
    rows.append(r)

rows.sort(key=lambda r: r.get("ingest_secs", 9999))

for r in rows:
    if "error" in r:
        print(f"  {r.get('model','?'):22}  ERROR {r['error']}")
        continue
    prov = r.get("provider", "")
    tag = f"{prov}/{r['model']}" if prov else r["model"]
    print(
        f"  {tag:28}  ingest={r.get('ingest_secs',0):6.1f}s  "
        f"graph={r.get('graph_passage_count',0):4}  "
        f"entities={r.get('entity_count',0):4}  "
        f"recall={r.get('recall_at_15',0)*100:5.0f}%  "
        f"faith={r.get('faithfulness',0)*100:4.0f}%  "
        f"{r.get('diagnostic_label','?')}"
    )
PY
fi

exit "$failures"
