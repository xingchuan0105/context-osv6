#!/usr/bin/env bash
set -euo pipefail

BASE_DIR="${HOME}/.local/share/avrag-dev"
LOG_DIR="${BASE_DIR}/logs"
QDRANT_DIR="${BASE_DIR}/qdrant"
MINIO_DIR="${BASE_DIR}/minio"
MINIO_DATA_DIR="${MINIO_DIR}/data"
MINIO_CONSOLE_ADDR="${MINIO_CONSOLE_ADDR:-127.0.0.1:9001}"
MINIO_API_ADDR="${MINIO_API_ADDR:-127.0.0.1:9000}"
QDRANT_URI="${QDRANT_URI:-http://127.0.0.1:6333}"

mkdir -p "${LOG_DIR}" "${QDRANT_DIR}" "${MINIO_DATA_DIR}"

echo "Starting PostgreSQL..."
sudo pg_ctlcluster 16 main start >/dev/null 2>&1 || true
pg_isready -h 127.0.0.1 -p 5432

echo "Starting Redis..."
sudo service redis-server start >/dev/null 2>&1 || true
redis-cli ping

echo "Ensuring avrag database..."
sudo -u postgres psql -tAc "SELECT 1 FROM pg_roles WHERE rolname='avrag'" | grep -q 1 || \
  sudo -u postgres psql -c "CREATE ROLE avrag LOGIN PASSWORD 'avrag';"
sudo -u postgres psql -tAc "SELECT 1 FROM pg_database WHERE datname='avrag_rs'" | grep -q 1 || \
  sudo -u postgres psql -c "CREATE DATABASE avrag_rs OWNER avrag;"

if ! pgrep -af "^qdrant .*${QDRANT_DIR}" >/dev/null 2>&1; then
  echo "Starting Qdrant..."
  nohup bash -lc "cd '${QDRANT_DIR}' && exec qdrant --disable-telemetry --uri '${QDRANT_URI}'" \
    >"${LOG_DIR}/qdrant.log" 2>&1 &
  echo $! > "${QDRANT_DIR}/qdrant.pid"
fi
for _ in $(seq 1 20); do
  if curl -fsS "${QDRANT_URI}/collections" >/dev/null 2>&1; then
    break
  fi
  sleep 1
done
curl -fsS "${QDRANT_URI}/collections" >/dev/null

if ! pgrep -af "^minio server ${MINIO_DATA_DIR}" >/dev/null 2>&1; then
  echo "Starting MinIO..."
  MINIO_ROOT_USER="${MINIO_ROOT_USER:-minioadmin}" \
  MINIO_ROOT_PASSWORD="${MINIO_ROOT_PASSWORD:-minioadmin}" \
  nohup minio server "${MINIO_DATA_DIR}" \
    --address "${MINIO_API_ADDR}" \
    --console-address "${MINIO_CONSOLE_ADDR}" \
    >"${LOG_DIR}/minio.log" 2>&1 &
  echo $! > "${MINIO_DIR}/minio.pid"
fi
for _ in $(seq 1 20); do
  if curl -fsS "http://${MINIO_API_ADDR}/minio/health/live" >/dev/null 2>&1; then
    break
  fi
  sleep 1
done
curl -fsS "http://${MINIO_API_ADDR}/minio/health/live" >/dev/null

echo
echo "Dev services are ready:"
echo "  PostgreSQL : postgres://avrag:avrag@127.0.0.1:5432/avrag_rs"
echo "  Redis      : redis://127.0.0.1:6379"
echo "  Qdrant     : ${QDRANT_URI}"
echo "  MinIO API  : http://${MINIO_API_ADDR}"
echo "  MinIO UI   : http://${MINIO_CONSOLE_ADDR}"
