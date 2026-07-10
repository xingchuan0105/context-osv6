#!/usr/bin/env bash
# L3 — real LLM thin sample (four agents). Not quality corpus.
# Requires env + network. Wave-end / manual / DR2 L3-thin.
set -euo pipefail
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
# shellcheck source=pyramid-lib.sh
source "${ROOT}/scripts/pyramid-lib.sh"
cd "$ROOT/avrag-rs"

echo "[PYRAMID] layer=L3-thin-llm begin"
export E2E_MODE="${E2E_MODE:-nightly}"
# Export only LLM key names if unset — never source full .env (pyramid-lib local Solo).
pyramid_export_llm_keys_if_needed
echo "==> L3 llm_real sample E2E_MODE=$E2E_MODE"
# Filter to llm_real module; still ignored tests need --ignored
cargo test -p app --test product_e2e --features product-e2e llm_real -- --ignored --test-threads=1 --nocapture \
  || pyramid_fail L3-thin-llm S5 \
    "E2E_MODE=nightly cargo test -p app --test product_e2e --features product-e2e llm_real -- --ignored --test-threads=1" \
    "check AGENT_LLM_API_KEY / network"

pyramid_ok L3-thin-llm
echo "L3 llm sample OK"
