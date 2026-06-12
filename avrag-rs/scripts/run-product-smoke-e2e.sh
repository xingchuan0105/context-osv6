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

RAG_SERIAL_MODULES=(
  ingestion_smoke
  rag_smoke
  rag_fallback_smoke
  rag_codegen_multitool_smoke
  memory_multiturn_smoke
  paddle_pdf_smoke
)

export E2E_MODE=smoke

echo "== Non-RAG smoke (parallel) =="
for t in "${NON_RAG_MODULES[@]}"; do
  cargo test --test product_e2e -p app "smoke::${t}" -- --test-threads=1 --nocapture &
done
cargo test --test product_e2e -p app product_e2e:: -- --test-threads=1 --nocapture &
wait

echo "== RAG smoke (serial) =="
for t in "${RAG_SERIAL_MODULES[@]}"; do
  cargo test --test product_e2e -p app "smoke::${t}" -- --test-threads=1 --nocapture
done
