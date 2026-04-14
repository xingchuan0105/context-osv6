#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "${ROOT_DIR}"

files=(
  avrag-rs/crates/common/src/lib.rs
  avrag-rs/crates/billing/src/lib.rs
  avrag-rs/crates/billing/src/core.rs
  avrag-rs/crates/share/src/lib.rs
  avrag-rs/crates/storage-qdrant/src/lib.rs
  avrag-rs/crates/search/src/lib.rs
  avrag-rs/crates/usage-limit/src/lib.rs
  avrag-rs/crates/admin/src/lib.rs
  avrag-rs/crates/ingestion/src/lib.rs
  avrag-rs/crates/llm/src/synthesizer.rs
  avrag-rs/crates/llm/src/summary.rs
  avrag-rs/crates/app/src/chat/service.rs
  avrag-rs/crates/app/src/chat/graphflow.rs
  avrag-rs/crates/app/src/lib_impl.rs
  avrag-rs/crates/transport-http/src/lib_impl.rs
  avrag-rs/crates/storage-pg/src/lib_impl.rs
  frontend_rust/crates/web-sdk/src/lib_impl.rs
  frontend_rust/crates/web-ui/src/routes/dashboard.rs
  frontend_rust/crates/web-ui/src/routes/auth.rs
  frontend_rust/crates/web-ui/src/routes/shared.rs
  frontend_rust/crates/web-ui/src/routes/shared/shared_kb_page.rs
  frontend_rust/crates/web-ui/src/routes/admin.rs
  frontend_rust/crates/web-ui/src/routes/admin/feature_flags_page.rs
  frontend_rust/crates/web-ui/src/routes/settings.rs
  frontend_rust/crates/web-ui/src/routes/dashboard/dashboard_list_page.rs
  frontend_rust/crates/web-ui/src/routes/dashboard/workspace_page.rs
  frontend_rust/crates/web-ui/src/routes/dashboard/workspace/page.rs
  frontend_rust/crates/web-ui/src/routes/dashboard/workspace_setup.rs
)

for path in "${files[@]}"; do
  lines="$(wc -l < "${path}")"
  if [ "${lines}" -gt 500 ]; then
    echo "file exceeds hard size limit: ${path} (${lines} lines)"
    exit 1
  fi
done
