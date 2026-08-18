#!/usr/bin/env bash
# Recreate avrag-api + avrag-worker on the VPS (host network + avrag-runtime:24.04).
# Intended to run ON the VPS as root (or via deploy-backend.sh remote step).
# Secrets: only /etc/avrag-rs/avrag.env — never commit that file.
set -euo pipefail

RUNTIME_IMAGE="${AVRAG_RUNTIME_IMAGE:-avrag-runtime:24.04}"
ENV_FILE="${AVRAG_ENV_FILE:-/etc/avrag-rs/avrag.env}"
OPT_ROOT="${AVRAG_OPT_ROOT:-/opt/avrag-rs}"
OBJ_ROOT="${AVRAG_OBJECT_ROOT:-/data/avrag/objects}"

die() { echo "run-avrag-containers: $*" >&2; exit 1; }

[[ -f "$ENV_FILE" ]] || die "missing env file: $ENV_FILE"
[[ -x "$OPT_ROOT/bin/avrag-api" ]] || die "missing $OPT_ROOT/bin/avrag-api"
[[ -x "$OPT_ROOT/bin/avrag-worker" ]] || die "missing $OPT_ROOT/bin/avrag-worker"
docker image inspect "$RUNTIME_IMAGE" >/dev/null 2>&1 || die "missing image $RUNTIME_IMAGE"

mkdir -p "$OBJ_ROOT"

run_one() {
  local name="$1" bin="$2"
  docker rm -f "$name" >/dev/null 2>&1 || true
  docker run -d \
    --name "$name" \
    --network host \
    --restart unless-stopped \
    --ulimit nofile=65536:65536 \
    --env-file "$ENV_FILE" \
    -v "${OPT_ROOT}:${OPT_ROOT}:ro" \
    -v "${OBJ_ROOT}:${OBJ_ROOT}" \
    -v "${ENV_FILE}:${ENV_FILE}:ro" \
    "$RUNTIME_IMAGE" \
    "${OPT_ROOT}/bin/${bin}"
  echo "run-avrag-containers: started $name"
}

run_one avrag-api avrag-api

# Worker replicas: ingestion/cleanup queues are claim-based (SKIP LOCKED) and
# replica-safe; periodic side jobs (billing/usage-export/retention/analytics)
# are not — they stay on replica 1 only (AVRAG_WORKER_SIDE_JOBS=0 elsewhere).
WORKER_REPLICAS="${AVRAG_WORKER_REPLICAS:-1}"
# Remove stale replicas left over from a previous larger replica count.
for stale in $(docker ps -a --format '{{.Names}}' | grep -E '^avrag-worker-[0-9]+$'); do
  idx="${stale##*-}"
  if [[ "$idx" -gt "$WORKER_REPLICAS" ]]; then
    docker rm -f "$stale" >/dev/null 2>&1 || true
    echo "run-avrag-containers: removed stale replica $stale"
  fi
done
for i in $(seq 1 "$WORKER_REPLICAS"); do
  wname="avrag-worker"
  extra_env=()
  if [[ "$i" -gt 1 ]]; then
    wname="avrag-worker-$i"
    extra_env=(-e AVRAG_WORKER_SIDE_JOBS=0)
  fi
  docker rm -f "$wname" >/dev/null 2>&1 || true
  docker run -d \
    --name "$wname" \
    --network host \
    --restart unless-stopped \
    --ulimit nofile=65536:65536 \
    --env-file "$ENV_FILE" \
    "${extra_env[@]}" \
    -e AVRAG_WORKER_ID="${wname}" \
    -v "${OPT_ROOT}:${OPT_ROOT}:ro" \
    -v "${OBJ_ROOT}:${OBJ_ROOT}" \
    -v "${ENV_FILE}:${ENV_FILE}:ro" \
    "$RUNTIME_IMAGE" \
    "${OPT_ROOT}/bin/avrag-worker"
  echo "run-avrag-containers: started $wname"
done

sleep 2
docker ps --filter name=avrag-api --filter name=avrag-worker --format 'table {{.Names}}\t{{.Status}}\t{{.Image}}'
docker ps --filter "name=avrag-worker-" --format 'table {{.Names}}\t{{.Status}}'
curl -sS -m 8 -o /dev/null -w "api_health:%{http_code}\n" http://127.0.0.1:8081/health || true
