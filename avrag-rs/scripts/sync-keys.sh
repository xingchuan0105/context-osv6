#!/usr/bin/env bash
# sync-keys.sh — Copy API keys from vault into .env
#
# Usage:
#   bash scripts/sync-keys.sh          # sync all keys
#   bash scripts/sync-keys.sh --check  # check which keys are missing
#
# Vault: ~/.config/avrag/keys.env
# Target: .env in project root
#
# This script is the ONLY approved way to populate API keys into .env.
# LLMs should run this script instead of writing keys manually.

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
ENV_FILE="$PROJECT_ROOT/.env"
ENV_EXAMPLE="$PROJECT_ROOT/.env.example"
VAULT="$HOME/.config/avrag/keys.env"

# Key variables managed by this script
# Source: crates/app/src/lib.rs AppConfig::from_env()
KEY_VARS=(
  # DashScope — embedding, MM embedding, MM rerank, rerank
  DASHSCOPE_API_KEY
  EMBEDDING_API_KEY
  MM_EMBEDDING_API_KEY
  MM_RERANK_API_KEY
  RERANK_API_KEY
  # Agent LLM — Chat, RAG, WebSearch (DeepSeek)
  AGENT_LLM_API_KEY
  # Memory LLM — Session summary, user profile (DeepSeek Flash)
  MEMORY_LLM_API_KEY
  # Ingestion LLM — Document summary, triplets (Gemini via DMXAPI)
  INGESTION_LLM_API_KEY
  # Search — Brave LLM Context primary, Perplexity legacy
  SEARCH_API_KEY
  PERPLEXITY_API_KEY
  # Document parsing
  MINERU_API_KEY
)

CHECK_ONLY=false
if [[ "${1:-}" == "--check" ]]; then
  CHECK_ONLY=true
fi

# Ensure vault exists
if [[ ! -f "$VAULT" ]]; then
  echo "ERROR: Key vault not found at $VAULT"
  echo "Create it with: mkdir -p ~/.config/avrag && touch ~/.config/avrag/keys.env"
  echo ""
  echo "Required variables:"
  for v in "${KEY_VARS[@]}"; do
    echo "  $v="
  done
  exit 1
fi

# Load vault values
declare -A VAULT_VALUES
while IFS='=' read -r key value; do
  # Skip comments and empty lines
  [[ "$key" =~ ^[[:space:]]*# ]] && continue
  [[ -z "$key" ]] && continue
  key="$(echo "$key" | xargs)"  # trim whitespace
  value="$(echo "$value" | xargs)"  # trim whitespace
  VAULT_VALUES["$key"]="$value"
done < "$VAULT"

# Check mode: report missing keys
if $CHECK_ONLY; then
  echo "=== API Key Status ==="
  missing=0
  for v in "${KEY_VARS[@]}"; do
    val="${VAULT_VALUES[$v]:-}"
    if [[ -z "$val" ]]; then
      echo "  MISSING  $v"
      missing=$((missing + 1))
    else
      # Show first 8 chars + asterisks
      masked="${val:0:8}****"
      echo "  OK       $v = $masked"
    fi
  done
  echo ""
  if [[ $missing -eq 0 ]]; then
    echo "All keys configured."
  else
    echo "$missing key(s) missing. Edit $VAULT to add them."
  fi
  exit 0
fi

# Ensure .env exists
if [[ ! -f "$ENV_FILE" ]]; then
  if [[ -f "$ENV_EXAMPLE" ]]; then
    cp "$ENV_EXAMPLE" "$ENV_FILE"
    echo "Created .env from .env.example"
  else
    touch "$ENV_FILE"
    echo "Created empty .env"
  fi
fi

# For each key variable, update or append in .env
updated=0
for v in "${KEY_VARS[@]}"; do
  val="${VAULT_VALUES[$v]:-}"
  if [[ -z "$val" ]]; then
    continue
  fi

  # Check if variable already exists in .env (with any value)
  if grep -q "^${v}=" "$ENV_FILE" 2>/dev/null; then
    # Replace existing line
    sed -i "s|^${v}=.*|${v}=${val}|" "$ENV_FILE"
  else
    # Append
    echo "${v}=${val}" >> "$ENV_FILE"
  fi
  updated=$((updated + 1))
done

echo "Synced $updated key(s) from vault to .env"

# Also sync non-key config lines that are in .env.example but missing from .env
# (base_url, model names, etc.)
while IFS='=' read -r key value; do
  [[ "$key" =~ ^[[:space:]]*# ]] && continue
  [[ -z "$key" ]] && continue
  key="$(echo "$key" | xargs)"
  value="$(echo "$value" | xargs)"
  [[ -z "$key" ]] && continue
  # Skip key variables (already handled above)
  for kv in "${KEY_VARS[@]}"; do
    [[ "$key" == "$kv" ]] && continue 2
  done
  # If this config line is missing from .env, append it
  if ! grep -q "^${key}=" "$ENV_FILE" 2>/dev/null; then
    echo "${key}=${value}" >> "$ENV_FILE"
  fi
done < "$ENV_EXAMPLE"

echo "Done. .env is ready."
