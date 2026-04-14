#!/usr/bin/env bash
set -euo pipefail

STRICT_CITATIONS=false
if [[ "${1:-}" == "--strict-citations" ]]; then
  STRICT_CITATIONS=true
fi

missing=()

require_var() {
  local key="$1"
  if [[ -z "${!key:-}" ]]; then
    missing+=("${key}")
  fi
}

require_qdrant() {
  if [[ -n "${QDRANT_URL:-}" ]]; then
    return 0
  fi
  if [[ -n "${QDRANT_HOST:-}" && -n "${QDRANT_PORT:-}" ]]; then
    return 0
  fi
  missing+=("QDRANT_URL (or QDRANT_HOST + QDRANT_PORT)")
}

echo "[preflight] checking baseline E2E env..."
require_var "DATABASE_URL"
require_qdrant

if [[ "${STRICT_CITATIONS}" == "true" ]]; then
  echo "[preflight] strict citation mode enabled."
  require_var "EMBEDDING_API_KEY"
  require_var "ANSWER_LLM_API_KEY"
fi

if [[ "${#missing[@]}" -gt 0 ]]; then
  echo "[preflight] missing required variables:"
  printf '  - %s\n' "${missing[@]}"
  exit 1
fi

echo "[preflight] environment checks passed."
