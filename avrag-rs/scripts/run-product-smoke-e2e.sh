#!/usr/bin/env bash
# PR smoke E2E — single source of truth for module lists (see smoke/mod.rs).
set -euo pipefail

cd "$(dirname "$0")/.."

NON_RAG_MODULES=(
  chat_smoke
  search_smoke
  auth_boundary
  share_boundary
)

# PR smoke unit tests (parallel with non-RAG smoke; no Docker).
UNIT_TEST_FILTERS=(
  setup::tests
  e2e_gate::tests
  test_context::tests
  mock_routing
)

RAG_SERIAL_MODULES=(
  ingestion_smoke
  rag_smoke
  rag_fallback_smoke
  rag_codegen_multitool_smoke
  memory_multiturn_smoke
  paddle_pdf_smoke
)

export E2E_MODE=smoke

echo "== Non-RAG smoke + unit tests (parallel) =="
for t in "${NON_RAG_MODULES[@]}"; do
  cargo test --test product_e2e -p app --features product-e2e "smoke::${t}" -- --test-threads=1 --nocapture &
done
for f in "${UNIT_TEST_FILTERS[@]}"; do
  cargo test --test product_e2e -p app --features product-e2e "$f" -- --test-threads=1 --nocapture &
done
wait

echo "== RAG smoke (serial) =="
for t in "${RAG_SERIAL_MODULES[@]}"; do
  cargo test --test product_e2e -p app --features product-e2e "smoke::${t}" -- --test-threads=1 --nocapture
done
