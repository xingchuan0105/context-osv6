#!/usr/bin/env bash
set -euo pipefail

BASE_DIR="${HOME}/.local/share/avrag-dev"
LOG_DIR="${BASE_DIR}/logs"
MINIO_DIR="${BASE_DIR}/minio"
MINIO_DATA_DIR="${MINIO_DIR}/data"
MINIO_CONSOLE_ADDR="${MINIO_CONSOLE_ADDR:-127.0.0.1:9001}"
MINIO_API_ADDR="${MINIO_API_ADDR:-127.0.0.1:9000}"
MILVUS_URL="${MILVUS_URL:-http://127.0.0.1:19530}"

mkdir -p "${LOG_DIR}" "${MINIO_DATA_DIR}"

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
echo "Core dev services are ready:"
echo "  PostgreSQL : postgres://avrag:avrag@127.0.0.1:5432/avrag_rs"
echo "  Redis      : redis://127.0.0.1:6379"
echo "  MinIO API  : http://${MINIO_API_ADDR}"
echo "  MinIO UI   : http://${MINIO_CONSOLE_ADDR}"
echo "  Milvus     : ${MILVUS_URL} (start separately)"
