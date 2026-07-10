# shellcheck shell=bash
# Shared helpers for pyramid / DR scripts. Source from other scripts:
#   # shellcheck source=pyramid-lib.sh
#   source "$(dirname "$0")/pyramid-lib.sh"

pyramid_fail() {
  local layer="$1"
  local signal="$2"
  local next="$3"
  shift 3 || true
  local detail="${*:-}"
  echo ""
  echo "[PYRAMID] FAIL layer=${layer} signal=${signal}"
  if [[ -n "$detail" ]]; then
    echo "[PYRAMID] detail=${detail}"
  fi
  echo "[PYRAMID] next= ${next}"
  echo "[PYRAMID] triage= red at ${layer} → re-run narrower command above; do not open full L3 first"
  return 1
}

pyramid_ok() {
  local layer="$1"
  echo "[PYRAMID] layer=${layer} result=OK"
}

_pyramid_env_file() {
  if [[ -n "${PYRAMID_ENV_FILE:-}" ]]; then
    echo "${PYRAMID_ENV_FILE}"
    return
  fi
  local root
  root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
  echo "${root}/avrag-rs/.env"
}

# Read a single KEY=value from .env without sourcing the whole file (no secret export flood).
# Strips optional surrounding quotes. Does not expand shell variables inside values.
_pyramid_env_get() {
  local key="$1"
  local envf
  envf="$(_pyramid_env_file)"
  [[ -f "$envf" ]] || return 1
  local line
  line="$(grep -E "^${key}=" "$envf" 2>/dev/null | head -1 || true)"
  [[ -n "$line" ]] || return 1
  local val="${line#"${key}="}"
  # strip surrounding " or '
  if [[ "$val" =~ ^\".*\"$ ]]; then
    val="${val:1:${#val}-2}"
  elif [[ "$val" =~ ^\'.*\'$ ]]; then
    val="${val:1:${#val}-2}"
  fi
  # skip empty / placeholder
  [[ -n "$val" ]] || return 1
  printf '%s' "$val"
}

# Export only the listed keys if unset (for dump/L3). Prefer existing env.
pyramid_export_keys_if_unset() {
  local key val
  for key in "$@"; do
    if [[ -n "${!key:-}" ]]; then
      continue
    fi
    if val="$(_pyramid_env_get "$key")"; then
      export "${key}=${val}"
    fi
  done
}

# Load only DATABASE_URL for ops dump (not full .env).
pyramid_load_database_url() {
  pyramid_export_keys_if_unset DATABASE_URL
}

# True if thin real-LLM sample can run (env or .env peek, no full source).
pyramid_has_llm_keys() {
  if [[ -n "${AGENT_LLM_API_KEY:-}" || -n "${DEEPSEEK_API_KEY:-}" || -n "${DMX_API_KEY:-}" ]]; then
    return 0
  fi
  _pyramid_env_get AGENT_LLM_API_KEY >/dev/null && return 0
  _pyramid_env_get DEEPSEEK_API_KEY >/dev/null && return 0
  _pyramid_env_get DMX_API_KEY >/dev/null && return 0
  return 1
}

# Export LLM keys only if L3 will run (still minimal set).
pyramid_export_llm_keys_if_needed() {
  pyramid_export_keys_if_unset AGENT_LLM_API_KEY DEEPSEEK_API_KEY DMX_API_KEY
}

pyramid_has_playwright() {
  command -v pnpm >/dev/null 2>&1
}

# Deprecated name kept for callers: no longer sources entire .env.
pyramid_load_env_if_present() {
  pyramid_load_database_url
}
