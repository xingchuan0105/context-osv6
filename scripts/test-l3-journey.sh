#!/usr/bin/env bash
# L3-journey — Playwright long UI stories (upload→RAG, chat, write, share).
# Not DR2 default. Wave-end / DR3 / explicit JOURNEY=1.
#
# Standard document for upload→RAG: frontend_next/e2e/fixtures/antifragile.txt
# (same bytes as product_e2e standard doc — see fixtures/standard_doc.rs).
set -euo pipefail
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
# shellcheck source=pyramid-lib.sh
source "${ROOT}/scripts/pyramid-lib.sh"
cd "$ROOT/frontend_next"

echo "[PYRAMID] layer=L3-journey begin"

if [[ "${CI:-}" != "true" && "${CI:-}" != "1" ]]; then
  export PLAYWRIGHT_REUSE_SERVER="${PLAYWRIGHT_REUSE_SERVER:-1}"
fi

# Backward compat: old callers expected smoke when JOURNEY unset.
# New default for this script is journey; DR2 uses test-l3-ui-smoke.sh.
if [[ "${JOURNEY:-1}" == "0" || "${L3_UI_MODE:-}" == "smoke" ]]; then
  echo "[PYRAMID] delegating to test-l3-ui-smoke.sh (JOURNEY=0 or L3_UI_MODE=smoke)"
  exec bash "${ROOT}/scripts/test-l3-ui-smoke.sh"
fi

echo "==> L3 Playwright journey (PLAYWRIGHT_REUSE_SERVER=${PLAYWRIGHT_REUSE_SERVER:-0})"
pnpm exec playwright test --project=journey e2e/specs/journey \
  || pyramid_fail L3-journey S3 \
    "cd frontend_next && pnpm exec playwright test --project=journey e2e/specs/journey" \
    "upload-rag uses antifragile.txt standard doc"

pyramid_ok L3-journey
echo "L3 journey OK"
