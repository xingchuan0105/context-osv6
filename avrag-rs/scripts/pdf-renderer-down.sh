#!/usr/bin/env bash
set -euo pipefail

PROJECT_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
PID_FILE="${PROJECT_ROOT}/.dev-logs/pdf-visual-renderer.pid"

if [[ ! -f "${PID_FILE}" ]]; then
  echo "pdf-visual-renderer pid file not found"
  exit 0
fi

PID="$(cat "${PID_FILE}")"
if kill -0 "${PID}" 2>/dev/null; then
  kill "${PID}"
  echo "Stopped pdf-visual-renderer (pid=${PID})"
else
  echo "pdf-visual-renderer is not running"
fi
rm -f "${PID_FILE}"
