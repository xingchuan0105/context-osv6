#!/usr/bin/env bash
# File size gate for current monorepo hotspots (TN remediation Wave 0).
# Hard limit: 1000 lines (production sources listed below).
# Soft warn:  800 lines — printed but non-fatal.
#
# Missing paths fail the check so the allowlist cannot rot silently.
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "${ROOT_DIR}"

HARD_LIMIT=1000
SOFT_LIMIT=800

# Production hotspots only (no frontend_rust in this gate's required set).
# Keep this list short and real; update when modules move.
files=(
  avrag-rs/bins/worker/src/pipeline/document_pipeline/mod.rs
  avrag-rs/crates/rag-core/src/runtime/execute.rs
  avrag-rs/crates/llm/src/embedding.rs
  avrag-rs/crates/app-chat/src/token_budget/simulate.rs
  # Wave 6 relocates (was app-chat/agents/*)
  avrag-rs/crates/agent-loop/src/react_loop/answer_contract.rs
  avrag-rs/crates/agent-tools/src/capability/registry.rs
  avrag-rs/crates/agent-tools/src/skills/registry.rs
  avrag-rs/crates/agent-tools/src/catalog.rs
  avrag-rs/crates/app-bootstrap/src/app_state/e2e_upload_helpers.rs
  avrag-rs/crates/app-bootstrap/src/app_state/state_methods.rs
  avrag-rs/crates/app-bootstrap/src/app_state/bound/mod.rs
  frontend_next/lib/workspace/client.ts
  frontend_next/components/admin/i18n/copy.ts
  frontend_next/components/admin/ops/feature-flags-surface.tsx
  contracts/src/rag_execute.rs
  contracts/src/chat.rs
  contracts/src/tool_call.rs
)

hard_failures=0
soft_warnings=0
missing=0

for path in "${files[@]}"; do
  if [ ! -f "${path}" ]; then
    echo "file size gate: missing path (update allowlist): ${path}"
    missing=$((missing + 1))
    continue
  fi
  lines="$(wc -l < "${path}" | tr -d ' ')"
  if [ "${lines}" -gt "${HARD_LIMIT}" ]; then
    echo "HARD: file exceeds ${HARD_LIMIT} lines: ${path} (${lines} lines) — decompose before growing further"
    hard_failures=$((hard_failures + 1))
  elif [ "${lines}" -gt "${SOFT_LIMIT}" ]; then
    echo "WARN: file exceeds soft limit ${SOFT_LIMIT} lines: ${path} (${lines} lines)"
    soft_warnings=$((soft_warnings + 1))
  fi
done

# Aggregate app_state (delegate wall) — report total, hard-fail if total > 2000
app_state_dir="avrag-rs/crates/app-bootstrap/src/app_state"
if [ -d "${app_state_dir}" ]; then
  app_state_total="$(find "${app_state_dir}" -name '*.rs' -print0 | xargs -0 cat | wc -l | tr -d ' ')"
  if [ "${app_state_total}" -gt 2000 ]; then
    echo "HARD: app_state aggregate exceeds 2000 lines: ${app_state_dir} (${app_state_total} lines)"
    hard_failures=$((hard_failures + 1))
  elif [ "${app_state_total}" -gt 1200 ]; then
    echo "WARN: app_state aggregate large: ${app_state_dir} (${app_state_total} lines) — Wave 3 target is <30 business methods / shrink delegates"
    soft_warnings=$((soft_warnings + 1))
  fi
fi

echo "file size gate: soft_warnings=${soft_warnings} hard_failures=${hard_failures} missing=${missing}"

if [ "${missing}" -gt 0 ] || [ "${hard_failures}" -gt 0 ]; then
  exit 1
fi

exit 0
