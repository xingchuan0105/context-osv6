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
  notebook_crud
  billing_boundary
)

# Registered for module-guard coverage only; not executed in PR smoke (#[ignore] / staging).
SMOKE_MANUAL_ONLY_MODULES=(
  search_real_smoke
  paddle_pdf_smoke
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
  paddle_image_smoke
)

is_manual_only_smoke_module() {
  local module="$1"
  local m
  for m in "${SMOKE_MANUAL_ONLY_MODULES[@]}"; do
    if [[ "$m" == "$module" ]]; then
      return 0
    fi
  done
  return 1
}

assert_smoke_module_coverage() {
  local -a registered=()
  local module discovered
  mapfile -t registered < <(printf '%s\n' "${NON_RAG_MODULES[@]}" "${RAG_SERIAL_MODULES[@]}" "${SMOKE_MANUAL_ONLY_MODULES[@]}" | sort -u)

  mapfile -t discovered < <(
    cargo test --test product_e2e -p app --features product-e2e smoke:: -- --list \
      | sed -n 's/.*::smoke::\([^:]*\)::.*/\1/p' \
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

  echo "OK: smoke module coverage guard passed (${#registered[@]} modules match cargo --list)"
}

if [[ "${1:-}" == "--check-modules" ]]; then
  assert_smoke_module_coverage
  exit 0
fi

assert_smoke_module_coverage

export E2E_MODE=smoke

echo "== Pre-build shared artifacts (avoid parallel cargo lock contention) =="
cargo build -p avrag-worker -p app --features product-e2e --tests

echo "== Paddle image routing + worker metadata contracts =="
cargo test -p ingestion image_file_routing_uses_paddle_ocr_image_route --quiet
cargo test -p avrag-worker paddle_image_route_metadata_contract --quiet

echo "== Non-RAG smoke + unit tests (parallel) =="
for t in "${NON_RAG_MODULES[@]}"; do
  if is_manual_only_smoke_module "$t"; then
    continue
  fi
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
