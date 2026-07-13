#!/usr/bin/env bash
# L3-full quality — rag_quality_prod / smoke_v5 / realistic corpus (not DR2 default).
#
# MUST stop product-dev-up avrag-worker first (preflight asserts no external workers).
#   pkill -f 'target/debug/avrag-worker'   # only if you intend to stop dev stack
set -euo pipefail
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
# shellcheck source=pyramid-lib.sh
source "${ROOT}/scripts/pyramid-lib.sh"
cd "$ROOT/avrag-rs"

echo "[PYRAMID] layer=L3-full-quality begin"
export E2E_MODE="${E2E_MODE:-nightly}"
pyramid_export_llm_keys_if_needed

echo "==> L3 rag_quality_prod (external worker must be stopped)"
cargo test -p app --test product_e2e --features product-e2e llm_real::rag_quality_prod -- --ignored --test-threads=1 --nocapture \
  || pyramid_fail L3-full-quality S6 \
    "pkill avrag-worker if needed; E2E_MODE=nightly cargo test -p app --test product_e2e llm_real::rag_quality_prod -- --ignored --test-threads=1" \
    "quality release gate"

pyramid_ok L3-full-quality
echo "L3 quality OK"
