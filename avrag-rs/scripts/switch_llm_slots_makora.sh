#!/usr/bin/env bash
# Point every live LLM slot at makora deepseek-ai/DeepSeek-V4-Flash, on BOTH the
# local dev .env and the VPS /etc/avrag-rs/avrag.env. Key is reused from the
# existing AGENT_LLM_API_KEY and never echoed. Thinking: INGESTION=on,
# RETRIEVE/MEMORY/TRIPLET=off, AGENT untouched (already on).
set -uo pipefail
cd /home/chuan/context-osv6/avrag-rs
export PATH="$HOME/.cargo/bin:$HOME/.local/bin:$PATH"
set -a
# shellcheck disable=SC1091
source .env
set +a

apply() {
  local f="$1" key="$2"
  set_env() {
    local k="$1" v="$2" f="$3"
    if grep -q "^${k}=" "$f"; then
      sed -i "s|^${k}=.*|${k}=${v}|" "$f"
    else
      printf '%s=%s\n' "$k" "$v" >>"$f"
    fi
  }
  for SLOT in INGESTION_LLM RETRIEVE_LLM MEMORY_LLM TRIPLET_LLM; do
    set_env "${SLOT}_BASE_URL" "https://inference.makora.com/v1" "$f"
    set_env "${SLOT}_MODEL" "deepseek-ai/DeepSeek-V4-Flash" "$f"
    set_env "${SLOT}_API_KEY" "$key" "$f"
    set_env "${SLOT}_API_STYLE" "openai" "$f"
  done
  set_env "INGESTION_LLM_ENABLE_THINKING" "true" "$f"
  set_env "INGESTION_LLM_TIMEOUT_MS" "180000" "$f"
  set_env "RETRIEVE_LLM_ENABLE_THINKING" "false" "$f"
  set_env "MEMORY_LLM_ENABLE_THINKING" "false" "$f"
  set_env "TRIPLET_LLM_ENABLE_THINKING" "false" "$f"
  set_env "TRIPLET_LLM_TIMEOUT_MS" "180000" "$f"
}

show() {
  grep -E "^(AGENT_LLM|INGESTION_LLM|RETRIEVE_LLM|MEMORY_LLM|TRIPLET_LLM)_(BASE_URL|MODEL|API_STYLE|ENABLE_THINKING|TIMEOUT_MS)=" "$1" | sort
  echo "  (API_KEY lines present: $(grep -cE "^(AGENT_LLM|INGESTION_LLM|RETRIEVE_LLM|MEMORY_LLM|TRIPLET_LLM)_API_KEY=." "$1"))"
}

echo "== local .env BEFORE =="
show /home/chuan/context-osv6/avrag-rs/.env
KEY_LOCAL="$(grep '^AGENT_LLM_API_KEY=' /home/chuan/context-osv6/avrag-rs/.env | cut -d= -f2-)"
apply /home/chuan/context-osv6/avrag-rs/.env "$KEY_LOCAL"
echo "== local .env AFTER =="
show /home/chuan/context-osv6/avrag-rs/.env

echo "== VPS avrag.env =="
sshpass -p "$VPS_MAIN_PASSWORD" ssh -o StrictHostKeyChecking=no "$VPS_MAIN_USER@$VPS_MAIN_HOST" \
  "KEY=\$(grep '^AGENT_LLM_API_KEY=' /etc/avrag-rs/avrag.env | cut -d= -f2-); $(declare -f apply set_env); apply /etc/avrag-rs/avrag.env \"\$KEY\" && show /etc/avrag-rs/avrag.env"
