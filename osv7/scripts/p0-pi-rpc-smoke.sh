#!/usr/bin/env bash
# P0: pi RPC process model smoke (no LLM turn).
set -euo pipefail
SESSION_DIR="${1:-/tmp/pi-p0-sessions}"
mkdir -p "$SESSION_DIR"

PI_BIN="${PI_BIN:-pi}"
if ! command -v "$PI_BIN" >/dev/null 2>&1; then
  echo "pi not found" >&2
  exit 1
fi

echo "==> pi version: $($PI_BIN --version)"

# Session A
OUT_A=$(mktemp)
(
  printf '%s\n' '{"id":"a1","type":"get_state"}'
  sleep 0.2
  printf '%s\n' '{"id":"a2","type":"set_session_name","name":"p0-session-a"}'
  sleep 0.2
  printf '%s\n' '{"id":"a3","type":"get_state"}'
  sleep 0.2
) | "$PI_BIN" --mode rpc --session-dir "$SESSION_DIR" --no-extensions 2>/dev/null | tee "$OUT_A" | head -20

SESSION_FILE=$(python3 - <<'PY' "$OUT_A"
import json,sys
path=sys.argv[1]
sf=None
for line in open(path):
    line=line.strip()
    if not line: continue
    o=json.loads(line)
    if o.get("type")=="response" and o.get("command")=="get_state" and o.get("success"):
        d=o.get("data") or {}
        if d.get("sessionFile"):
            sf=d["sessionFile"]
        elif d.get("sessionId"):
            print(d.get("sessionId",""), end="")
            # sessionFile may be omitted with some flags
print(sf or "", end="")
PY
)

echo "==> session dir: $SESSION_DIR"
echo "==> last get_state lines:"
grep '"command":"get_state"' "$OUT_A" | tail -2

# Concurrent processes: two pi RPC at once (process model evidence)
OUT_B=$(mktemp)
OUT_C=$(mktemp)
(
  printf '%s\n' '{"id":"b1","type":"get_state"}'
  sleep 0.4
) | "$PI_BIN" --mode rpc --session-dir "$SESSION_DIR" --no-extensions --name p0-b 2>/dev/null >"$OUT_B" &
PID_B=$!
(
  printf '%s\n' '{"id":"c1","type":"get_state"}'
  sleep 0.4
) | "$PI_BIN" --mode rpc --session-dir "$SESSION_DIR" --no-extensions --name p0-c 2>/dev/null >"$OUT_C" &
PID_C=$!
wait $PID_B $PID_C || true

SID_B=$(grep -o '"sessionId":"[^"]*"' "$OUT_B" | head -1 || true)
SID_C=$(grep -o '"sessionId":"[^"]*"' "$OUT_C" | head -1 || true)
echo "==> concurrent RPC sessionId B: $SID_B"
echo "==> concurrent RPC sessionId C: $SID_C"
echo "==> pi RSS sample (any pi process):"
ps -o pid,rss,cmd -C node 2>/dev/null | grep -i '[p]i' | head -5 || ps aux | grep -E '[p]i --mode' | head -5 || true

echo "==> session files in dir:"
ls -la "$SESSION_DIR" 2>/dev/null | head -15 || true
echo "==> OK (rpc smoke)"
