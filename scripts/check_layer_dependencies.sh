#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "${ROOT_DIR}"

if rg -n 'storage_pg|redis::|qdrant' avrag-rs/crates/transport-http/src; then
  echo "transport-http depends on infra implementation"
  exit 1
fi

if rg -n 'transport_http' avrag-rs/crates/app/src; then
  echo "app depends on transport-http"
  exit 1
fi
