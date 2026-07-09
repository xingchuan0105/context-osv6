#!/usr/bin/env bash
# L3 — real LLM thin sample (four agents). Not quality corpus.
# Requires env + network. Wave-end / manual.
set -euo pipefail
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT/avrag-rs"

export E2E_MODE="${E2E_MODE:-nightly}"
echo "==> L3 llm_real sample E2E_MODE=$E2E_MODE"
# Filter to llm_real module; still ignored tests need --ignored
cargo test -p app --test product_e2e --features product-e2e llm_real -- --ignored --test-threads=1 --nocapture

echo "L3 llm sample finished (check failures above)"
