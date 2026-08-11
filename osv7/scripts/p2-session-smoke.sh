#!/usr/bin/env bash
# P2 wrap: multi-turn projection + card-keeper signals + HTTP sessions API.
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
: "${DEEPSEEK_API_KEY:?need deepseek}"
: "${DATABASE_URL:?need DATABASE_URL for projection}"

export OSV7_ROOT="$ROOT"
export OSV7_AGENTD_ADDR="127.0.0.1:18095"
export OSV7_SESSION_DIR="/tmp/osv7-pi-sessions-smoke"
mkdir -p bin "$OSV7_SESSION_DIR"

echo "==> build"
go build -o bin/agentd-server ./cmd/agentd-server
go build -o bin/retrieval-cli ./cmd/retrieval-cli
go test ./internal/store/ ./internal/agentd/ -count=1

./bin/agentd-server >/tmp/agentd-sess.log 2>&1 &
SRV=$!
cleanup() { kill "$SRV" 2>/dev/null || true; }
trap cleanup EXIT
for i in $(seq 1 40); do
  curl -sf "http://127.0.0.1:18095/healthz" >/dev/null && break
  sleep 0.15
done

echo "==> turn 1 create session"
curl -sS -X POST "http://127.0.0.1:18095/v1/chat" \
  -H 'Content-Type: application/json' \
  -d '{"message":"记住这个暗号：蓝鸟。只回答：收到。","user_id":"p2-smoke","persist":true,"harness":false,"timeout_sec":90}' \
  | tee /tmp/p2-sess-t1.json
SID=$(python3 -c 'import json;print(json.load(open("/tmp/p2-sess-t1.json")).get("product_session_id",""))')
echo "product_session_id=$SID"
test -n "$SID"

echo "==> turn 2 multi-turn"
curl -sS -X POST "http://127.0.0.1:18095/v1/chat" \
  -H 'Content-Type: application/json' \
  -d "{\"message\":\"我刚才说的暗号是什么？只回答暗号本身。\",\"user_id\":\"p2-smoke\",\"session_id\":\"$SID\",\"persist\":true,\"harness\":false,\"timeout_sec\":90}" \
  | tee /tmp/p2-sess-t2.json
python3 - <<'PY'
import json
d=json.load(open("/tmp/p2-sess-t2.json"))
ans=(d.get("answer") or "")
print("t2 answer:", ans[:120])
assert "蓝鸟" in ans or "蓝" in ans, d
assert d.get("product_session_id")
PY

echo "==> list sessions / messages"
curl -sS "http://127.0.0.1:18095/v1/sessions?user_id=p2-smoke" | tee /tmp/p2-sess-list.json | head -c 400; echo
curl -sS "http://127.0.0.1:18095/v1/sessions/$SID/messages" | tee /tmp/p2-sess-msgs.json | head -c 600; echo
python3 - <<'PY'
import json
msgs=json.load(open("/tmp/p2-sess-msgs.json"))["messages"]
roles=[m["role"] for m in msgs]
print("roles", roles, "n", len(msgs))
assert roles.count("user")>=2 and roles.count("assistant")>=2
# only gated content in assistant bubbles
for m in msgs:
    if m["role"]=="assistant":
        assert "tool_call" not in m["content"].lower()
print("projection OK")
PY

echo "==> card-keeper soft signal (harness without forcing tools is flaky; check fields exist)"
WS="${OSV7_WORKSPACE_ID:-0c8391f1-8bfb-415f-9a7f-10624b7cfb4d}"
curl -sS -X POST "http://127.0.0.1:18095/v1/chat" \
  -H 'Content-Type: application/json' \
  -d "{\"message\":\"1+1=? 只输出数字，不要检索。\",\"workspace_id\":\"$WS\",\"harness\":true,\"persist\":false,\"timeout_sec\":90}" \
  | tee /tmp/p2-card.json
python3 - <<'PY'
import json
d=json.load(open("/tmp/p2-card.json"))
print("card_missing", d.get("card_missing"), "retrieval_invoked", d.get("retrieval_invoked"))
print("observation", (d.get("card_observation") or "")[:160])
assert d.get("harness_enabled") is True
assert d.get("card_missing") is True
assert d.get("retrieval_invoked") is False
assert d.get("card_observation")
print("card-keeper fields OK")
PY

echo "==> P2 session smoke OK"
