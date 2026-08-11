#!/usr/bin/env bash
# P4: wallet debit + share public read + ETag.
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT"
if [[ -f "$ROOT/../avrag-rs/.env" ]]; then
  set -a; source "$ROOT/../avrag-rs/.env"; set +a
fi
: "${DATABASE_URL:?}"
: "${EMBEDDING_API_KEY:?}"
export DEEPSEEK_API_KEY="${DEEPSEEK_API_KEY:-${CHAT_LLM_API_KEY:-}}"
: "${DEEPSEEK_API_KEY:?}"

USER_ID="p4-user-$(python3 -c 'import uuid;print(uuid.uuid4().hex[:8])')"
WS=$(python3 -c 'import uuid;print(uuid.uuid4())')
echo "user=$USER_ID workspace=$WS"

mkdir -p bin
go test ./internal/billing/ ./internal/share/ ./internal/ingest/ -count=1
go build -o bin/osv7d ./cmd/osv7d
go build -o bin/ingest-cli ./cmd/ingest-cli

export OSV7_ROOT="$ROOT" OSV7_ADDR="127.0.0.1:18096"
./bin/osv7d >/tmp/osv7d-p4.log 2>&1 &
PID=$!
cleanup() { kill "$PID" 2>/dev/null || true; }
trap cleanup EXIT
for i in $(seq 1 40); do curl -sf "http://127.0.0.1:18096/healthz" >/dev/null && break; sleep 0.15; done

echo "==> topup 100 fen"
curl -sS -X POST "http://127.0.0.1:18096/v1/billing/topup" \
  -H 'Content-Type: application/json' \
  -d "{\"user_id\":\"$USER_ID\",\"amount_fen\":100,\"idempotency_key\":\"topup-$USER_ID-1\"}" | tee /tmp/p4-topup.json
python3 -c 'import json;d=json.load(open("/tmp/p4-topup.json")); assert d["balance_fen"]==100, d'

echo "==> chat debit 10 fen"
curl -sS -X POST "http://127.0.0.1:18096/v1/chat" \
  -H 'Content-Type: application/json' \
  -d "{\"message\":\"只输出：hi\",\"user_id\":\"$USER_ID\",\"harness\":false,\"persist\":true,\"timeout_sec\":90}" \
  | tee /tmp/p4-chat.json
python3 -c 'import json;d=json.load(open("/tmp/p4-chat.json")); assert d.get("answer")'
curl -sS "http://127.0.0.1:18096/v1/billing/wallet?user_id=$USER_ID" | tee /tmp/p4-wallet.json
python3 -c 'import json;d=json.load(open("/tmp/p4-wallet.json")); print("balance",d["balance_fen"]); assert d["balance_fen"]==90'

echo "==> floor reject"
# drain to 0
USER2="p4-broke-$(python3 -c 'import uuid;print(uuid.uuid4().hex[:6])')"
curl -sS -X POST "http://127.0.0.1:18096/v1/chat" \
  -H 'Content-Type: application/json' \
  -d "{\"message\":\"x\",\"user_id\":\"$USER2\",\"harness\":false,\"persist\":false,\"timeout_sec\":30}" \
  | tee /tmp/p4-floor.json || true
# expect 402 body
code=$(curl -sS -o /tmp/p4-floor.json -w '%{http_code}' -X POST "http://127.0.0.1:18096/v1/chat" \
  -H 'Content-Type: application/json' \
  -d "{\"message\":\"x\",\"user_id\":\"$USER2\",\"harness\":false,\"persist\":false,\"timeout_sec\":30}")
echo "floor http=$code"
test "$code" = "402"
python3 -c 'import json;d=json.load(open("/tmp/p4-floor.json")); assert d["error"]=="balance_insufficient"; print(d["fact"][:80])'

echo "==> BYOK skips debit"
curl -sS -X POST "http://127.0.0.1:18096/v1/billing/byok" -H 'Content-Type: application/json' \
  -d "{\"user_id\":\"$USER2\",\"capability\":\"chat\",\"enabled\":true}" >/dev/null
code=$(curl -sS -o /tmp/p4-byok.json -w '%{http_code}' -X POST "http://127.0.0.1:18096/v1/chat" \
  -H 'Content-Type: application/json' \
  -d "{\"message\":\"只输出1\",\"user_id\":\"$USER2\",\"harness\":false,\"persist\":false,\"timeout_sec\":90}")
echo "byok chat http=$code"
test "$code" = "200"

echo "==> ingest with owner debit (2 chunks = 2 fen)"
export OSV7_OWNER_USER_ID="$USER_ID"
cat > /tmp/p4-ir.json <<'JSON'
{"title":"p4 share doc","blocks":[{"text":"P4-SHARE-MARKER-777 unique blob for public share sample."},{"text":"second block for two-chunk debit."}]}
JSON
./bin/ingest-cli agent-package --workspace "$WS" --file /tmp/p4-ir.json | tee /tmp/p4-ingest.json
curl -sS "http://127.0.0.1:18096/v1/billing/wallet?user_id=$USER_ID" | tee /tmp/p4-wallet2.json
python3 -c 'import json;d=json.load(open("/tmp/p4-wallet2.json")); print("after ingest",d["balance_fen"]); assert d["balance_fen"]==88'

echo "==> share create + public GET + ETag"
curl -sS -X POST "http://127.0.0.1:18096/v1/share" \
  -H 'Content-Type: application/json' \
  -d "{\"workspace_id\":\"$WS\",\"owner_user_id\":\"$USER_ID\",\"title\":\"P4 demo\",\"ttl_hours\":24}" \
  | tee /tmp/p4-share.json
TOK=$(python3 -c 'import json;print(json.load(open("/tmp/p4-share.json"))["token"])')
curl -sS -D /tmp/p4-pub.hdr "http://127.0.0.1:18096/public/s/$TOK" | tee /tmp/p4-pub.json
python3 - <<'PY'
import json
d=json.load(open("/tmp/p4-pub.json"))
assert d["workspace_id"]
assert d["chunk_count"]>=1
assert d.get("etag")
print("public ok chunks", d["chunk_count"], "etag", d["etag"])
PY
ETAG=$(python3 -c 'import json;print(json.load(open("/tmp/p4-pub.json"))["etag"])')
code=$(curl -sS -o /dev/null -w '%{http_code}' -H "If-None-Match: $ETAG" "http://127.0.0.1:18096/public/s/$TOK")
echo "etag 304? http=$code"
test "$code" = "304"

echo "==> P4 smoke OK"
