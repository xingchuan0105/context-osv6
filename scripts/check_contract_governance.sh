#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "${ROOT_DIR}"

if rg -n 'pub struct (ChatRequest|ChatResponse|ChatEvent|WorkspaceResponse|DocumentStatusResponse|AuthEnvelope|UsageLimitResponse)' frontend_rust/crates/web-sdk/src avrag-rs/crates/transport-http/src avrag-rs/crates/app/src; then
  echo "manual transport DTO definition found outside contracts crate"
  exit 1
fi

if awk '
  BEGIN {
    in_dtos = 0
    depth = 0
    found = 0
  }
  /pub mod dtos[[:space:]]*\{/ {
    in_dtos = 1
    depth = 1
    next
  }
  in_dtos {
    if ($0 ~ /pub struct[[:space:]]+[A-Za-z_][A-Za-z0-9_]*[[:space:]]*\{/) {
      found = 1
      exit 0
    }
    opens = gsub(/\{/, "{")
    closes = gsub(/\}/, "}")
    depth += opens - closes
    if (depth <= 0) {
      in_dtos = 0
    }
  }
  END {
    exit(found ? 0 : 1)
  }
' frontend_rust/crates/web-sdk/src/lib_impl.rs; then
  echo "manual transport DTO definition found outside contracts crate"
  exit 1
fi

if rg -n '"crates/web-sdk"|"crates/web-ui"' avrag-rs/Cargo.toml; then
  echo "archived frontend crates still present in avrag-rs workspace"
  exit 1
fi
