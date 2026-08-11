#!/usr/bin/env bash
# P2: agentd one-shot chat via pi RPC + outbound gate (deepseek).
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT"

if [[ -f "$ROOT/../avrag-rs/.env" ]]; then
  set -a
  # shellcheck disable=SC1091
  source "$ROOT/../avrag-rs/.env"
  set +a
fi

# Prefer product deepseek keys
export DEEPSEEK_API_KEY="${DEEPSEEK_API_KEY:-${CHAT_LLM_API_KEY:-${AGENT_LLM_API_KEY:-}}}"
: "${DEEPSEEK_API_KEY:?DEEPSEEK_API_KEY or CHAT_LLM_API_KEY required}"

export OSV7_PI_PROVIDER="${OSV7_PI_PROVIDER:-deepseek}"
export OSV7_PI_MODEL="${OSV7_PI_MODEL:-deepseek-v4-flash}"

mkdir -p bin
echo "==> unit tests agentd gate"
go test ./internal/agentd/ -count=1

echo "==> build agentd-chat"
go build -o bin/agentd-chat ./cmd/agentd-chat

MSG="${1:-用不超过十个汉字回答：1+1等于几？只输出数字。}"
echo "==> chat: $MSG"
./bin/agentd-chat -no-extensions -json -timeout 120s "$MSG" | tee /tmp/p2-agentd-chat.json

python3 - <<'PY'
import json,sys
d=json.load(open("/tmp/p2-agentd-chat.json"))
assert not d.get("blocked"), d
assert d.get("answer"), d
print("OK answer=", d["answer"][:200])
print("events=", d.get("events"), "duration_ms=", d.get("duration_ms"))
PY
echo "==> P2 agentd smoke OK"
