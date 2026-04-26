#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
AVRAG_DIR="${ROOT_DIR}/avrag-rs"
NEXT_DIR="${ROOT_DIR}/frontend_next"
SESSION="${CONTEXT_OS_DEV_SESSION:-context-os-dev}"
BASE_DIR="${HOME}/.local/share/avrag-dev"
QDRANT_DIR="${BASE_DIR}/qdrant"
MINIO_DATA_DIR="${BASE_DIR}/minio/data"
CARGO_TARGET_DIR="${CARGO_TARGET_DIR:-${HOME}/.cache/context-osv6/target/avrag-rs}"
QDRANT_URI="${QDRANT_URI:-http://127.0.0.1:6333}"
MINIO_API_ADDR="${MINIO_API_ADDR:-127.0.0.1:9000}"
MINIO_CONSOLE_ADDR="${MINIO_CONSOLE_ADDR:-127.0.0.1:9001}"

if ! command -v tmux >/dev/null 2>&1; then
  echo "tmux is required for this dev stack script." >&2
  exit 1
fi

if tmux has-session -t "${SESSION}" 2>/dev/null; then
  echo "Context OS dev stack is already running in tmux session '${SESSION}'."
  echo "Attach with: tmux attach -t ${SESSION}"
  exit 0
fi

mkdir -p "${QDRANT_DIR}" "${MINIO_DATA_DIR}"

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

tmux new-session -d -s "${SESSION}" -n qdrant \
  "cd '${QDRANT_DIR}' && exec qdrant --disable-telemetry --uri '${QDRANT_URI}'"

tmux new-window -t "${SESSION}" -n minio \
  "MINIO_ROOT_USER='${MINIO_ROOT_USER:-minioadmin}' MINIO_ROOT_PASSWORD='${MINIO_ROOT_PASSWORD:-minioadmin}' exec minio server '${MINIO_DATA_DIR}' --address '${MINIO_API_ADDR}' --console-address '${MINIO_CONSOLE_ADDR}'"

tmux new-window -t "${SESSION}" -n office \
  "cd '${AVRAG_DIR}' && set -a && source .env && set +a && export CARGO_TARGET_DIR='${CARGO_TARGET_DIR}' && OFFICE_PARSER_BIND=127.0.0.1:9090 exec cargo run -p avrag-office-parser-jvm --bin office-parser-jvm"

tmux new-window -t "${SESSION}" -n api \
  "cd '${AVRAG_DIR}' && set -a && source .env && set +a && export CARGO_TARGET_DIR='${CARGO_TARGET_DIR}' && exec cargo run -p avrag-api"

tmux new-window -t "${SESSION}" -n worker \
  "cd '${AVRAG_DIR}' && set -a && source .env && set +a && export CARGO_TARGET_DIR='${CARGO_TARGET_DIR}' && exec cargo run -p avrag-worker"

tmux new-window -t "${SESSION}" -n next \
  "cd '${NEXT_DIR}' && exec pnpm dev"

tmux select-window -t "${SESSION}:next"

echo "Context OS dev stack is starting in tmux session '${SESSION}'."
echo "Attach with: tmux attach -t ${SESSION}"
echo
echo "URLs:"
echo "  frontend       http://127.0.0.1:3000"
echo "  api            http://127.0.0.1:8080"
echo "  office parser  http://127.0.0.1:9090/v1/healthz"
echo "  qdrant         http://127.0.0.1:6333"
echo "  minio          http://127.0.0.1:9001"
