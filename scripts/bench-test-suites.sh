#!/usr/bin/env bash
# Measure wall-clock for pyramid entries (P5). Writes markdown fragment to stdout.
set -euo pipefail
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

run_timed() {
  local name="$1"
  shift
  local start end elapsed
  start=$(date +%s)
  set +e
  "$@"
  local rc=$?
  set -e
  end=$(date +%s)
  elapsed=$((end - start))
  printf '| %s | %ss | rc=%s |\n' "$name" "$elapsed" "$rc"
  return 0
}

echo "## Bench $(date -Iseconds) host=$(hostname)"
echo
echo '| Suite | Wall-clock | Result |'
echo '|-------|------------|--------|'

run_timed "L1 file-size gate" bash scripts/check_file_size_limits.sh
run_timed "L1 agent-tools --lib" bash -c "cd avrag-rs && cargo test -p agent-tools --lib -- --test-threads=4 -q"
run_timed "L1 agent-loop --lib" bash -c "cd avrag-rs && cargo test -p agent-loop --lib -- --test-threads=4 -q"
run_timed "L1 app-chat --lib" bash -c "cd avrag-rs && cargo test -p app-chat --lib -- --test-threads=4 -q"
run_timed "L1 transport-http --lib" bash -c "cd avrag-rs && cargo test -p transport-http --lib -- --test-threads=4 -q"
run_timed "L1 storage-pg --lib" bash -c "cd avrag-rs && cargo test -p avrag-storage-pg --lib -- --test-threads=2 -q"
run_timed "L1 frontend tsc" bash -c "cd frontend_next && pnpm exec tsc --noEmit"

echo
echo "Note: L2 product smoke / L3 journey / L3 llm_real not auto-run here (Docker/API cost)."
echo "Run manually and paste rows into docs/engineering/TEST_PYRAMID_INVENTORY_2026-07-09.md"
