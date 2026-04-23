#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
LOG_DIR="${PROJECT_ROOT}/.dev-logs"
PID_FILE="${LOG_DIR}/office-parser-jvm.pid"
LOG_FILE="${LOG_DIR}/office-parser-jvm.log"
BINARY="${PROJECT_ROOT}/target/debug/office-parser-jvm"
BIND="${OFFICE_PARSER_BIND:-127.0.0.1:9090}"
HEALTHZ_URL="http://${BIND}/v1/healthz"

mkdir -p "$LOG_DIR"

if [[ -f "$PID_FILE" ]]; then
  OLD_PID="$(cat "$PID_FILE" || true)"
  if [[ -n "${OLD_PID}" ]] && kill -0 "$OLD_PID" 2>/dev/null; then
    echo "office-parser-jvm already running (pid=$OLD_PID)"
    curl --noproxy 127.0.0.1 -fsS "$HEALTHZ_URL" || true
    exit 0
  fi
fi

if [[ ! -x "$BINARY" ]]; then
  echo "Building office-parser-jvm..."
  cargo build -p avrag-office-parser-jvm --bin office-parser-jvm --manifest-path "${PROJECT_ROOT}/Cargo.toml"
fi

echo "Starting office-parser-jvm on ${BIND}..."
OFFICE_PARSER_BIND="$BIND" nohup "$BINARY" >"$LOG_FILE" 2>&1 &
PID=$!
echo "$PID" >"$PID_FILE"

for _ in $(seq 1 30); do
  if curl --noproxy 127.0.0.1 -fsS "$HEALTHZ_URL" >/dev/null 2>&1; then
    echo "office-parser-jvm is ready (pid=$PID)"
    curl --noproxy 127.0.0.1 -fsS "$HEALTHZ_URL"
    echo
    exit 0
  fi
  sleep 1
done

echo "office-parser-jvm failed to become ready; check ${LOG_FILE}" >&2
exit 1
