#!/usr/bin/env bash
# L2-patho — pathological / SLA / terminal-integrity / authz mechanism tests.
# Not daily default. Required for DR2 (准部署).
#
# Budget: ≤ 15 min. Filters `patho_` across P0 CAP crates.
# Plan: docs/engineering/ACCEPTANCE_PYRAMID_STABILIZATION_PLAN_2026-07-10.md
set -euo pipefail
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
# shellcheck source=pyramid-lib.sh
source "${ROOT}/scripts/pyramid-lib.sh"
cd "$ROOT/avrag-rs"

export CARGO_BUILD_JOBS="${CARGO_BUILD_JOBS:-2}"
THREADS="${L2_PATHO_TEST_THREADS:-2}"

run_patho() {
  local pkg="$1"
  local cap="$2"
  shift 2
  echo "[PYRAMID] layer=L2-patho cap=${cap} pkg=${pkg}"
  cargo test -p "$pkg" "$@" patho_ -- --test-threads="${THREADS}" \
    || pyramid_fail L2-patho S4 \
      "cargo test -p ${pkg} $* patho_ -- --test-threads=${THREADS}" \
      "cap=${cap}"
}

echo "======== L2-patho (P0 CAP matrix) ========"
echo "[PYRAMID] layer=L2-patho begin"

run_patho ingestion CAP-INGEST --lib
run_patho write-core CAP-WRITE --lib
run_patho transport-http "CAP-AUTH+CAP-STREAM" --lib
run_patho agent-loop CAP-CHAT --lib
run_patho app-chat CAP-WRITE --lib
run_patho avrag-rag-core CAP-RAG --test graph_tenant_isolation

pyramid_ok L2-patho
echo "L2 patho OK"
