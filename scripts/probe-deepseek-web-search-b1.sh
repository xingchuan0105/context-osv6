#!/usr/bin/env bash
# B1: Probe whether DeepSeek official API exposes usable native web search.
# Does not change product wiring. Credentials: avrag-rs/.env AGENT_LLM_* or E2E_LLM_*.
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
ENV_FILE="$ROOT/avrag-rs/.env"
REPORT_DIR="$ROOT/docs/engineering/_reports"
mkdir -p "$REPORT_DIR"
REPORT="$REPORT_DIR/deepseek-web-b1-latest.md"

if [[ -f "$ENV_FILE" ]]; then
  set -a
  # shellcheck disable=SC1090
  source "$ENV_FILE"
  set +a
fi

BASE="${AGENT_LLM_BASE_URL:-${E2E_LLM_BASE_URL:-https://api.deepseek.com}}"
KEY="${AGENT_LLM_API_KEY:-${E2E_LLM_API_KEY:-}}"
MODEL="${AGENT_LLM_MODEL:-${E2E_LLM_MODEL:-deepseek-v4-flash}}"
BASE="${BASE%/}"

if [[ -z "$KEY" ]]; then
  echo "probe-deepseek-web-search-b1: missing AGENT_LLM_API_KEY / E2E_LLM_API_KEY" >&2
  exit 1
fi

ts="$(date -u +%Y-%m-%dT%H:%M:%SZ)"
tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"' EXIT

note() { echo "$*" | tee -a "$tmp/log.txt"; }

note "# DeepSeek native web search B1 probe"
note ""
note "- time_utc: $ts"
note "- base_url: $BASE"
note "- model: $MODEL"
note "- key: set (len=${#KEY})"
note ""

# --- A: plain chat completions ---
note "## A — OpenAI chat/completions (no tools)"
code_a="$(curl -sS -m 45 -o "$tmp/a.json" -w '%{http_code}' \
  -X POST "$BASE/v1/chat/completions" \
  -H "Authorization: Bearer $KEY" \
  -H "Content-Type: application/json" \
  -d "$(python3 -c "import json; print(json.dumps({
    'model': '''$MODEL''',
    'messages': [{'role':'user','content':'Reply with exactly: pong'}],
    'max_tokens': 32,
    'temperature': 0
  }))")" || echo 000)"
note "- http: $code_a"
python3 - <<'PY' "$tmp/a.json" | tee -a "$tmp/log.txt"
import json,sys
p=sys.argv[1]
try:
  d=json.load(open(p))
  ch=(d.get("choices") or [{}])[0]
  msg=(ch.get("message") or {})
  print("- content_snippet:", repr((msg.get("content") or "")[:120]))
  print("- finish_reason:", ch.get("finish_reason"))
except Exception as e:
  print("- parse_err:", e, open(p).read()[:200])
PY

# --- B: tools web_search variants ---
note ""
note "## B — OpenAI tools (web_search / web_search_preview shapes)"
for tools_json in \
  '[{"type":"web_search"}]' \
  '[{"type":"function","function":{"name":"web_search","description":"Search the web","parameters":{"type":"object","properties":{"query":{"type":"string"}},"required":["query"]}}}]' \
  '[{"type":"web_search_preview"}]'
do
  label="$(echo "$tools_json" | head -c 60)"
  code_b="$(curl -sS -m 45 -o "$tmp/b.json" -w '%{http_code}' \
    -X POST "$BASE/v1/chat/completions" \
    -H "Authorization: Bearer $KEY" \
    -H "Content-Type: application/json" \
    -d "$(python3 -c "import json,sys; tools=json.loads(sys.argv[1]); print(json.dumps({
      'model': '''$MODEL''',
      'messages': [{'role':'user','content':'What is the latest DeepSeek model name announced officially? Use tools if available.'}],
      'max_tokens': 256,
      'temperature': 0,
      'tools': tools
    }))" "$tools_json")" || echo 000)"
  note "- tools=$label → http=$code_b"
  python3 - <<'PY' "$tmp/b.json" | tee -a "$tmp/log.txt"
import json,sys
p=sys.argv[1]
try:
  d=json.load(open(p))
  if d.get("error"):
    print("  error:", d["error"].get("message") or d["error"])
  else:
    ch=(d.get("choices") or [{}])[0]
    msg=ch.get("message") or {}
    print("  finish:", ch.get("finish_reason"), "tool_calls:", bool(msg.get("tool_calls")), "content_len:", len(msg.get("content") or ""))
except Exception as e:
  print("  parse_err:", e, open(p).read()[:160].replace("\n"," "))
PY
done

# --- C: Anthropic-compatible messages ---
note ""
note "## C — Anthropic-compatible /anthropic/v1/messages"
ANTH_BASE="$BASE"
if [[ "$ANTH_BASE" != *anthropic* ]]; then
  ANTH_BASE="${BASE}/anthropic"
fi
code_c="$(curl -sS -m 45 -o "$tmp/c.json" -w '%{http_code}' \
  -X POST "$ANTH_BASE/v1/messages" \
  -H "x-api-key: $KEY" \
  -H "anthropic-version: 2023-06-01" \
  -H "Content-Type: application/json" \
  -d "$(python3 -c "import json; print(json.dumps({
    'model': '''$MODEL''',
    'max_tokens': 256,
    'messages': [{'role':'user','content':'What day is today UTC? Short answer.'}],
  }))")" || echo 000)"
note "- endpoint: $ANTH_BASE/v1/messages"
note "- http: $code_c"
python3 - <<'PY' "$tmp/c.json" | tee -a "$tmp/log.txt"
import json,sys
p=sys.argv[1]
try:
  d=json.load(open(p))
  if d.get("error") or d.get("type")=="error":
    print("- error:", d.get("error") or d)
  else:
    content=d.get("content")
    print("- content_type:", type(content).__name__, "keys:", list(d.keys())[:12])
    if isinstance(content, list) and content:
      print("- block0:", str(content[0])[:200])
except Exception as e:
  print("- parse_err:", e, open(p).read()[:200].replace("\n"," "))
PY

# Anthropic with web_search tool (Claude-style)
code_c2="$(curl -sS -m 60 -o "$tmp/c2.json" -w '%{http_code}' \
  -X POST "$ANTH_BASE/v1/messages" \
  -H "x-api-key: $KEY" \
  -H "anthropic-version: 2023-06-01" \
  -H "Content-Type: application/json" \
  -d "$(python3 -c "import json; print(json.dumps({
    'model': '''$MODEL''',
    'max_tokens': 512,
    'messages': [{'role':'user','content':'Search the web: latest DeepSeek API model name. Cite sources if tools work.'}],
    'tools': [{'type': 'web_search_20250305', 'name': 'web_search', 'max_uses': 3}],
  }))")" || echo 000)"
note "- anthropic tools web_search_20250305 http: $code_c2"
python3 - <<'PY' "$tmp/c2.json" | tee -a "$tmp/log.txt"
import json,sys
p=sys.argv[1]
raw=open(p).read()
try:
  d=json.loads(raw)
  err=d.get("error") or (d if d.get("type")=="error" else None)
  if err:
    print("  error:", str(err)[:300])
  else:
    print("  stop_reason:", d.get("stop_reason"), "content_blocks:", len(d.get("content") or []))
    for b in (d.get("content") or [])[:4]:
      print("  block:", str(b)[:180])
except Exception as e:
  print("  parse_err:", e, raw[:200].replace("\n"," "))
PY

note ""
note "## Verdict (auto heuristic)"
python3 - <<'PY' | tee -a "$tmp/log.txt"
import json, pathlib
tmp = pathlib.Path(__import__("os").environ.get("TMPDIR", "/tmp"))
# re-read from files written above via argv not available — use cwd log only
print("- See HTTP codes above.")
print("- usable_native: true only if tools/web_search returned tool results or citations without 4xx.")
print("- If all tool shapes 400 and anthropic web_search errors → keep Brave for product.")
PY

cp "$tmp/log.txt" "$REPORT"
echo "probe-deepseek-web-search-b1: wrote $REPORT"
cat "$REPORT"
