#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT_DIR"

if ! command -v stylance >/dev/null 2>&1; then
  echo "stylance-cli 未安装。先执行: cargo install stylance-cli --locked" >&2
  exit 1
fi

build_output() {
  local output_file="$1"
  mkdir -p "$(dirname "$output_file")"
  stylance crates/web-ui --folder src --output-file "$output_file"
}

build_output "$ROOT_DIR/target/site/pkg/stylance.css"
build_output "$ROOT_DIR/pkg/stylance.css"
