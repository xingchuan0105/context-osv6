#!/usr/bin/env bash
# L2 — mechanisms (mock product smoke + loop/tools). Not daily default.
set -euo pipefail
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT/avrag-rs"

echo "==> L2 agent-tools + agent-loop + storage-pg lib"
cargo test -p agent-tools --lib -- --test-threads=4
cargo test -p agent-loop --lib -- --test-threads=4
cargo test -p avrag-storage-pg --lib -- --test-threads=2

if [[ -x "$ROOT/avrag-rs/scripts/run-product-smoke-e2e.sh" ]]; then
  echo "==> L2 product smoke mock (existing runner)"
  bash "$ROOT/avrag-rs/scripts/run-product-smoke-e2e.sh"
elif [[ -x "$ROOT/scripts/run-product-smoke-e2e.sh" ]]; then
  bash "$ROOT/scripts/run-product-smoke-e2e.sh"
else
  echo "WARN: product smoke runner not found; lib portion only"
fi

echo "L2 mechanisms OK"
