#!/usr/bin/env bash
# Start/stop/status for Context-OS Client local data plane (Postgres + Redis + full Milvus).
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
COMPOSE_FILE="$ROOT/desktop/runtime/docker-compose.client.yml"
DATA_DIR="$ROOT/desktop/runtime/data"

die() { echo "desktop-local-stack: $*" >&2; exit 1; }
log() { echo "desktop-local-stack: $*"; }

command -v docker >/dev/null 2>&1 || die "docker not found"
docker compose version >/dev/null 2>&1 || die "docker compose plugin required"

mkdir -p "$DATA_DIR"/{etcd,minio,milvus,pg,redis}

cd "$ROOT/desktop/runtime"
export COMPOSE_PROJECT_NAME=context-os-client

cmd="${1:-status}"

case "$cmd" in
  up)
    log "starting Postgres + Redis + Milvus stack…"
    docker compose -f docker-compose.client.yml up -d
    log "waiting for ports…"
    sleep 3
    bash "$0" status
    ;;
  down)
    docker compose -f docker-compose.client.yml down
    log "stopped"
    ;;
  status)
    docker compose -f docker-compose.client.yml ps
    echo
    for spec in "5433:postgres" "6380:redis" "19530:milvus"; do
      port="${spec%%:*}"
      name="${spec##*:}"
      if (echo >/dev/tcp/127.0.0.1/"$port") >/dev/null 2>&1; then
        echo "  $name :$port  OK"
      else
        # fallback nc
        if command -v nc >/dev/null 2>&1 && nc -z 127.0.0.1 "$port" 2>/dev/null; then
          echo "  $name :$port  OK"
        else
          echo "  $name :$port  DOWN"
        fi
      fi
    done
    echo
    echo "Env for client process (optional):"
    echo "  CLIENT_PG_PORT=5433 CLIENT_REDIS_PORT=6380 CLIENT_MILVUS_PORT=19530"
    echo "  DATABASE_URL=postgres://avrag:avrag@127.0.0.1:5433/avrag_client"
    echo "  REDIS_URL=redis://127.0.0.1:6380/0"
    echo "  MILVUS_URL=http://127.0.0.1:19530"
    ;;
  logs)
    docker compose -f docker-compose.client.yml logs -f --tail=100
    ;;
  *)
    die "usage: $0 {up|down|status|logs}"
    ;;
esac
