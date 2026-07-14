#!/usr/bin/env bash
# Start/stop/status for Context-OS Client product processes (avrag-api + avrag-worker)
# against the local data plane (desktop/runtime/client.env).
#
# API listens on 127.0.0.1:18080 by default (offset from product-dev 8080).
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
AVRAG_DIR="$ROOT/avrag-rs"
RUNTIME_DIR="$ROOT/desktop/runtime"
ENV_FILE="$RUNTIME_DIR/client.env"
RUN_DIR="$RUNTIME_DIR/run"
LOG_DIR="$RUNTIME_DIR/logs"
OBJECTS_DIR="$RUNTIME_DIR/objects"
PID_API="$RUN_DIR/api.pid"
PID_WORKER="$RUN_DIR/worker.pid"
STACK_SCRIPT="$ROOT/scripts/desktop-local-stack.sh"

CLIENT_API_HOST="${CLIENT_API_HOST:-127.0.0.1}"
CLIENT_API_PORT="${CLIENT_API_PORT:-18080}"
AVRAG_API_ADDR="${AVRAG_API_ADDR:-${CLIENT_API_HOST}:${CLIENT_API_PORT}}"
AVRAG_PUBLIC_BASE_URL="${AVRAG_PUBLIC_BASE_URL:-http://${CLIENT_API_HOST}:${CLIENT_API_PORT}}"
AVRAG_OBJECT_ROOT="${AVRAG_OBJECT_ROOT:-$OBJECTS_DIR}"
MILVUS_COLLECTION_PREFIX="${MILVUS_COLLECTION_PREFIX:-avrag_client}"

die() { echo "desktop-local-product: $*" >&2; exit 1; }
log() { echo "desktop-local-product: $*"; }

mkdir -p "$RUN_DIR" "$LOG_DIR" "$OBJECTS_DIR"

port_open() {
  local host="$1" port="$2"
  if (echo >/dev/tcp/"$host"/"$port") >/dev/null 2>&1; then
    return 0
  fi
  if command -v nc >/dev/null 2>&1 && nc -z "$host" "$port" 2>/dev/null; then
    return 0
  fi
  return 1
}

pid_alive() {
  local pidfile="$1"
  [[ -f "$pidfile" ]] || return 1
  local pid
  pid="$(cat "$pidfile" 2>/dev/null || true)"
  [[ -n "${pid:-}" ]] || return 1
  kill -0 "$pid" 2>/dev/null
}

stop_pidfile() {
  local name="$1" pidfile="$2"
  if ! [[ -f "$pidfile" ]]; then
    log "$name: no pid file"
    return 0
  fi
  local pid
  pid="$(cat "$pidfile" 2>/dev/null || true)"
  if [[ -n "${pid:-}" ]] && kill -0 "$pid" 2>/dev/null; then
    log "stopping $name pid=$pid"
    kill "$pid" 2>/dev/null || true
    for _ in $(seq 1 20); do
      kill -0 "$pid" 2>/dev/null || break
      sleep 0.25
    done
    if kill -0 "$pid" 2>/dev/null; then
      log "force kill $name pid=$pid"
      kill -9 "$pid" 2>/dev/null || true
    fi
  fi
  rm -f "$pidfile"
}

find_bin() {
  local name="$1"
  # Prefer staged client layout (packaged or monorepo desktop/runtime/bin), then cargo targets.
  local client_home="${CONTEXT_OS_CLIENT_HOME:-$RUNTIME_DIR}"
  local candidates=(
    "$client_home/bin/$name"
    "$client_home/bin/${name}.exe"
    "$RUNTIME_DIR/bin/$name"
    "$RUNTIME_DIR/bin/${name}.exe"
    "$ROOT/desktop/src-tauri/binaries/${name}-$(rustc -vV 2>/dev/null | awk '/^host:/{print $2}')"
    "$ROOT/desktop/src-tauri/binaries/${name}-$(rustc -vV 2>/dev/null | awk '/^host:/{print $2}').exe"
    "$AVRAG_DIR/target/release/$name"
    "$AVRAG_DIR/target/release/${name}.exe"
    "$AVRAG_DIR/target/debug/$name"
    "$AVRAG_DIR/target/debug/${name}.exe"
    "${CARGO_TARGET_DIR:-}/release/$name"
    "${CARGO_TARGET_DIR:-}/debug/$name"
    "$HOME/.cache/context-osv6/target/avrag-rs/release/$name"
    "$HOME/.cache/context-osv6/target/avrag-rs/debug/$name"
  )
  local p
  for p in "${candidates[@]}"; do
    [[ -n "$p" && ( -x "$p" || -f "$p" ) ]] || continue
    # Prefer executable bit when present; Windows .exe may lack +x on NTFS mounts.
    if [[ -x "$p" || "$p" == *.exe ]]; then
      echo "$p"
      return 0
    fi
  done
  return 1
}

ensure_bins() {
  local api_bin worker_bin
  if api_bin="$(find_bin avrag-api)" && worker_bin="$(find_bin avrag-worker)"; then
    echo "$api_bin|$worker_bin"
    return 0
  fi
  log "binaries missing — building avrag-api + avrag-worker (release, jobs=${CARGO_BUILD_JOBS:-2})…"
  (
    cd "$AVRAG_DIR"
    export CARGO_BUILD_JOBS="${CARGO_BUILD_JOBS:-2}"
    cargo build --release -p avrag-api -p avrag-worker
  )
  api_bin="$(find_bin avrag-api)" || die "avrag-api binary not found after build"
  worker_bin="$(find_bin avrag-worker)" || die "avrag-worker binary not found after build"
  echo "$api_bin|$worker_bin"
}

load_env() {
  # LLM / embedding keys from monorepo .env if present, then overlay client data plane.
  if [[ -f "$AVRAG_DIR/.env" ]]; then
    set -a
    # shellcheck disable=SC1090
    source "$AVRAG_DIR/.env"
    set +a
  fi
  [[ -f "$ENV_FILE" ]] || die "missing $ENV_FILE — run: bash scripts/desktop-local-stack.sh ensure"
  set -a
  # shellcheck disable=SC1090
  source "$ENV_FILE"
  set +a

  # Force client product bind + isolation (wins over any monorepo .env).
  export AVRAG_API_ADDR
  export AVRAG_PUBLIC_BASE_URL
  export AVRAG_OBJECT_ROOT
  export MILVUS_COLLECTION_PREFIX
  # Migrations already applied by stack ensure; avoid double work on every API boot.
  export AVRAG_RUN_MIGRATIONS="${AVRAG_RUN_MIGRATIONS_PRODUCT:-false}"
  export REDIS_ADDR="${CLIENT_REDIS_HOST:-127.0.0.1}:${CLIENT_REDIS_PORT:-6380}"
  export RUST_LOG="${RUST_LOG:-info,avrag_api=info,avrag_worker=info}"
}

wait_api() {
  local deadline=$((SECONDS + ${1:-90}))
  log "waiting for API http://${CLIENT_API_HOST}:${CLIENT_API_PORT}/health …"
  while true; do
    if curl -fsS --max-time 2 "http://${CLIENT_API_HOST}:${CLIENT_API_PORT}/health" >/dev/null 2>&1; then
      log "API healthy"
      return 0
    fi
    if (( SECONDS >= deadline )); then
      die "timeout waiting for API health (see $LOG_DIR/api.log)"
    fi
    sleep 1
  done
}

start_procs() {
  load_env

  if ! port_open "${CLIENT_PG_HOST:-127.0.0.1}" "${CLIENT_PG_PORT:-5433}"; then
    die "Postgres not up on :${CLIENT_PG_PORT:-5433} — run: bash scripts/desktop-local-stack.sh ensure"
  fi
  if ! port_open "${CLIENT_REDIS_HOST:-127.0.0.1}" "${CLIENT_REDIS_PORT:-6380}"; then
    die "Redis not up on :${CLIENT_REDIS_PORT:-6380} — run: bash scripts/desktop-local-stack.sh ensure"
  fi

  if pid_alive "$PID_API" && curl -fsS --max-time 2 "http://${CLIENT_API_HOST}:${CLIENT_API_PORT}/health" >/dev/null 2>&1; then
    log "API already running (pid $(cat "$PID_API"))"
  else
    stop_pidfile api "$PID_API"
    local bins api_bin worker_bin
    bins="$(ensure_bins)"
    api_bin="${bins%%|*}"
    worker_bin="${bins##*|}"

    log "starting avrag-api → $AVRAG_API_ADDR ($api_bin)"
    (
      cd "$AVRAG_DIR"
      set -a
      # shellcheck disable=SC1091
      [[ -f "$AVRAG_DIR/.env" ]] && source "$AVRAG_DIR/.env"
      # shellcheck disable=SC1090
      source "$ENV_FILE"
      set +a
      export AVRAG_API_ADDR AVRAG_PUBLIC_BASE_URL AVRAG_OBJECT_ROOT MILVUS_COLLECTION_PREFIX
      export AVRAG_RUN_MIGRATIONS="${AVRAG_RUN_MIGRATIONS_PRODUCT:-false}"
      export REDIS_ADDR="${CLIENT_REDIS_HOST:-127.0.0.1}:${CLIENT_REDIS_PORT:-6380}"
      export RUST_LOG="${RUST_LOG:-info,avrag_api=info}"
      # Re-assert data plane after monorepo .env (client wins).
      # shellcheck disable=SC1090
      set -a; source "$ENV_FILE"; set +a
      export AVRAG_API_ADDR AVRAG_PUBLIC_BASE_URL AVRAG_OBJECT_ROOT MILVUS_COLLECTION_PREFIX
      export AVRAG_RUN_MIGRATIONS="${AVRAG_RUN_MIGRATIONS_PRODUCT:-false}"
      export REDIS_ADDR="${CLIENT_REDIS_HOST:-127.0.0.1}:${CLIENT_REDIS_PORT:-6380}"
      nohup "$api_bin" >>"$LOG_DIR/api.log" 2>&1 &
      echo $! >"$PID_API"
    )
  fi

  if pid_alive "$PID_WORKER"; then
    log "worker already running (pid $(cat "$PID_WORKER"))"
  else
    stop_pidfile worker "$PID_WORKER"
    local bins2 worker_bin2
    bins2="$(ensure_bins)"
    worker_bin2="${bins2##*|}"
    log "starting avrag-worker ($worker_bin2)"
    (
      cd "$AVRAG_DIR"
      set -a
      # shellcheck disable=SC1091
      [[ -f "$AVRAG_DIR/.env" ]] && source "$AVRAG_DIR/.env"
      # shellcheck disable=SC1090
      source "$ENV_FILE"
      set +a
      export AVRAG_API_ADDR AVRAG_PUBLIC_BASE_URL AVRAG_OBJECT_ROOT MILVUS_COLLECTION_PREFIX
      export AVRAG_RUN_MIGRATIONS="${AVRAG_RUN_MIGRATIONS_PRODUCT:-false}"
      export REDIS_ADDR="${CLIENT_REDIS_HOST:-127.0.0.1}:${CLIENT_REDIS_PORT:-6380}"
      export RUST_LOG="${RUST_LOG:-info,avrag_worker=info}"
      set -a; source "$ENV_FILE"; set +a
      export AVRAG_API_ADDR AVRAG_PUBLIC_BASE_URL AVRAG_OBJECT_ROOT MILVUS_COLLECTION_PREFIX
      export AVRAG_RUN_MIGRATIONS="${AVRAG_RUN_MIGRATIONS_PRODUCT:-false}"
      export REDIS_ADDR="${CLIENT_REDIS_HOST:-127.0.0.1}:${CLIENT_REDIS_PORT:-6380}"
      nohup "$worker_bin2" >>"$LOG_DIR/worker.log" 2>&1 &
      echo $! >"$PID_WORKER"
    )
  fi

  wait_api 120
  print_status
}

print_status() {
  local api_ok=DOWN worker_ok=DOWN health=""
  if curl -fsS --max-time 2 "http://${CLIENT_API_HOST}:${CLIENT_API_PORT}/health" >/dev/null 2>&1; then
    api_ok=OK
    health="$(curl -fsS --max-time 2 "http://${CLIENT_API_HOST}:${CLIENT_API_PORT}/health" 2>/dev/null || true)"
  elif port_open "$CLIENT_API_HOST" "$CLIENT_API_PORT"; then
    api_ok="PORT_OPEN"
  fi
  if pid_alive "$PID_WORKER"; then
    worker_ok=OK
  fi

  echo
  echo "  api     http://${CLIENT_API_HOST}:${CLIENT_API_PORT}  $api_ok"
  echo "  worker  pidfile=$PID_WORKER  $worker_ok"
  if [[ -n "$health" ]]; then
    echo "  health  $health"
  fi
  echo "  logs    $LOG_DIR/"
  echo "  objects $AVRAG_OBJECT_ROOT"
  echo "  env     $ENV_FILE"
  echo
}

cmd="${1:-status}"

case "$cmd" in
  ensure)
    # Data plane + product processes
    if [[ -x "$STACK_SCRIPT" ]]; then
      bash "$STACK_SCRIPT" ensure
    else
      die "stack script missing: $STACK_SCRIPT"
    fi
    start_procs
    log "ensure complete — product API at $AVRAG_PUBLIC_BASE_URL"
    ;;
  start)
    start_procs
    ;;
  stop)
    stop_pidfile api "$PID_API"
    stop_pidfile worker "$PID_WORKER"
    log "product processes stopped"
    ;;
  status)
    print_status
    if pid_alive "$PID_API"; then
      echo "  api.pid=$(cat "$PID_API")"
    fi
    if pid_alive "$PID_WORKER"; then
      echo "  worker.pid=$(cat "$PID_WORKER")"
    fi
    ;;
  logs)
    tail -n 80 "$LOG_DIR/api.log" 2>/dev/null || true
    echo "-----"
    tail -n 80 "$LOG_DIR/worker.log" 2>/dev/null || true
    ;;
  *)
    die "usage: $0 {ensure|start|stop|status|logs}"
    ;;
esac
