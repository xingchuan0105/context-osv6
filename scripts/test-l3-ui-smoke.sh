#!/usr/bin/env bash
# L3-thin-ui — Playwright smoke (auth / legal / navigation).
# DR2 default UI layer. Does NOT run journey upload-RAG (see test-l3-journey.sh).
#
# Requires: local Next + avrag-api (PLAYWRIGHT_REUSE_SERVER=1 by default).
# API must match DB schema (rebuild after org-removal migrations).
set -euo pipefail
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
# shellcheck source=pyramid-lib.sh
source "${ROOT}/scripts/pyramid-lib.sh"
cd "$ROOT/frontend_next"

echo "[PYRAMID] layer=L3-thin-ui begin"

if [[ "${CI:-}" != "true" && "${CI:-}" != "1" ]]; then
  export PLAYWRIGHT_REUSE_SERVER="${PLAYWRIGHT_REUSE_SERVER:-1}"
fi

# Fail fast on stale API / schema skew before Playwright globalSetup noise.
if [[ "${SKIP_DEV_STACK_CHECK:-0}" != "1" ]]; then
  echo "==> dev-stack-check (login probe)"
  bash "${ROOT}/scripts/dev-stack-check.sh" \
    || pyramid_fail L3-thin-ui S2 \
      "bash scripts/dev-stack-check.sh  # or rebuild avrag-api" \
      "login 5xx usually means API binary older than org-removal migrations"
fi

echo "==> L3 Playwright smoke (short) (PLAYWRIGHT_REUSE_SERVER=${PLAYWRIGHT_REUSE_SERVER:-0})"
if ! pnpm exec playwright test --project=functional e2e/specs/smoke 2>/dev/null; then
  pnpm exec playwright test e2e/specs/smoke \
    || pyramid_fail L3-thin-ui S3 \
      "cd frontend_next && pnpm exec playwright test e2e/specs/smoke" \
      "rebuild avrag-api if login 500 org_id; check E2E_RESET_SECRET"
fi

pyramid_ok L3-thin-ui
echo "L3 UI smoke OK"
