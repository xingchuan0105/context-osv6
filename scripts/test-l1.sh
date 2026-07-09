#!/usr/bin/env bash
# L1 — daily fast base (product拍板: 日常只跑这一层)
# Usage: bash scripts/test-l1.sh [extra cargo -p crates...]
set -euo pipefail
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

echo "==> L1 file-size gate"
bash scripts/check_file_size_limits.sh

cd avrag-rs
DEFAULT_PKGS=(agent-tools agent-loop app-chat)
if [[ $# -gt 0 ]]; then
  PKGS=("$@")
else
  PKGS=("${DEFAULT_PKGS[@]}")
fi

echo "==> L1 cargo test --lib: ${PKGS[*]}"
for p in "${PKGS[@]}"; do
  if [[ "$p" == "agent-loop" ]]; then cargo test -p "$p" --lib -- --test-threads=1; else cargo test -p "$p" --lib -- --test-threads=4; fi
done

if [[ -d "$ROOT/frontend_next" ]]; then
  echo "==> L1 frontend tsc (if pnpm available)"
  if command -v pnpm >/dev/null 2>&1; then
    pnpm -C "$ROOT/frontend_next" exec tsc --noEmit
  else
    echo "skip tsc: pnpm not found"
  fi
fi

echo "L1 OK"
