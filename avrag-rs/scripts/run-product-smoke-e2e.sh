#!/usr/bin/env bash
# PR smoke E2E — single source of truth for module lists (see smoke/mod.rs).
set -euo pipefail

cd "$(dirname "$0")/.."

trap 'docker ps -aq --filter name=avrag-test- | xargs -r docker rm -f' EXIT

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

assert_smoke_module_coverage() {
  local -a registered=()
  local module discovered
  mapfile -t registered < <(printf '%s\n' "${NON_RAG_MODULES[@]}" "${RAG_SERIAL_MODULES[@]}" | sort -u)

  mapfile -t discovered < <(
    cargo test --test product_e2e -p app --features product-e2e smoke:: -- --list \
      | sed -n 's/^smoke::\([^:]*\)::.*/\1/p' \
      | sort -u
  )

  for module in "${discovered[@]}"; do
    if ! printf '%s\n' "${registered[@]}" | grep -qx "$module"; then
      echo "ERROR: smoke module '$module' is not listed in run-product-smoke-e2e.sh (NON_RAG_MODULES or RAG_SERIAL_MODULES)" >&2
      exit 1
    fi
  done

  for module in "${registered[@]}"; do
    if ! printf '%s\n' "${discovered[@]}" | grep -qx "$module"; then
      echo "ERROR: run-product-smoke-e2e.sh lists smoke module '$module' but cargo --list found no tests under smoke::${module}::" >&2
      exit 1
    fi
  done
}

assert_smoke_module_coverage

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
