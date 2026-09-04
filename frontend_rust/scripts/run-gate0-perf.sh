#!/bin/bash
# Gate 0 Playwright collector. Requires target/debug/web-server + target/site.
# Usage:
#   GATE0=1 bash scripts/run-gate0-perf.sh
#   GATE0=1 GATE0_NEXT_BASE=http://127.0.0.1:3000 GATE0_API_BASE=http://127.0.0.1:18081 \
#     bash scripts/run-gate0-perf.sh
#   不要把 GATE0_NEXT_BASE 指到 :8080（那是 Plane，不是 Next）。
set -euo pipefail
export LANG=C.UTF-8
export PATH="$HOME/.local/opt/wasm-bindgen-cli-0.2.127:$HOME/.cargo/bin:$PATH"
export GATE0="${GATE0:-1}"
cd "$(dirname "$0")/../tests/browser"
pnpm exec playwright test --config playwright.gate0.config.ts
