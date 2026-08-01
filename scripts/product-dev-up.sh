#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
AVRAG_DIR="${ROOT_DIR}/avrag-rs"
NEXT_DIR="${ROOT_DIR}/frontend_next"
SESSION="${CONTEXT_OS_DEV_SESSION:-context-os-dev}"
BASE_DIR="${HOME}/.local/share/avrag-dev"
MINIO_DATA_DIR="${BASE_DIR}/minio/data"
# Default: local avrag-rs/target (Cargo default). Override only if you intentionally
# use shared cache: CARGO_TARGET_DIR=$HOME/.cache/context-osv6/target/avrag-rs
CARGO_TARGET_DIR="${CARGO_TARGET_DIR:-${AVRAG_DIR}/target}"
# Cap rustc parallelism on WSL/low-RAM (override with CARGO_BUILD_JOBS=N).
export CARGO_BUILD_JOBS="${CARGO_BUILD_JOBS:-2}"
MILVUS_URL="${MILVUS_URL:-http://127.0.0.1:19530}"
MINIO_API_ADDR="${MINIO_API_ADDR:-127.0.0.1:9000}"
MINIO_CONSOLE_ADDR="${MINIO_CONSOLE_ADDR:-127.0.0.1:9001}"

# Load backend preference from avrag-rs/.env when present (do not export secrets here).
RETRIEVAL_BACKEND="${RETRIEVAL_BACKEND:-milvus}"
if [[ -f "${AVRAG_DIR}/.env" ]]; then
  # shellcheck disable=SC1091
  set -a
  # shellcheck source=/dev/null
  source "${AVRAG_DIR}/.env"
  set +a
  RETRIEVAL_BACKEND="${RETRIEVAL_BACKEND:-milvus}"
fi
RETRIEVAL_BACKEND="$(echo "${RETRIEVAL_BACKEND}" | tr '[:upper:]' '[:lower:]')"

if ! command -v tmux >/dev/null 2>&1; then
  echo "tmux is required for this dev stack script." >&2
  exit 1
fi

if tmux has-session -t "${SESSION}" 2>/dev/null; then
  echo "Context OS dev stack is already running in tmux session '${SESSION}'."
  echo "Attach with: tmux attach -t ${SESSION}"
  exit 0
fi

mkdir -p "${MINIO_DATA_DIR}"
DEV_LOG_DIR="${AVRAG_DIR}/.dev-logs"
mkdir -p "${DEV_LOG_DIR}"

echo "Starting PostgreSQL and Redis..."
sudo pg_ctlcluster 16 main start >/dev/null 2>&1 || true
pg_isready -h 127.0.0.1 -p 5432
sudo service redis-server start >/dev/null 2>&1 || true
redis-cli ping

echo "Ensuring avrag database..."
sudo -u postgres psql -tAc "SELECT 1 FROM pg_roles WHERE rolname='avrag'" | grep -q 1 || \
  sudo -u postgres psql -c "CREATE ROLE avrag LOGIN PASSWORD 'avrag';"
sudo -u postgres psql -tAc "SELECT 1 FROM pg_database WHERE datname='avrag_rs'" | grep -q 1 || \
  sudo -u postgres psql -c "CREATE DATABASE avrag_rs OWNER avrag;"

if [[ "${RETRIEVAL_BACKEND}" == "pgvector" || "${RETRIEVAL_BACKEND}" == "postgres" || "${RETRIEVAL_BACKEND}" == "pg" ]]; then
  echo "Retrieval backend: pgvector (Milvus not required for local RAG)."
  if ! sudo -u postgres psql -d avrag_rs -tAc "SELECT 1 FROM pg_extension WHERE extname='vector'" | grep -q 1; then
    echo "Ensuring PostgreSQL pgvector extension..."
    if ! sudo -u postgres psql -d avrag_rs -c "CREATE EXTENSION IF NOT EXISTS vector;" 2>/dev/null; then
      echo "WARN: CREATE EXTENSION vector failed. Install package e.g. postgresql-16-pgvector, then re-run." >&2
    fi
  fi
else
  echo "Retrieval backend: milvus (start stack separately if needed)."
  if ! curl -s --connect-timeout 1 "${MILVUS_URL%/}/v2/vectordb/collections/list" -X POST \
      -H 'Content-Type: application/json' -d '{}' >/dev/null 2>&1; then
    echo "WARN: Milvus not reachable at ${MILVUS_URL}."
    echo "  Start with: docker compose -f ${AVRAG_DIR}/docker-compose.milvus.yml up -d"
    echo "  Or switch local RAG: RETRIEVAL_BACKEND=pgvector in ${AVRAG_DIR}/.env"
  fi
fi

# pdf-visual-renderer / office-parser 已退役（markitdown 唯一生产解析器，2026-07-31 W2）：
# worker 只依赖 PATH 上的 markitdown CLI，不再起本地解析服务。

tmux new-session -d -s "${SESSION}" -n minio \
  "MINIO_ROOT_USER='${MINIO_ROOT_USER:-minioadmin}' MINIO_ROOT_PASSWORD='${MINIO_ROOT_PASSWORD:-minioadmin}' exec minio server '${MINIO_DATA_DIR}' --address '${MINIO_API_ADDR}' --console-address '${MINIO_CONSOLE_ADDR}'"

tmux new-window -t "${SESSION}" -n api \
  "cd '${AVRAG_DIR}' && set -a && source .env && set +a && export CARGO_TARGET_DIR='${CARGO_TARGET_DIR}' && exec cargo run -p avrag-api 2>&1 | tee -a '${DEV_LOG_DIR}/api.log'"

tmux new-window -t "${SESSION}" -n worker \
  "cd '${AVRAG_DIR}' && set -a && source .env && set +a && export CARGO_TARGET_DIR='${CARGO_TARGET_DIR}' && export RUST_LOG=\"\${RUST_LOG:-info,avrag_worker=info}\" && exec cargo run -p avrag-worker 2>&1 | tee -a '${DEV_LOG_DIR}/worker.log'"

tmux new-window -t "${SESSION}" -n next \
  "cd '${NEXT_DIR}' && exec pnpm dev"

tmux select-window -t "${SESSION}:next"

echo "Context OS dev stack is starting in tmux session '${SESSION}'."
echo "Attach with: tmux attach -t ${SESSION}"
echo
echo "URLs:"
echo "  frontend       http://127.0.0.1:3000"
echo "  api            http://127.0.0.1:8080"
if [[ "${RETRIEVAL_BACKEND}" == "pgvector" || "${RETRIEVAL_BACKEND}" == "postgres" || "${RETRIEVAL_BACKEND}" == "pg" ]]; then
  echo "  retrieval      pgvector (Postgres; no Milvus)"
else
  echo "  milvus         ${MILVUS_URL} (start separately if needed)"
fi
echo "  minio          http://127.0.0.1:9001"
echo "Logs:"
echo "  worker         ${DEV_LOG_DIR}/worker.log"
echo "  api            ${DEV_LOG_DIR}/api.log"
