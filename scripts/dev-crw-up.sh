#!/usr/bin/env bash
# Start local CRW (https://github.com/us/crw) for web auto-scrape / client.fetch.
# No cloud API key required. Default: http://127.0.0.1:3000
set -euo pipefail

NAME="${CRW_DOCKER_NAME:-crw}"
IMAGE="${CRW_DOCKER_IMAGE:-ghcr.io/us/crw}"
# Host port 3100 — product Next.js already binds :3000.
PORT="${CRW_PORT:-3100}"
CONTAINER_PORT=3000

if ! command -v docker >/dev/null 2>&1; then
  echo "docker is required to run CRW locally" >&2
  exit 1
fi

if docker ps --format '{{.Names}}' | grep -qx "$NAME"; then
  echo "CRW already running: container=$NAME → http://127.0.0.1:$PORT"
  exit 0
fi

if docker ps -a --format '{{.Names}}' | grep -qx "$NAME"; then
  echo "Removing failed/stopped container $NAME to rebind ports…"
  docker rm -f "$NAME" >/dev/null 2>&1 || true
fi

echo "Pulling/running $IMAGE as $NAME on 127.0.0.1:$PORT (container :$CONTAINER_PORT)…"
docker run -d --name "$NAME" -p "127.0.0.1:${PORT}:${CONTAINER_PORT}" "$IMAGE" >/dev/null

echo "Waiting for CRW /v1/scrape on :$PORT…"
for _ in $(seq 1 45); do
  if curl -sf -X POST "http://127.0.0.1:${PORT}/v1/scrape" \
       -H 'Content-Type: application/json' \
       -d '{"url":"https://example.com","formats":["markdown"]}' >/dev/null 2>&1; then
    echo "CRW ready: CRW_BASE_URL=http://127.0.0.1:${PORT}"
    echo "  WEB_AUTO_SCRAPE=1  (see avrag-rs/.env)"
    exit 0
  fi
  sleep 1
done

echo "CRW container started but /v1/scrape not ready yet; check: docker logs $NAME" >&2
exit 1
