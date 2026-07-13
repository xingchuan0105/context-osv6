#!/usr/bin/env bash
# L3-full quality — rag_quality_prod default gate (not DR2 default).
#
# Included (filter llm_real::rag_quality_prod, skip triplet_benchmark):
#   production_rag_evaluator, rag_system_prompt_smoke_v5, rag_tools_golden_set,
#   realistic_corpus_full_eval
# Excluded: triplet_benchmark_* — needs dedicated env (see scripts/benchmark_triplet_models.sh).
#
# MUST stop product-dev-up avrag-worker first (preflight asserts no external workers).
#   pkill -x avrag-worker   # only if you intend to stop dev stack
set -euo pipefail
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
# shellcheck source=pyramid-lib.sh
source "${ROOT}/scripts/pyramid-lib.sh"
cd "$ROOT/avrag-rs"

echo "[PYRAMID] layer=L3-full-quality begin"
export E2E_MODE="${E2E_MODE:-nightly}"
pyramid_export_llm_keys_if_needed

echo "==> L3 rag_quality_prod (skip triplet_benchmark; external worker must be stopped)"
# --skip: libtest substring match; keeps default quality free of env-gated benchmarks.
cargo test -p app --test product_e2e --features product-e2e llm_real::rag_quality_prod -- \
  --ignored --test-threads=1 --nocapture --skip triplet_benchmark \
  || pyramid_fail L3-full-quality S6 \
    "stop host avrag-worker; E2E_MODE=nightly cargo test -p app --test product_e2e llm_real::rag_quality_prod -- --ignored --test-threads=1 --skip triplet_benchmark" \
    "quality release gate (triplet: scripts/benchmark_triplet_models.sh)"

pyramid_ok L3-full-quality
echo "L3 quality OK"
