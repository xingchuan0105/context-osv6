#!/bin/bash
# Gate 0 Playwright collector. Requires target/debug/web-server + target/site.
# Usage: GATE0=1 [GATE0_NEXT_BASE=http://127.0.0.1:8080] bash scripts/run-gate0-perf.sh
set -euo pipefail
export LANG=C.UTF-8
export PATH="$HOME/.local/opt/wasm-bindgen-cli-0.2.127:$HOME/.cargo/bin:$PATH"
export GATE0="${GATE0:-1}"
cd "$(dirname "$0")/../tests/browser"
pnpm exec playwright test --config playwright.gate0.config.ts
