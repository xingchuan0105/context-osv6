#!/usr/bin/env bash
# L1 — daily fast base (product拍板: 日常只跑这一层)
# Usage: bash scripts/test-l1.sh [cargo -p crate names...]
#
# Default packages (when no args): agent-tools, agent-loop, app-chat
# When changing transport-http / storage-pg, pass them as args, e.g.:
#   bash scripts/test-l1.sh transport-http
#   bash scripts/test-l1.sh agent-tools agent-loop app-chat transport-http storage-pg
#
# Layer semantics / acceptance gates: avrag-rs/docs/e2e-gates.md
# Coverage remediation (Write/guardrails closeout): docs/engineering/E2E_COVERAGE_REMEDIATION_PLAN_2026-07-10.md
#
# Resource defaults (WSL / ~16G RAM friendly):
#   CARGO_BUILD_JOBS — rustc parallel compile jobs (default 2)
#   L1_TEST_THREADS  — libtest threads per package (default 2; agent-loop always 1)
# Override: CARGO_BUILD_JOBS=8 L1_TEST_THREADS=4 bash scripts/test-l1.sh
set -euo pipefail
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
# shellcheck source=pyramid-lib.sh
source "${ROOT}/scripts/pyramid-lib.sh"
cd "$ROOT"

export CARGO_BUILD_JOBS="${CARGO_BUILD_JOBS:-2}"
L1_TEST_THREADS="${L1_TEST_THREADS:-2}"

echo "[PYRAMID] layer=L1 begin"
echo "==> L1 file-size gate"
if [[ -f scripts/check_file_size_limits.sh ]]; then
  bash scripts/check_file_size_limits.sh \
    || pyramid_fail L1 S0 "bash scripts/check_file_size_limits.sh" "file-size gate"
else
  echo "skip file-size gate (script not found)"
fi

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
  cargo test -p "$p" --lib -- --test-threads="${threads}" \
    || pyramid_fail L1 S0/S1 "cargo test -p ${p} --lib -- --test-threads=${threads}" "crate=${p}"
done

if [[ -d "$ROOT/frontend_next" ]]; then
  echo "==> L1 frontend tsc (if pnpm available)"
  if command -v pnpm >/dev/null 2>&1; then
    pnpm -C "$ROOT/frontend_next" exec tsc --noEmit \
      || pyramid_fail L1 S0 "pnpm -C frontend_next exec tsc --noEmit" "frontend types"
  else
    echo "skip tsc: pnpm not found"
  fi
fi

pyramid_ok L1
echo "L1 OK"
