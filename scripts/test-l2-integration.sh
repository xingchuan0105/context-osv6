#!/usr/bin/env bash
# L2 — full mock integration product_e2e (serial). Wave-end / weekly core.
set -euo pipefail
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT/avrag-rs"

export E2E_MODE="${E2E_MODE:-integration}"
echo "==> L2 integration E2E_MODE=$E2E_MODE"
cargo test -p app --test product_e2e --features product-e2e -- --test-threads=1

echo "L2 integration OK"
