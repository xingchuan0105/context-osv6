#!/usr/bin/env bash
# Goal D local gate: Rust mock x2 + optional embedding_cache + llm_real + Playwright.
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT/avrag-rs"

echo "== Phase 0/1: Rust mock full suite (round 1) =="
cargo test --test product_e2e -p app -- --test-threads=1

echo "== Phase 0/1: Rust mock full suite (round 2) =="
cargo test --test product_e2e -p app -- --test-threads=1

echo "== Phase 3: Embedding cache (ignored, requires Redis docker) =="
cargo test -p app --test product_e2e integration::embedding_cache -- --ignored --test-threads=1 || {
  echo "WARN: embedding_cache skipped or failed (Redis docker required)"
}

echo "== Phase 6: llm_real (ignored) =="
cargo test -p app --test product_e2e llm_real -- --ignored --test-threads=1 --nocapture || {
  echo "WARN: llm_real failed or credentials missing"
}

echo "== Phase 5: Playwright =="
cd "$ROOT/frontend_next"
npx playwright test --project=auth --project=functional --project=journey --project=skills

echo "Goal D gate complete."
