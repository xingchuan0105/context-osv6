#!/usr/bin/env bash
# Start a local Milvus stack for E2E/CI and gate until it accepts vector DB
# requests on port 19530.
#
# Why a dedicated script: the Playwright webServer (avrag-api) connects to
# Milvus on startup, and journey/skills specs exercise upload -> RAG. Bringing
# Milvus up before `playwright test` removes the "Milvus not ready" flake that
# `docker compose up -d --wait` alone does not cover (the standalone service in
# docker-compose.milvus.yml has no healthcheck, so --wait returns as soon as
# etcd/minio are healthy, not when the 19530 REST API is up).
#
# Callable from any workflow working-directory: the compose file is resolved
# relative to this script's location (repo root -> avrag-rs/docker-compose.milvus.yml).
#
# Used by:
#   .github/workflows/frontend-journey.yml
#   .github/workflows/frontend-skills.yml
#   .github/workflows/frontend-smoke.yml
#   .github/workflows/nightly-llm-real.yml
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
COMPOSE_FILE="$REPO_ROOT/avrag-rs/docker-compose.milvus.yml"

if [ ! -f "$COMPOSE_FILE" ]; then
  echo "ci-start-milvus: docker-compose.milvus.yml not found at $COMPOSE_FILE" >&2
  exit 1
fi

echo "ci-start-milvus: bringing up Milvus stack via $COMPOSE_FILE"
docker compose -f "$COMPOSE_FILE" up -d --wait

MILVUS_HOST="${MILVUS_HOST:-127.0.0.1}"
MILVUS_PORT="${MILVUS_PORT:-19530}"
HEALTH_URL="http://${MILVUS_HOST}:${MILVUS_PORT}/v2/vectordb/collections/list"

echo "ci-start-milvus: probing $HEALTH_URL"
for _ in $(seq 1 30); do
  if curl -sf -X POST "$HEALTH_URL" \
        -H 'Content-Type: application/json' \
        -d '{"dbName":"default"}' >/dev/null 2>&1; then
    echo "ci-start-milvus: Milvus is ready"
    exit 0
  fi
  sleep 2
done

echo "ci-start-milvus: Milvus failed to become ready at $HEALTH_URL" >&2
docker compose -f "$COMPOSE_FILE" ps || true
exit 1
