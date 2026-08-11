#!/usr/bin/env bash
# P2 deepen: agentd + harness tools (set_query_card/lexical/dense) on real corpus.
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT"

if [[ -f "$ROOT/../avrag-rs/.env" ]]; then
  set -a
  # shellcheck disable=SC1091
  source "$ROOT/../avrag-rs/.env"
  set +a
fi
export DEEPSEEK_API_KEY="${DEEPSEEK_API_KEY:-${CHAT_LLM_API_KEY:-${AGENT_LLM_API_KEY:-}}}"
: "${DEEPSEEK_API_KEY:?need deepseek key}"
export OSV7_PI_PROVIDER="${OSV7_PI_PROVIDER:-deepseek}"
export OSV7_PI_MODEL="${OSV7_PI_MODEL:-deepseek-v4-flash}"

WS="${OSV7_WORKSPACE_ID:-0c8391f1-8bfb-415f-9a7f-10624b7cfb4d}"
mkdir -p bin
echo "==> build"
go build -o bin/retrieval-cli ./cmd/retrieval-cli
go build -o bin/agentd-chat ./cmd/agentd-chat
go build -o bin/agentd-server ./cmd/agentd-server
go test ./internal/agentd/ ./internal/retrieval/ -count=1

# CLI harness path (no LLM)
echo "==> retrieval-cli path"
rm -f /tmp/osv7-retrieval-smoke-state.json
export OSV7_RETRIEVAL_STATE=/tmp/osv7-retrieval-smoke-state.json
./bin/retrieval-cli set-card --workspace "$WS" --actions lexical,dense >/tmp/rc-card.json
./bin/retrieval-cli lexical --query "滴灌通" --limit 3 >/tmp/rc-lex.json
python3 - <<'PY'
import json
d=json.load(open("/tmp/rc-lex.json"))
assert d.get("total_hits",0)>=1, d
print("lexical hits", d["total_hits"], "aliases", [h["alias"] for h in d.get("handles",[])])
PY

MSG="${1:-请先 set_query_card（workspace 已在环境中），再用 lexical 或 dense 检索「滴灌通的核心机制 DRC」，根据证据用一两句话回答 DRC 全称是什么。不要编造。}"
echo "==> agentd-chat harness ws=$WS"
./bin/agentd-chat -harness -workspace "$WS" -json -timeout 180s "$MSG" | tee /tmp/p2-harness-chat.json

python3 - <<'PY'
import json
d=json.load(open("/tmp/p2-harness-chat.json"))
print("answer:", (d.get("answer") or "")[:300])
print("tools:", [t.get("name") for t in d.get("tools") or []])
print("blocked:", d.get("blocked"), "duration_ms:", d.get("duration_ms"))
tools={t.get("name") for t in (d.get("tools") or [])}
# soft assert: prefer harness tools used; don't fail hard if model answers from memory
if not d.get("answer"):
    raise SystemExit("empty answer")
if d.get("blocked"):
    raise SystemExit("gate blocked")
# report whether retrieval tools ran
print("harness_tools_used:", bool(tools & {"set_query_card","lexical","dense","grep"}))
PY

# HTTP JSON smoke (background server)
echo "==> agentd-server HTTP"
export OSV7_ROOT="$ROOT"
export OSV7_AGENTD_ADDR="127.0.0.1:18090"
./bin/agentd-server >/tmp/agentd-server.log 2>&1 &
SRV_PID=$!
cleanup() { kill "$SRV_PID" 2>/dev/null || true; }
trap cleanup EXIT
for i in $(seq 1 30); do
  if curl -sf "http://127.0.0.1:18090/healthz" >/dev/null; then break; fi
  sleep 0.2
done
curl -sf "http://127.0.0.1:18090/healthz"
echo
# short no-harness chat via HTTP
curl -sS -X POST "http://127.0.0.1:18090/v1/chat" \
  -H 'Content-Type: application/json' \
  -d '{"message":"只输出数字：3-1=？","harness":false,"timeout_sec":90}' \
  | tee /tmp/p2-http-chat.json
echo
python3 -c 'import json;d=json.load(open("/tmp/p2-http-chat.json")); assert d.get("answer"), d; print("http answer", d["answer"][:80])'

echo "==> P2 harness smoke done"
