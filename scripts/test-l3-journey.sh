#!/usr/bin/env bash
# L3 — real UI. Default: Playwright smoke (short). JOURNEY=1 for full journey.
# Product: 短旅程波次末；长旅程发版/夜间.
set -euo pipefail
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT/frontend_next"

if [[ "${JOURNEY:-0}" == "1" ]]; then
  echo "==> L3 Playwright journey"
  pnpm exec playwright test --project=functional e2e/specs/journey
else
  echo "==> L3 Playwright smoke (short)"
  pnpm exec playwright test --project=functional e2e/specs/smoke || \
    pnpm exec playwright test e2e/specs/smoke
fi

echo "L3 journey OK"
