#!/usr/bin/env bash
set -euo pipefail

FRONTEND_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
FRONTEND_PORT="${FRONTEND_PORT:-3000}"
PID_FILE="${FRONTEND_PID_FILE:-/tmp/contextosv5-frontend-${FRONTEND_PORT}.pid}"

is_port_listening() {
  local port="$1"
  ss -ltn | awk '{print $4}' | grep -Eq "(^|[\\[\\]:\\.])${port}$"
}

repo_next_listener_pids() {
  lsof -tiTCP:"${FRONTEND_PORT}" -sTCP:LISTEN -Pn 2>/dev/null || true
}

is_repo_frontend_pid() {
  local pid="$1"
  local cwd

  [[ -n "${pid}" ]] || return 1
  [[ -d "/proc/${pid}" ]] || return 1

  cwd="$(readlink -f "/proc/${pid}/cwd" 2>/dev/null || true)"
  [[ "${cwd}" == "${FRONTEND_DIR}" ]]
}

cleanup_repo_frontend_listeners() {
  local pid

  while read -r pid; do
    [[ -n "${pid}" ]] || continue
    if is_repo_frontend_pid "${pid}"; then
      kill "${pid}" 2>/dev/null || true
    fi
  done < <(repo_next_listener_pids)
}

stopped=0

if [[ -f "${PID_FILE}" ]]; then
  pid="$(cat "${PID_FILE}" 2>/dev/null || true)"
  if [[ -n "${pid}" ]] && kill -0 "${pid}" 2>/dev/null; then
    kill "${pid}" 2>/dev/null || true
    stopped=1
  fi
  rm -f "${PID_FILE}" || true
fi

# Clean up direct Next dev processes launched from this frontend path.
pkill -f "${FRONTEND_DIR}/node_modules/.bin/next dev --hostname 127.0.0.1 --port ${FRONTEND_PORT}" 2>/dev/null || true
cleanup_repo_frontend_listeners

sleep 0.3

if is_port_listening "${FRONTEND_PORT}"; then
  echo "port ${FRONTEND_PORT} is still in use after shutdown attempt"
  exit 1
fi

if [[ "${stopped}" == "1" ]]; then
  echo "frontend down: port=${FRONTEND_PORT}"
else
  echo "frontend already stopped: port=${FRONTEND_PORT}"
fi
