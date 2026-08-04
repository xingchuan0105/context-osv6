#!/usr/bin/env bash
# SkillOpt bottom-up (L1.5+L2): optimize knowledge-base SKILL on full 149.
# Dual-channel Flash for rollout (process env overrides .env):
#   agent/chat/e2e → OpenCode Go deepseek-v4-flash
#   judge/memory   → Ollama Cloud deepseek-v4-flash:0731-cloud
#   mutual FALLBACKS on rate-limit / 5xx
# Optimizer reflect uses QWEN_CHAT_* (already OpenCode Go in .env).
set -euo pipefail
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
SK="$ROOT/avrag-rs/tools/skillopt"
ENV_FILE="$ROOT/avrag-rs/.env"
if [[ -f "$ENV_FILE" ]]; then
  set -a
  # shellcheck disable=SC1090
  source "$ENV_FILE"
  set +a
fi

: "${OPENCODE_GO_API_KEY:?}"
: "${OPENCODE_GO_BASE_URL:=https://opencode.ai/zen/go/v1}"
: "${OLLAMA_API_KEY:?}"
: "${OLLAMA_HOST:=https://ollama.com}"
OLLAMA_V1="${OLLAMA_HOST%/}/v1"
OLLAMA_FLASH="${OLLAMA_MODEL:-deepseek-v4-flash:0731-cloud}"
GO_FLASH=deepseek-v4-flash

export DENSE_BACKEND=vgrag
export RETRIEVAL_GRAPH_AUGMENT=0
unset E2E_FORCE_INGEST 2>/dev/null || true
export E2E_MODE=nightly
export E2E_CONCURRENCY="${E2E_CONCURRENCY:-8}"
export CARGO_BUILD_JOBS="${CARGO_BUILD_JOBS:-2}"
export RAG_EVAL_V2=1

# Rollout agent
export AGENT_LLM_BASE_URL="$OPENCODE_GO_BASE_URL"
export AGENT_LLM_API_KEY="$OPENCODE_GO_API_KEY"
export AGENT_LLM_MODEL="$GO_FLASH"
export E2E_LLM_BASE_URL="$OPENCODE_GO_BASE_URL"
export E2E_LLM_API_KEY="$OPENCODE_GO_API_KEY"
export E2E_LLM_MODEL="$GO_FLASH"
export CHAT_LLM_BASE_URL="$OPENCODE_GO_BASE_URL"
export CHAT_LLM_API_KEY="$OPENCODE_GO_API_KEY"
export CHAT_LLM_MODEL="$GO_FLASH"
export SEARCH_LLM_BASE_URL="$OPENCODE_GO_BASE_URL"
export SEARCH_LLM_API_KEY="$OPENCODE_GO_API_KEY"
export SEARCH_LLM_MODEL="$GO_FLASH"

# Judge
export JUDGE_LLM_BASE_URL="$OLLAMA_V1"
export JUDGE_LLM_API_KEY="$OLLAMA_API_KEY"
export JUDGE_LLM_MODEL="$OLLAMA_FLASH"
export MEMORY_LLM_BASE_URL="$OLLAMA_V1"
export MEMORY_LLM_API_KEY="$OLLAMA_API_KEY"
export MEMORY_LLM_MODEL="$OLLAMA_FLASH"

export AGENT_LLM_FALLBACKS="[{\"base_url\":\"${OLLAMA_V1}\",\"api_key\":\"${OLLAMA_API_KEY}\",\"model\":\"${OLLAMA_FLASH}\"}]"
export JUDGE_LLM_FALLBACKS="[{\"base_url\":\"${OPENCODE_GO_BASE_URL}\",\"api_key\":\"${OPENCODE_GO_API_KEY}\",\"model\":\"${GO_FLASH}\"}]"
export E2E_LLM_FALLBACKS="$AGENT_LLM_FALLBACKS"
export CHAT_LLM_FALLBACKS="$AGENT_LLM_FALLBACKS"

# Optimizer reflect (skillopt qwen_chat) — OpenCode Go
export QWEN_CHAT_BASE_URL="$OPENCODE_GO_BASE_URL"
export QWEN_CHAT_API_KEY="$OPENCODE_GO_API_KEY"
export QWEN_CHAT_MODEL="$GO_FLASH"

LOG_DIR="$ROOT/output/runtime-logs"
mkdir -p "$LOG_DIR"
STAMP="$(date -u +%Y%m%d-%H%M%S)"
LOG="$LOG_DIR/skillopt_retrieval_full149_${STAMP}.log"

echo "[skillopt] bottom-up L1.5+L2 full149 prompt_target=knowledge-base/SKILL.md" | tee "$LOG"
echo "[skillopt] agent=OpenCodeGo/$GO_FLASH judge=Ollama/$OLLAMA_FLASH concurrency=$E2E_CONCURRENCY" | tee -a "$LOG"

cd "$SK"
# ensure venv
if [[ ! -x .venv/bin/python ]]; then
  bash scripts/setup.sh
fi

# static check first
.venv/bin/python train_avrag149.py --config configs/avrag149/retrieval-full149.yaml --check 2>&1 | tee -a "$LOG"

echo "[skillopt] starting train…" | tee -a "$LOG"
# no parallel full cargo tests
export E2E_CONCURRENCY
nohup .venv/bin/python train_avrag149.py --config configs/avrag149/retrieval-full149.yaml \
  >>"$LOG" 2>&1 &
echo $! >"$LOG_DIR/skillopt_retrieval_full149.pid"
echo "[skillopt] pid=$(cat "$LOG_DIR/skillopt_retrieval_full149.pid") log=$LOG"
