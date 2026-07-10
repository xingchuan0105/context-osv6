#!/usr/bin/env bash
# L1 — daily fast base (product拍板: 日常只跑这一层)
# Usage: bash scripts/test-l1.sh [extra cargo -p crates...]
#
# Resource defaults (WSL / ~16G RAM friendly):
#   CARGO_BUILD_JOBS — rustc parallel compile jobs (default 2)
#   L1_TEST_THREADS  — libtest threads per package (default 2; agent-loop always 1)
# Override examples:
#   CARGO_BUILD_JOBS=8 L1_TEST_THREADS=4 bash scripts/test-l1.sh
set -euo pipefail
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

# Cap compile + test parallelism before any cargo invocation.
export CARGO_BUILD_JOBS="${CARGO_BUILD_JOBS:-2}"
L1_TEST_THREADS="${L1_TEST_THREADS:-2}"

echo "==> L1 file-size gate"
bash scripts/check_file_size_limits.sh

cd avrag-rs
DEFAULT_PKGS=(agent-tools agent-loop app-chat)
if [[ $# -gt 0 ]]; then
  PKGS=("$@")
else
  PKGS=("${DEFAULT_PKGS[@]}")
fi

echo "==> L1 cargo (CARGO_BUILD_JOBS=${CARGO_BUILD_JOBS}) test --lib: ${PKGS[*]} (test-threads default=${L1_TEST_THREADS})"
for p in "${PKGS[@]}"; do
  threads="${L1_TEST_THREADS}"
  if [[ "$p" == "agent-loop" ]]; then
    threads=1
  fi
  echo "  -> cargo test -p ${p} --lib -- --test-threads=${threads}"
  cargo test -p "$p" --lib -- --test-threads="${threads}"
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
