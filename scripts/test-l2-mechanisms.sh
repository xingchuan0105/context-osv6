#!/usr/bin/env bash
# L2 — mechanisms (mock product smoke + loop/tools). Not daily default.
# DR1 / DR2 prerequisite. Plan: ACCEPTANCE_PYRAMID_STABILIZATION_PLAN.
set -euo pipefail
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
# shellcheck source=pyramid-lib.sh
source "${ROOT}/scripts/pyramid-lib.sh"
cd "$ROOT/avrag-rs"

echo "[PYRAMID] layer=L2-core begin"

echo "==> L2 agent-tools + agent-loop + storage-pg lib"
cargo test -p agent-tools --lib -- --test-threads=4 \
  || pyramid_fail L2-core S1 "cargo test -p agent-tools --lib" "agent-tools"
cargo test -p agent-loop --lib -- --test-threads=1 \
  || pyramid_fail L2-core S1 "cargo test -p agent-loop --lib -- --test-threads=1" "agent-loop"
cargo test -p avrag-storage-pg --lib -- --test-threads=2 \
  || pyramid_fail L2-core S1 "cargo test -p avrag-storage-pg --lib" "storage-pg"

if [[ -x "$ROOT/avrag-rs/scripts/run-product-smoke-e2e.sh" ]]; then
  echo "==> L2 product smoke mock (existing runner)"
  bash "$ROOT/avrag-rs/scripts/run-product-smoke-e2e.sh" \
    || pyramid_fail L2-core S2 \
      "bash avrag-rs/scripts/run-product-smoke-e2e.sh  # or single smoke::MODULE" \
      "product smoke mock"
elif [[ -x "$ROOT/scripts/run-product-smoke-e2e.sh" ]]; then
  bash "$ROOT/scripts/run-product-smoke-e2e.sh" \
    || pyramid_fail L2-core S2 "bash scripts/run-product-smoke-e2e.sh" "product smoke mock"
else
  echo "WARN: product smoke runner not found; lib portion only"
fi

pyramid_ok L2-core
echo "L2 mechanisms OK"
