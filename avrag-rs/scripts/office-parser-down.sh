#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
PID_FILE="${PROJECT_ROOT}/.dev-logs/office-parser-jvm.pid"

if [[ ! -f "$PID_FILE" ]]; then
  echo "office-parser-jvm pid file not found"
  exit 0
fi

PID="$(cat "$PID_FILE" || true)"
if [[ -n "${PID}" ]] && kill -0 "$PID" 2>/dev/null; then
  kill "$PID" || true
  echo "Stopped office-parser-jvm (pid=$PID)"
else
  echo "office-parser-jvm is not running"
fi

rm -f "$PID_FILE"
