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

echo "[preflight] checking baseline E2E env..."
require_var "DATABASE_URL"
require_var "MILVUS_URL"

if [[ "${STRICT_CITATIONS}" == "true" ]]; then
  echo "[preflight] strict citation mode enabled."
  require_var "EMBEDDING_API_KEY"
  require_var "AGENT_LLM_API_KEY"
fi

if [[ "${#missing[@]}" -gt 0 ]]; then
  echo "[preflight] missing required variables:"
  printf '  - %s\n' "${missing[@]}"
  exit 1
fi

echo "[preflight] environment checks passed."
