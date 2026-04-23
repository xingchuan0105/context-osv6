#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT_DIR"

if ! command -v cargo-leptos >/dev/null 2>&1; then
  echo "cargo-leptos 未安装。先执行: cargo install cargo-leptos --locked" >&2
  exit 1
fi

mkdir -p target/site/pkg
python3 scripts/watch_design_tokens.py &
TOKENS_WATCH_PID=$!

STYLANCE_PID=""
TAILWIND_PID=""

cargo leptos watch "$@" &
LEPTOS_PID=$!

for _ in $(seq 1 90); do
  if curl -sf http://127.0.0.1:3000/pkg/web_ui.js >/dev/null 2>&1; then
    break
  fi
  sleep 1
done

stylance crates/web-ui \
  --folder src \
  --output-file ./target/site/pkg/stylance.css

stylance crates/web-ui \
  --folder src \
  --output-file ./target/site/pkg/stylance.css \
  --watch &
STYLANCE_PID=$!

npx tailwindcss \
  -c tailwind.config.js \
  -i ./crates/web-ui/src/index.css \
  -o ./target/site/pkg/index.css </dev/null

npx tailwindcss \
  -c tailwind.config.js \
  -i ./crates/web-ui/src/index.css \
  -o ./target/site/pkg/index.css \
  --watch=always </dev/null &
TAILWIND_PID=$!

cleanup() {
  kill "$TOKENS_WATCH_PID" "$LEPTOS_PID" 2>/dev/null || true
  [[ -n "$STYLANCE_PID" ]] && kill "$STYLANCE_PID" 2>/dev/null || true
  [[ -n "$TAILWIND_PID" ]] && kill "$TAILWIND_PID" 2>/dev/null || true
}
trap cleanup EXIT INT TERM

wait "$LEPTOS_PID"
