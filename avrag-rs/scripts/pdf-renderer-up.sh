#!/usr/bin/env bash
set -euo pipefail

PROJECT_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
LOG_DIR="${PROJECT_ROOT}/.dev-logs"
PID_FILE="${LOG_DIR}/pdf-visual-renderer.pid"
LOG_FILE="${LOG_DIR}/pdf-visual-renderer.log"
SERVICE_DIR="${PROJECT_ROOT}/services/pdf-visual-renderer"
BIND="${PDF_RENDERER_BIND:-127.0.0.1:9091}"
VENV="${SERVICE_DIR}/.venv"

mkdir -p "${LOG_DIR}"

HOST="${BIND%:*}"
PORT="${BIND##*:}"
HEALTHZ_URL="http://${HOST}:${PORT}/v1/healthz"

is_pdf_renderer_healthy() {
  curl -fsS "${HEALTHZ_URL}" 2>/dev/null | grep -q '"service"[[:space:]]*:[[:space:]]*"pdf-visual-renderer"'
}

detect_port_conflict() {
  if ! curl -fsS "http://${HOST}:${PORT}/" >/dev/null 2>&1; then
    return 1
  fi
  if is_pdf_renderer_healthy; then
    return 1
  fi
  echo "Port ${BIND} is already in use by another service (not pdf-visual-renderer)." >&2
  if curl -fsS "http://${HOST}:${PORT}/api/v1/health" 2>/dev/null | grep -q '"status"[[:space:]]*:[[:space:]]*"ok"'; then
    echo "Detected Milvus metrics/health on ${PORT}. It should be remapped to 19091 in docker-compose.milvus.yml." >&2
    echo "Fix: cd avrag-rs && docker compose -f docker-compose.milvus.yml up -d standalone" >&2
  fi
  echo "Either stop the conflicting service or set PDF_RENDERER_BIND=127.0.0.1:<free-port> and PDF_RENDERER_BASE_URL to match." >&2
  return 0
}

if is_pdf_renderer_healthy; then
  echo "pdf-visual-renderer already healthy at ${BIND}"
  exit 0
fi

if detect_port_conflict; then
  exit 1
fi

if [[ -f "${PID_FILE}" ]]; then
  OLD_PID="$(cat "${PID_FILE}")"
  if kill -0 "${OLD_PID}" 2>/dev/null; then
    if is_pdf_renderer_healthy; then
      echo "pdf-visual-renderer already running (pid=${OLD_PID})"
      exit 0
    fi
    echo "Stale pdf-visual-renderer pid=${OLD_PID}; stopping before restart"
    kill "${OLD_PID}" 2>/dev/null || true
    rm -f "${PID_FILE}"
  fi
fi

if [[ ! -d "${VENV}" ]]; then
  echo "Creating venv at ${VENV}..."
  python3 -m venv "${VENV}"
  "${VENV}/bin/pip" install -q -r "${SERVICE_DIR}/requirements.txt"
fi

echo "Starting pdf-visual-renderer on ${BIND}..."
PDF_RENDERER_BIND="${BIND}" nohup "${VENV}/bin/python" "${SERVICE_DIR}/app.py" >"${LOG_FILE}" 2>&1 &
PID=$!
echo "${PID}" > "${PID_FILE}"

for _ in $(seq 1 30); do
  if is_pdf_renderer_healthy; then
    echo "pdf-visual-renderer is ready (pid=${PID})"
    curl -fsS "${HEALTHZ_URL}"
    echo
    exit 0
  fi
  sleep 0.5
done

echo "pdf-visual-renderer failed to become ready; check ${LOG_FILE}" >&2
if detect_port_conflict; then
  :
fi
exit 1
