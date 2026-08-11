#!/usr/bin/env bash
# Probe DeepSeek official Responses API web_search vs Anthropic messages web_search.
# Credentials: avrag-rs/.env AGENT_LLM_* (no secrets in report).
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
ENV_FILE="$ROOT/avrag-rs/.env"
REPORT_DIR="$ROOT/docs/engineering/_reports"
mkdir -p "$REPORT_DIR"
REPORT="$REPORT_DIR/deepseek-responses-web-latest.md"

if [[ -f "$ENV_FILE" ]]; then
  set -a
  # shellcheck disable=SC1090
  source "$ENV_FILE"
  set +a
fi

BASE="${AGENT_LLM_BASE_URL:-https://api.deepseek.com}"
KEY="${AGENT_LLM_API_KEY:-}"
MODEL="${AGENT_LLM_MODEL:-deepseek-v4-flash}"
BASE="${BASE%/}"
# Force flash for Responses (docs: only v4-flash)
RESP_MODEL="deepseek-v4-flash"
QUERY="${1:-What is BYOK Bring Your Own Key in cloud security?}"

if [[ -z "$KEY" ]]; then
  echo "missing AGENT_LLM_API_KEY" >&2
  exit 1
fi

tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"' EXIT
ts="$(date -u +%Y-%m-%dT%H:%M:%SZ)"

echo "== query =="
echo "$QUERY"
echo

# --- A: Responses API + web_search ---
cat >"$tmp/resp_body.json" <<EOF
{
  "model": "$RESP_MODEL",
  "input": $(python3 -c 'import json,sys; print(json.dumps(sys.argv[1]))' "$QUERY"),
  "tools": [{"type": "web_search"}],
  "max_output_tokens": 1024,
  "reasoning": {"effort": "none"}
}
EOF

echo "== A: POST $BASE/responses + tools web_search =="
t0=$(date +%s%3N)
code=$(curl -sS -o "$tmp/resp_out.json" -w '%{http_code}' \
  -X POST "$BASE/responses" \
  -H "Authorization: Bearer $KEY" \
  -H "Content-Type: application/json" \
  -H "Accept: application/json" \
  --data-binary @"$tmp/resp_body.json" \
  --max-time 120 || echo "000")
t1=$(date +%s%3N)
ms_a=$((t1 - t0))
echo "http=$code elapsed_ms=$ms_a"
python3 - <<'PY' "$tmp/resp_out.json" "$code" "$ms_a" "$tmp/a_summary.txt"
import json, sys, re
path, code, ms, outp = sys.argv[1:5]
raw = open(path, encoding="utf-8", errors="replace").read()
lines = [f"http={code}", f"elapsed_ms={ms}", f"body_bytes={len(raw)}"]
try:
    data = json.loads(raw)
except Exception as e:
    lines.append(f"json_error={e}")
    lines.append(raw[:800])
    open(outp, "w").write("\n".join(lines))
    print("\n".join(lines))
    raise SystemExit(0)

# top-level keys
lines.append("top_keys=" + ",".join(sorted(data.keys())[:30]))
# output_text helper if present
ot = data.get("output_text")
if isinstance(ot, str):
    lines.append(f"output_text_len={len(ot)}")
    lines.append("output_text_head=" + ot[:400].replace("\n", " "))

urls = set()
titles = []
web_calls = 0
text_parts = []

def walk(o, depth=0):
    global web_calls
    if isinstance(o, dict):
        t = o.get("type")
        if t in ("web_search_call", "web_search_tool_result", "server_tool_use"):
            web_calls += 1
        if t == "web_search_call" or "web_search" in str(t or ""):
            web_calls += 1
        for k in ("url", "link", "href"):
            if isinstance(o.get(k), str) and o[k].startswith("http"):
                urls.add(o[k])
        if isinstance(o.get("title"), str) and o.get("title").strip():
            titles.append(o["title"].strip()[:80])
        if isinstance(o.get("text"), str) and o["text"].strip():
            text_parts.append(o["text"][:200])
        # annotations / citations
        for ann in o.get("annotations") or []:
            if isinstance(ann, dict):
                u = ann.get("url") or ann.get("href")
                if isinstance(u, str) and u.startswith("http"):
                    urls.add(u)
        for v in o.values():
            walk(v, depth + 1)
    elif isinstance(o, list):
        for i in o:
            walk(i, depth + 1)

walk(data)
# also regex urls in raw
for m in re.findall(r"https?://[^\s\"'<>]+", raw):
    if "deepseek" not in m and len(m) < 200:
        urls.add(m.rstrip(").,]"))

lines.append(f"web_related_nodes≈{web_calls}")
lines.append(f"urls_found={len(urls)}")
for u in list(urls)[:12]:
    lines.append(f"  url: {u}")
for t in titles[:8]:
    lines.append(f"  title: {t}")
if text_parts:
    lines.append("text_sample=" + text_parts[0][:300].replace("\n", " "))
# error
if data.get("error"):
    lines.append("error=" + json.dumps(data["error"], ensure_ascii=False)[:400])
open(outp, "w").write("\n".join(lines))
print("\n".join(lines))
# save pretty truncated
open(path + ".pretty", "w").write(json.dumps(data, ensure_ascii=False, indent=2)[:12000])
PY

echo
# --- B: Anthropic path (current product) ---
cat >"$tmp/anth_body.json" <<EOF
{
  "model": "$RESP_MODEL",
  "max_tokens": 1024,
  "messages": [{"role": "user", "content": $(python3 -c 'import json,sys; print(json.dumps("Search the web and list sources with titles and URLs for: "+sys.argv[1])' "$QUERY")}],
  "tools": [{"type": "web_search_20250305", "name": "web_search", "max_uses": 3}]
}
EOF

ANTH="$BASE/anthropic/v1/messages"
echo "== B: POST $ANTH + web_search_20250305 =="
t0=$(date +%s%3N)
code=$(curl -sS -o "$tmp/anth_out.json" -w '%{http_code}' \
  -X POST "$ANTH" \
  -H "x-api-key: $KEY" \
  -H "anthropic-version: 2023-06-01" \
  -H "Content-Type: application/json" \
  --data-binary @"$tmp/anth_body.json" \
  --max-time 120 || echo "000")
t1=$(date +%s%3N)
ms_b=$((t1 - t0))
echo "http=$code elapsed_ms=$ms_b"
python3 - <<'PY' "$tmp/anth_out.json" "$code" "$ms_b" "$tmp/b_summary.txt"
import json, sys, re
path, code, ms, outp = sys.argv[1:5]
raw = open(path, encoding="utf-8", errors="replace").read()
lines = [f"http={code}", f"elapsed_ms={ms}", f"body_bytes={len(raw)}"]
try:
    data = json.loads(raw)
except Exception as e:
    lines.append(f"json_error={e}")
    open(outp, "w").write("\n".join(lines))
    print("\n".join(lines))
    raise SystemExit(0)

urls, titles, snippets = [], [], []
for block in data.get("content") or []:
    if not isinstance(block, dict):
        continue
    if block.get("type") == "web_search_tool_result":
        for item in block.get("content") or []:
            if not isinstance(item, dict):
                continue
            if item.get("type") != "web_search_result":
                continue
            u = (item.get("url") or "").strip()
            t = (item.get("title") or "").strip()
            if u:
                urls.append(u)
            if t:
                titles.append(t[:80])
            sn = item.get("snippet") or item.get("page_age") or ""
            if sn:
                snippets.append(str(sn)[:80])
            enc = item.get("encrypted_content")
            if enc:
                snippets.append(f"encrypted_content_len={len(str(enc))}")
    if block.get("type") == "text" and block.get("text"):
        lines.append("text_head=" + block["text"][:300].replace("\n", " "))

lines.append(f"web_results={len(urls)}")
for u in urls[:12]:
    lines.append(f"  url: {u}")
for t in titles[:8]:
    lines.append(f"  title: {t}")
if snippets:
    lines.append("snippet_notes=" + "; ".join(snippets[:5]))
if data.get("error"):
    lines.append("error=" + json.dumps(data["error"], ensure_ascii=False)[:400])
open(outp, "w").write("\n".join(lines))
print("\n".join(lines))
open(path + ".pretty", "w").write(json.dumps(data, ensure_ascii=False, indent=2)[:12000])
PY

echo
# --- C: Responses with web_search_2025_08_26 ---
cat >"$tmp/resp2_body.json" <<EOF
{
  "model": "$RESP_MODEL",
  "input": $(python3 -c 'import json,sys; print(json.dumps(sys.argv[1]))' "$QUERY"),
  "tools": [{"type": "web_search_2025_08_26"}],
  "max_output_tokens": 1024,
  "reasoning": {"effort": "none"}
}
EOF

echo "== C: POST $BASE/responses + tools web_search_2025_08_26 =="
t0=$(date +%s%3N)
code=$(curl -sS -o "$tmp/resp2_out.json" -w '%{http_code}' \
  -X POST "$BASE/responses" \
  -H "Authorization: Bearer $KEY" \
  -H "Content-Type: application/json" \
  --data-binary @"$tmp/resp2_body.json" \
  --max-time 120 || echo "000")
t1=$(date +%s%3N)
ms_c=$((t1 - t0))
echo "http=$code elapsed_ms=$ms_c"
python3 - <<'PY' "$tmp/resp2_out.json" "$code" "$ms_c" "$tmp/c_summary.txt"
import json, sys, re
path, code, ms, outp = sys.argv[1:5]
raw = open(path, encoding="utf-8", errors="replace").read()
lines = [f"http={code}", f"elapsed_ms={ms}", f"body_bytes={len(raw)}"]
try:
    data = json.loads(raw)
except Exception as e:
    lines.append(str(e))
    open(outp, "w").write("\n".join(lines)); print("\n".join(lines)); raise SystemExit(0)
urls = set(re.findall(r"https?://[^\s\"'<>]+", raw))
urls = {u for u in urls if "deepseek" not in u and len(u) < 200}
lines.append(f"urls_found={len(urls)}")
for u in list(urls)[:10]:
    lines.append(f"  url: {u}")
ot = data.get("output_text")
if isinstance(ot, str):
    lines.append(f"output_text_len={len(ot)}")
    lines.append("output_text_head=" + ot[:300].replace("\n", " "))
if data.get("error"):
    lines.append("error=" + json.dumps(data["error"], ensure_ascii=False)[:400])
open(outp, "w").write("\n".join(lines))
print("\n".join(lines))
PY

# Write report
{
  echo "# DeepSeek Responses vs Anthropic web_search probe"
  echo
  echo "- **time_utc**: $ts"
  echo "- **query**: \`$QUERY\`"
  echo "- **base**: \`$BASE\`"
  echo "- **model**: \`$RESP_MODEL\`"
  echo
  echo "## A — Responses + \`web_search\`"
  echo '```'
  cat "$tmp/a_summary.txt"
  echo '```'
  echo
  echo "## B — Anthropic + \`web_search_20250305\` (current product)"
  echo '```'
  cat "$tmp/b_summary.txt"
  echo '```'
  echo
  echo "## C — Responses + \`web_search_2025_08_26\`"
  echo '```'
  cat "$tmp/c_summary.txt"
  echo '```'
  echo
  echo "## Raw pretty (truncated) saved under probe tmp during run; latest report only summaries."
} | tee "$REPORT"

echo
echo "report: $REPORT"
# keep pretty bodies for inspection
cp -f "$tmp/resp_out.json.pretty" "$REPORT_DIR/deepseek-responses-A.pretty.json" 2>/dev/null || \
  cp -f "$tmp/resp_out.json" "$REPORT_DIR/deepseek-responses-A.raw.json" 2>/dev/null || true
cp -f "$tmp/anth_out.json.pretty" "$REPORT_DIR/deepseek-anthropic-B.pretty.json" 2>/dev/null || \
  cp -f "$tmp/anth_out.json" "$REPORT_DIR/deepseek-anthropic-B.raw.json" 2>/dev/null || true
echo "pretty dumps: $REPORT_DIR/deepseek-*-*.json"
