#!/bin/bash
# Gate 0 Playwright collector.
# Usage:
#   GATE0=1 bash scripts/run-gate0-perf.sh
#   GATE0=1 GATE0_RUST_PROFILE=release bash scripts/run-gate0-perf.sh
#   GATE0=1 GATE0_NEXT_BASE=http://127.0.0.1:3000 GATE0_API_BASE=http://127.0.0.1:18081 \
#     bash scripts/run-gate0-perf.sh
#   GATE0_STRESS=1 GATE0_RUST_PROFILE=release bash scripts/run-gate0-perf.sh
#   不要把 GATE0_NEXT_BASE 指到 :8080（那是 Plane，不是 Next）。
# debug 需要 target/debug/web-server；release 需要先 cargo leptos build --release。
# GATE0_STRESS=1 只跑 30 分钟堆斜率（不要同时开 GATE0=1，否则会先跑 5+20）。
set -euo pipefail
export LANG=C.UTF-8
export PATH="$HOME/.local/opt/wasm-bindgen-cli-0.2.127:$HOME/.cargo/bin:$PATH"
export GATE0_STRESS="${GATE0_STRESS:-}"
if [ "${GATE0_STRESS}" = "1" ]; then
  export GATE0="${GATE0:-}"
else
  export GATE0="${GATE0:-1}"
fi
export GATE0_RUST_PROFILE="${GATE0_RUST_PROFILE:-debug}"
cd "$(dirname "$0")/../tests/browser"
pnpm exec playwright test --config playwright.gate0.config.ts
