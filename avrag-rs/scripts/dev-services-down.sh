#!/usr/bin/env bash
set -euo pipefail

BASE_DIR="${HOME}/.local/share/avrag-dev"

if [ -f "${BASE_DIR}/minio/minio.pid" ]; then
  kill "$(cat "${BASE_DIR}/minio/minio.pid")" >/dev/null 2>&1 || true
  rm -f "${BASE_DIR}/minio/minio.pid"
fi

sudo service redis-server stop >/dev/null 2>&1 || true
sudo pg_ctlcluster 16 main stop >/dev/null 2>&1 || true

echo "Dev services stopped."
