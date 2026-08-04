#!/usr/bin/env bash
# SkillOpt one progressive spoke (few-shot + gotcha), full-149 slow train.
#
# Usage:
#   bash scripts/run-skillopt-spoke.sh graph|tables|grounding|codegen|codegen-codepass|thin-strategies
#   E2E_CONCURRENCY=10 bash scripts/run-skillopt-spoke.sh codegen-codepass
#   # 断点续跑（SkillOpt 读 out_root/runtime_state.json → 从 last_completed_step+1 继续）：
#   OUT_ROOT=avrag-rs/tools/skillopt/outputs/skillopt_avrag149_YYYYMMDD_HHMMSS \
#     bash scripts/run-skillopt-spoke.sh codegen-codepass
#
# Dual-channel Flash (process env overrides .env):
#   agent  = OpenCode Go / deepseek-v4-flash
#   judge  = Ollama / deepseek-v4-flash:0731-cloud
#   optimizer (SkillOpt reflect / slow_update / merge) = Ollama same Flash
#     (Go returns HTTP 403 on long optimizer calls; agent path stays on Go)
# Forces the spoke under edit into first-round context via
# E2E_SKILLOPT_FORCE_KB_REFS so progressive disclosure does not hide trainable text.
#
# Spoke configs set evaluation.eval_test: false — no per-spoke baseline/best/final
# test tax. After graph→tables→grounding→codegen (+ optional thin), run one-shot accept:
#   avrag-rs/tools/skillopt/.venv/bin/python eval_avrag149.py --skill <best_skill.md>
# (or a wrapper when multi-spoke product pack is ready).
#
# codegen-codepass: score_mode=code_pass (L1.5 一次通过率，非终答 PASS).
set -euo pipefail
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
SK="$ROOT/avrag-rs/tools/skillopt"
ENV_FILE="$ROOT/avrag-rs/.env"
SPOKE="${1:-}"
case "$SPOKE" in
  graph)   CFG=configs/avrag149/spoke-graph-full149.yaml; FORCE=strategies-graph ;;
  tables)  CFG=configs/avrag149/spoke-tables-full149.yaml; FORCE=strategies-tables ;;
  grounding) CFG=configs/avrag149/spoke-grounding-full149.yaml; FORCE=strategies-grounding ;;
  codegen) CFG=configs/avrag149/spoke-codegen-full149.yaml; FORCE=strategies-codegen ;;
  codegen-codepass|codepass)
    CFG=configs/avrag149/spoke-codegen-codepass-full149.yaml; FORCE=strategies-codegen ;;
  thin-strategies|strategies)
    CFG=configs/avrag149/spoke-thin-strategies-full149.yaml; FORCE=strategies ;;
  *)
    echo "usage: $0 graph|tables|grounding|codegen|codegen-codepass|thin-strategies" >&2
    exit 2
    ;;
esac

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
# Default 10 (judge/agent load); override on CLI if needed.
export E2E_CONCURRENCY="${E2E_CONCURRENCY:-10}"
export CARGO_BUILD_JOBS="${CARGO_BUILD_JOBS:-2}"
export RAG_EVAL_V2=1
export E2E_SKILLOPT_FORCE_KB_REFS="$FORCE"

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
# Optimizer must be a stable OpenAI-compatible chat endpoint. Ollama 0731 is
# reliable here; Go 403'd reflect calls (error 1010) and zeroed all patches.
export QWEN_CHAT_BASE_URL="$OLLAMA_V1"
export QWEN_CHAT_API_KEY="$OLLAMA_API_KEY"
export QWEN_CHAT_MODEL="$OLLAMA_FLASH"
# skillopt trainer reads OPTIMIZER_DEPLOYMENT / flat optimizer_model; empty
# model.optimizer in yaml falls back to package default Qwen/Qwen3.5-4B.
export OPTIMIZER_DEPLOYMENT="$OLLAMA_FLASH"
export TARGET_DEPLOYMENT="$OLLAMA_FLASH"
export QWEN_CHAT_ENABLE_THINKING=false
export QWEN_CHAT_MAX_TOKENS="${QWEN_CHAT_MAX_TOKENS:-8000}"

LOG_DIR="$ROOT/output/runtime-logs"
mkdir -p "$LOG_DIR"
STAMP="$(date -u +%Y%m%d-%H%M%S)"
LOG="$LOG_DIR/skillopt_spoke_${SPOKE}_${STAMP}.log"

echo "[skillopt-spoke] spoke=$SPOKE force_refs=$FORCE cfg=$CFG" | tee "$LOG"
echo "[skillopt-spoke] agent=Go/$GO_FLASH judge=Ollama/$OLLAMA_FLASH optimizer=Ollama/$OLLAMA_FLASH concurrency=$E2E_CONCURRENCY" | tee -a "$LOG"

cd "$SK"
export PYTHONUNBUFFERED=1

# Optional resume: reuse an existing trainer out_root (runtime_state.json / history).
TRAIN_ARGS=(--config "$CFG")
if [[ -n "${OUT_ROOT:-}" ]]; then
  if [[ "$OUT_ROOT" != /* ]]; then
    OUT_ROOT="$ROOT/$OUT_ROOT"
  fi
  OUT_ROOT="$(cd "$(dirname "$OUT_ROOT")" && pwd)/$(basename "$OUT_ROOT")"
  if [[ ! -d "$OUT_ROOT" ]]; then
    echo "[skillopt-spoke] OUT_ROOT does not exist: $OUT_ROOT" >&2
    exit 1
  fi
  TRAIN_ARGS+=(--out_root "$OUT_ROOT")
  if [[ -f "$OUT_ROOT/runtime_state.json" ]]; then
    echo "[skillopt-spoke] resume OUT_ROOT=$OUT_ROOT" | tee -a "$LOG"
    python3 -c "import json;s=json.load(open('$OUT_ROOT/runtime_state.json'));print('  last_completed_step=',s.get('last_completed_step'),'best=',s.get('best_score'))" | tee -a "$LOG"
  else
    echo "[skillopt-spoke] OUT_ROOT set but no runtime_state.json — will start/continue in that dir" | tee -a "$LOG"
  fi
fi

.venv/bin/python train_avrag149.py "${TRAIN_ARGS[@]}" --check 2>&1 | tee -a "$LOG"
nohup env PYTHONUNBUFFERED=1 .venv/bin/python train_avrag149.py "${TRAIN_ARGS[@]}" >>"$LOG" 2>&1 &
echo $! >"$LOG_DIR/skillopt_spoke_${SPOKE}.pid"
echo "[skillopt-spoke] pid=$(cat "$LOG_DIR/skillopt_spoke_${SPOKE}.pid") log=$LOG"
if [[ -n "${OUT_ROOT:-}" ]]; then
  echo "[skillopt-spoke] out_root=$OUT_ROOT"
fi
