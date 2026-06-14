#!/usr/bin/env bash
# Promote the newest llm_real run as persistent baseline for regression diff.
set -euo pipefail

cd "$(dirname "$0")/.."

OUTPUT_ROOT="${1:-crates/app/tests/e2e_output}"
LIMIT="${2:-1}"

latest="$(
  cargo run -p e2e-analyzer --quiet -- llm-real list --output "$OUTPUT_ROOT" --limit "$LIMIT" \
    | tail -n "$LIMIT" \
    | head -n 1 \
    | awk '{print $1}'
)"

if [[ -z "$latest" ]]; then
  echo "ERROR: no llm_real runs under ${OUTPUT_ROOT}/llm_real" >&2
  exit 1
fi

echo "Promoting baseline: $latest"
cargo run -p e2e-analyzer -- baseline --run "$latest"
