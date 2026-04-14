#!/usr/bin/env bash
set -euo pipefail

FRONTEND_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
ROOT_DIR="$(cd "${FRONTEND_DIR}/.." && pwd)"
BACKEND_DIR="${ROOT_DIR}/go-backend"
ENV_LOCAL="${FRONTEND_DIR}/.env.local"

FRONTEND_PORT="${FRONTEND_PORT:-3000}"
BACKEND_PORT="${BACKEND_PORT:-38080}"
BACKEND_URL="${BACKEND_URL:-http://127.0.0.1:${BACKEND_PORT}}"
NEXT_PUBLIC_API_URL="${NEXT_PUBLIC_API_URL:-${BACKEND_URL}}"
NEXT_PUBLIC_APP_URL="${NEXT_PUBLIC_APP_URL:-http://127.0.0.1:${FRONTEND_PORT}}"
PID_FILE="${FRONTEND_PID_FILE:-/tmp/contextosv5-frontend-${FRONTEND_PORT}.pid}"
LOG_FILE="${FRONTEND_LOG_FILE:-/tmp/contextosv5-frontend-${FRONTEND_PORT}.log}"
HEALTH_URL="${FRONTEND_HEALTH_URL:-http://127.0.0.1:${FRONTEND_PORT}}"
START_TIMEOUT_SEC="${FRONTEND_START_TIMEOUT_SEC:-45}"
NEXT_BIN="${FRONTEND_DIR}/node_modules/.bin/next"

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

  sleep 0.3
}

upsert_env() {
  local file="$1"
  local key="$2"
  local value="$3"
  local tmp
  tmp="$(mktemp "${file}.XXXXXX")"

  if [[ -f "${file}" ]]; then
    awk -v key="${key}" -v value="${value}" '
      BEGIN { done = 0 }
      index($0, key "=") == 1 {
        print key "=" value
        done = 1
        next
      }
      { print }
      END {
        if (!done) {
          print key "=" value
        }
      }
    ' "${file}" >"${tmp}"
  else
    printf '%s=%s\n' "${key}" "${value}" >"${tmp}"
  fi

  mv "${tmp}" "${file}"
}

if [[ ! -x "${NEXT_BIN}" ]]; then
  echo "missing next binary: ${NEXT_BIN}"
  echo "run: cd ${FRONTEND_DIR} && npm install"
  exit 1
fi

if [[ -f "${PID_FILE}" ]]; then
  old_pid="$(cat "${PID_FILE}" 2>/dev/null || true)"
  if [[ -n "${old_pid}" ]] && kill -0 "${old_pid}" 2>/dev/null; then
    kill "${old_pid}" 2>/dev/null || true
    sleep 0.3
  fi
  rm -f "${PID_FILE}" || true
fi

cleanup_repo_frontend_listeners

if is_port_listening "${FRONTEND_PORT}"; then
  echo "port ${FRONTEND_PORT} is already in use. stop the existing frontend first."
  echo "tip: run ${FRONTEND_DIR}/scripts/frontend-dev-down.sh"
  exit 1
fi

upsert_env "${ENV_LOCAL}" "NEXT_PUBLIC_API_URL" "${NEXT_PUBLIC_API_URL}"
upsert_env "${ENV_LOCAL}" "BACKEND_URL" "${BACKEND_URL}"
upsert_env "${ENV_LOCAL}" "NEXT_PUBLIC_APP_URL" "${NEXT_PUBLIC_APP_URL}"

mkdir -p "${ROOT_DIR}/.cache"

(
  cd "${FRONTEND_DIR}"
  XDG_CACHE_HOME="${ROOT_DIR}/.cache" \
  NEXT_TELEMETRY_DISABLED=1 \
  "${NEXT_BIN}" dev --hostname 127.0.0.1 --port "${FRONTEND_PORT}" >"${LOG_FILE}" 2>&1
) &

pid=$!
echo "${pid}" >"${PID_FILE}"

for ((i = 1; i <= START_TIMEOUT_SEC; i++)); do
  if ! kill -0 "${pid}" 2>/dev/null; then
    echo "frontend process exited before becoming healthy (pid=${pid})"
    tail -n 120 "${LOG_FILE}" || true
    exit 1
  fi
  code="$(curl --noproxy '*' -sS --connect-timeout 2 -m 2 -o /tmp/contextosv5-frontend-health.out -w '%{http_code}' "${HEALTH_URL}" || true)"
  if [[ "${code}" =~ ^(200|301|302|307|308)$ ]]; then
    echo "frontend up: url=${HEALTH_URL} pid=${pid} log=${LOG_FILE}"
    exit 0
  fi
  sleep 1
done

echo "frontend did not become healthy within ${START_TIMEOUT_SEC}s: ${HEALTH_URL}"
tail -n 120 "${LOG_FILE}" || true
exit 1
