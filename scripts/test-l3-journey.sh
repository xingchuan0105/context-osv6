#!/usr/bin/env bash
# L3 — real UI. Default: Playwright smoke (short). JOURNEY=1 for full journey.
# Product: 短旅程波次末；长旅程发版/夜间. DR2 L3-thin.
set -euo pipefail
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
# shellcheck source=pyramid-lib.sh
source "${ROOT}/scripts/pyramid-lib.sh"
cd "$ROOT/frontend_next"

echo "[PYRAMID] layer=L3-thin-journey begin"

# Local: reuse already-running avrag-api/worker (8080/8081) to avoid webServer port fights.
# CI always starts fresh servers (PLAYWRIGHT_REUSE_SERVER unset/false there).
if [[ "${CI:-}" != "true" && "${CI:-}" != "1" ]]; then
  export PLAYWRIGHT_REUSE_SERVER="${PLAYWRIGHT_REUSE_SERVER:-1}"
fi

if [[ "${JOURNEY:-0}" == "1" ]]; then
  echo "==> L3 Playwright journey (PLAYWRIGHT_REUSE_SERVER=${PLAYWRIGHT_REUSE_SERVER:-0})"
  pnpm exec playwright test --project=functional e2e/specs/journey \
    || pyramid_fail L3-thin-journey S3 \
      "cd frontend_next && JOURNEY=1 pnpm exec playwright test --project=functional e2e/specs/journey" \
      "full journey"
else
  echo "==> L3 Playwright smoke (short) (PLAYWRIGHT_REUSE_SERVER=${PLAYWRIGHT_REUSE_SERVER:-0})"
  if ! pnpm exec playwright test --project=functional e2e/specs/smoke 2>/dev/null; then
    pnpm exec playwright test e2e/specs/smoke \
      || pyramid_fail L3-thin-journey S3 \
        "cd frontend_next && pnpm exec playwright test e2e/specs/smoke" \
        "playwright smoke"
  fi
fi

pyramid_ok L3-thin-journey
echo "L3 journey OK"
