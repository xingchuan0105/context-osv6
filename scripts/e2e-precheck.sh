#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
AVRAG_RS="$ROOT/avrag-rs"
COMPOSE_FILE="$AVRAG_RS/docker-compose.milvus.yml"

echo "[e2e-precheck] starting Milvus compose stack..."
docker compose -f "$COMPOSE_FILE" up -d

echo "[e2e-precheck] waiting for Milvus API on 19530..."
deadline=$((SECONDS + 90))
while [ "$SECONDS" -lt "$deadline" ]; do
  code="$(curl -s -o /dev/null -w '%{http_code}' -X POST http://127.0.0.1:19530/v2/vectordb/collections/list \
    -H 'Content-Type: application/json' -d '{"dbName":"default"}' || true)"
  if [ "$code" = "200" ]; then
    echo "[e2e-precheck] Milvus ready (HTTP 200)"
    break
  fi
  if ! docker inspect -f '{{.State.Running}}' milvus-standalone 2>/dev/null | grep -q true; then
    echo "[e2e-precheck] milvus-standalone is not running; recent logs:" >&2
    docker logs --tail 40 milvus-standalone >&2 || true
    exit 1
  fi
  sleep 1
done

if [ "$SECONDS" -ge "$deadline" ]; then
  echo "[e2e-precheck] Milvus did not become ready in 90s" >&2
  exit 1
fi

echo "[e2e-precheck] OK"
