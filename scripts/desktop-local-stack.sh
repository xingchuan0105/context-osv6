#!/usr/bin/env bash
# Start/stop/status/migrate for Context-OS Client local data plane
# (Postgres+pgvector + Redis). Writes desktop/runtime/client.env.
#
# STACK_MODE:
#   auto   (default) — native if host tools found, else docker compose
#   native           — host pg_ctl + redis-server (no Docker)
#   docker           — docker compose (legacy / CI)
#
# Desktop retrieval: RETRIEVAL_BACKEND=pgvector (no Milvus).
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
RUNTIME_DIR="$ROOT/desktop/runtime"
COMPOSE_FILE="$RUNTIME_DIR/docker-compose.client.yml"
DATA_DIR="$RUNTIME_DIR/data"
ENV_FILE="$RUNTIME_DIR/client.env"
MIGRATIONS_DIR="$ROOT/avrag-rs/migrations"
RUN_DIR="$RUNTIME_DIR/run"
LOG_DIR="$RUNTIME_DIR/logs"
MODE_FILE="$RUNTIME_DIR/stack.mode"

# Defaults (localhost-only; offset from host system services)
CLIENT_PG_HOST="${CLIENT_PG_HOST:-127.0.0.1}"
CLIENT_PG_PORT="${CLIENT_PG_PORT:-5433}"
CLIENT_REDIS_HOST="${CLIENT_REDIS_HOST:-127.0.0.1}"
CLIENT_REDIS_PORT="${CLIENT_REDIS_PORT:-6380}"
CLIENT_PG_USER="${CLIENT_PG_USER:-avrag}"
CLIENT_PG_PASSWORD="${CLIENT_PG_PASSWORD:-avrag}"
CLIENT_PG_DB="${CLIENT_PG_DB:-avrag_client}"
RETRIEVAL_BACKEND="${RETRIEVAL_BACKEND:-pgvector}"
STACK_MODE="${STACK_MODE:-auto}"

DATABASE_URL="${DATABASE_URL:-postgres://${CLIENT_PG_USER}:${CLIENT_PG_PASSWORD}@${CLIENT_PG_HOST}:${CLIENT_PG_PORT}/${CLIENT_PG_DB}}"
REDIS_URL="${REDIS_URL:-redis://${CLIENT_REDIS_HOST}:${CLIENT_REDIS_PORT}/0}"

# Native data dirs (separate from docker volumes under data/pg, data/redis)
PGDATA_NATIVE="${PGDATA_NATIVE:-$DATA_DIR/pg-native}"
REDIS_DIR_NATIVE="${REDIS_DIR_NATIVE:-$DATA_DIR/redis-native}"
REDIS_PID="$RUN_DIR/redis-native.pid"
PG_LOG="$LOG_DIR/postgres-native.log"
REDIS_LOG="$LOG_DIR/redis-native.log"

die() { echo "desktop-local-stack: $*" >&2; exit 1; }
log() { echo "desktop-local-stack: $*"; }

mkdir -p "$DATA_DIR" "$RUN_DIR" "$LOG_DIR" "$PGDATA_NATIVE" "$REDIS_DIR_NATIVE"

port_open() {
  local host="${2:-127.0.0.1}" port="$1"
  if (echo >/dev/tcp/"$host"/"$port") >/dev/null 2>&1; then
    return 0
  fi
  if command -v nc >/dev/null 2>&1 && nc -z "$host" "$port" 2>/dev/null; then
    return 0
  fi
  return 1
}

# ── Native tool discovery ────────────────────────────────────────────────────

# Prefer install/bundled portable trees unless COS_USE_SYSTEM_PG=1.
# Order: env → runtime/pgsql → bundled/* → native/ → system → PATH
find_pg_bin() {
  local d use_sys="${COS_USE_SYSTEM_PG:-0}"
  if [[ -n "${PG_BIN_DIR:-}" ]]; then
    d="$PG_BIN_DIR"
    if [[ -x "$d/pg_ctl" || -f "$d/pg_ctl.exe" ]] && [[ -x "$d/initdb" || -f "$d/initdb.exe" ]]; then
      echo "$d"
      return 0
    fi
  fi
  if [[ "$use_sys" != "1" && "$use_sys" != "true" ]]; then
    for d in \
      "$RUNTIME_DIR/pgsql/bin" \
      "$RUNTIME_DIR/bundled/windows-x64/pgsql/bin" \
      "$RUNTIME_DIR/bundled/linux-x64/pgsql/bin" \
      "$RUNTIME_DIR/native/pgsql/bin" \
      "${CONTEXT_OS_RUNTIME:+$CONTEXT_OS_RUNTIME/pgsql/bin}"
    do
      [[ -n "$d" ]] || continue
      if [[ -x "$d/pg_ctl" || -f "$d/pg_ctl.exe" ]] && [[ -x "$d/initdb" || -f "$d/initdb.exe" ]]; then
        echo "$d"
        return 0
      fi
    done
  fi
  for d in \
    /usr/lib/postgresql/16/bin \
    /usr/lib/postgresql/15/bin \
    /usr/lib/postgresql/17/bin \
    /usr/local/pgsql/bin
  do
    [[ -x "$d/pg_ctl" && -x "$d/initdb" ]] || continue
    echo "$d"
    return 0
  done
  if command -v pg_ctl >/dev/null 2>&1 && command -v initdb >/dev/null 2>&1; then
    dirname "$(command -v pg_ctl)"
    return 0
  fi
  return 1
}

find_redis_server() {
  local f use_sys="${COS_USE_SYSTEM_PG:-0}"
  if [[ -n "${REDIS_SERVER_BIN:-}" && ( -x "$REDIS_SERVER_BIN" || -f "$REDIS_SERVER_BIN" ) ]]; then
    echo "$REDIS_SERVER_BIN"
    return 0
  fi
  if [[ "$use_sys" != "1" && "$use_sys" != "true" ]]; then
    for f in \
      "$RUNTIME_DIR/redis/redis-server.exe" \
      "$RUNTIME_DIR/redis/redis-server" \
      "$RUNTIME_DIR/bundled/windows-x64/redis/redis-server.exe" \
      "$RUNTIME_DIR/bundled/windows-x64/redis/redis-server" \
      "$RUNTIME_DIR/bundled/linux-x64/redis/redis-server" \
      "$RUNTIME_DIR/native/redis/redis-server" \
      "$RUNTIME_DIR/native/redis/redis-server.exe"
    do
      if [[ -x "$f" || -f "$f" ]]; then
        echo "$f"
        return 0
      fi
    done
    if [[ -n "${CONTEXT_OS_RUNTIME:-}" ]]; then
      for f in \
        "$CONTEXT_OS_RUNTIME/redis/redis-server.exe" \
        "$CONTEXT_OS_RUNTIME/redis/redis-server"
      do
        if [[ -x "$f" || -f "$f" ]]; then
          echo "$f"
          return 0
        fi
      done
    fi
  fi
  command -v redis-server 2>/dev/null
}

native_tools_ok() {
  find_pg_bin >/dev/null 2>&1 && find_redis_server >/dev/null 2>&1
}

docker_tools_ok() {
  command -v docker >/dev/null 2>&1 && docker compose version >/dev/null 2>&1
}

resolve_mode() {
  local want="${STACK_MODE:-auto}"
  case "$want" in
    native|docker) echo "$want"; return 0 ;;
    auto)
      if native_tools_ok; then
        echo "native"
      elif docker_tools_ok; then
        echo "docker"
      else
        die "no stack backend: install postgresql-16 + postgresql-16-pgvector + redis-server
  (Debian/Ubuntu: sudo apt-get install -y postgresql-16 postgresql-16-pgvector redis-server)
  or install Docker and set STACK_MODE=docker"
      fi
      ;;
    *) die "invalid STACK_MODE=$want (use auto|native|docker)" ;;
  esac
}

save_mode() {
  printf '%s\n' "$1" >"$MODE_FILE"
}

load_mode() {
  if [[ -f "$MODE_FILE" ]]; then
    tr -d '[:space:]' <"$MODE_FILE"
  else
    resolve_mode
  fi
}

# ── Env file ─────────────────────────────────────────────────────────────────

write_env() {
  local client_api_host="${CLIENT_API_HOST:-127.0.0.1}"
  local client_api_port="${CLIENT_API_PORT:-18080}"
  local object_root="${AVRAG_OBJECT_ROOT:-$RUNTIME_DIR/objects}"
  local jwt_file="$RUNTIME_DIR/jwt.secret"
  local jwt_secret="${JWT_SECRET:-}"
  local stack_mode_val="${1:-$(load_mode 2>/dev/null || echo native)}"
  mkdir -p "$object_root"
  if [[ -z "$jwt_secret" && -f "$jwt_file" ]]; then
    jwt_secret="$(tr -d '[:space:]' <"$jwt_file")"
  fi
  if [[ -z "$jwt_secret" ]]; then
    if command -v openssl >/dev/null 2>&1; then
      jwt_secret="$(openssl rand -hex 32)"
    else
      jwt_secret="$(head -c 32 /dev/urandom | od -An -tx1 | tr -d ' \n')"
    fi
    printf '%s\n' "$jwt_secret" >"$jwt_file"
    chmod 600 "$jwt_file" 2>/dev/null || true
    log "generated $jwt_file"
  fi
  cat >"$ENV_FILE" <<EOF
# Generated by scripts/desktop-local-stack.sh — do not commit secrets for shared envs.
# Source:  set -a && source desktop/runtime/client.env && set +a
# Product: bash scripts/desktop-local-product.sh ensure
#
# STACK_MODE=${stack_mode_val}  (native = no Docker)

STACK_MODE=${stack_mode_val}
CLIENT_PG_HOST=${CLIENT_PG_HOST}
CLIENT_PG_PORT=${CLIENT_PG_PORT}
CLIENT_REDIS_HOST=${CLIENT_REDIS_HOST}
CLIENT_REDIS_PORT=${CLIENT_REDIS_PORT}
CLIENT_API_HOST=${client_api_host}
CLIENT_API_PORT=${client_api_port}

DATABASE_URL=${DATABASE_URL}
REDIS_URL=${REDIS_URL}
REDIS_ADDR=${CLIENT_REDIS_HOST}:${CLIENT_REDIS_PORT}

RETRIEVAL_BACKEND=${RETRIEVAL_BACKEND}
MILVUS_COLLECTION_PREFIX=avrag_client

AVRAG_API_ADDR=${client_api_host}:${client_api_port}
AVRAG_PUBLIC_BASE_URL=http://${client_api_host}:${client_api_port}
AVRAG_OBJECT_ROOT=${object_root}

JWT_SECRET=${jwt_secret}

AVRAG_RUN_MIGRATIONS=true
AVRAG_MIGRATIONS_DIR=${MIGRATIONS_DIR}
EOF
  log "wrote $ENV_FILE (STACK_MODE=${stack_mode_val}, RETRIEVAL_BACKEND=${RETRIEVAL_BACKEND})"
}

print_env_hint() {
  echo
  echo "Client process env:"
  echo "  STACK_MODE=$(load_mode 2>/dev/null || echo '?')"
  echo "  DATABASE_URL=$DATABASE_URL"
  echo "  REDIS_URL=$REDIS_URL"
  echo "  RETRIEVAL_BACKEND=${RETRIEVAL_BACKEND}"
  echo "  API       http://127.0.0.1:${CLIENT_API_PORT:-18080}"
  echo "  file: $ENV_FILE"
  echo "  product: bash scripts/desktop-local-product.sh ensure"
}

# ── Migrations ───────────────────────────────────────────────────────────────

pg_has_pgvector_rag() {
  command -v psql >/dev/null 2>&1 || return 1
  PGPASSWORD="${CLIENT_PG_PASSWORD}" psql -h "$CLIENT_PG_HOST" -p "$CLIENT_PG_PORT" \
    -U "$CLIENT_PG_USER" -d "$CLIENT_PG_DB" -tAc \
    "SELECT 1 FROM information_schema.tables WHERE table_name = 'rag_kg_relations' LIMIT 1" \
    2>/dev/null | grep -q 1
}

desktop_soft_migrate_fallback() {
  local err="$1"
  if ! pg_has_pgvector_rag; then
    return 1
  fi
  if echo "$err" | grep -qiE 'pg_bigm|0061|rag_bigm'; then
    log "WARNING: migration 0061 (pg_bigm) unavailable — CJK lexical may degrade; VGRAG/graph OK"
  else
    log "WARNING: migrate error but rag_kg_* exists; continuing (desktop soft path)"
  fi
  PGPASSWORD="${CLIENT_PG_PASSWORD}" psql -h "$CLIENT_PG_HOST" -p "$CLIENT_PG_PORT" \
    -U "$CLIENT_PG_USER" -d "$CLIENT_PG_DB" -v ON_ERROR_STOP=0 \
    -c "CREATE EXTENSION IF NOT EXISTS pg_trgm;" >/dev/null 2>&1 || true
  return 0
}

run_migrate() {
  [[ -d "$MIGRATIONS_DIR" ]] || die "migrations dir missing: $MIGRATIONS_DIR"
  write_env "$(load_mode)"

  if ! port_open "$CLIENT_PG_PORT"; then
    die "Postgres not reachable on :${CLIENT_PG_PORT} — run: $0 ensure"
  fi

  local sqlx_bin=""
  if command -v sqlx >/dev/null 2>&1; then
    sqlx_bin="sqlx"
  elif [[ -x "${HOME}/.cargo/bin/sqlx" ]]; then
    sqlx_bin="${HOME}/.cargo/bin/sqlx"
  else
    die "sqlx CLI not found. Install: cargo install sqlx-cli --no-default-features --features postgres"
  fi

  log "running $sqlx_bin migrate run …"
  local migrate_out
  set +e
  migrate_out="$("$sqlx_bin" migrate run \
    --source "$MIGRATIONS_DIR" \
    --database-url "$DATABASE_URL" 2>&1)"
  local migrate_rc=$?
  set -e
  echo "$migrate_out"

  if [[ $migrate_rc -eq 0 ]]; then
    log "migrations applied"
    return 0
  fi
  if desktop_soft_migrate_fallback "$migrate_out"; then
    log "migrations soft-complete (desktop pgvector path)"
    return 0
  fi
  die "sqlx migrate failed (exit $migrate_rc)"
}

# ── Native Postgres + Redis ──────────────────────────────────────────────────

stop_docker_client_stack() {
  if docker_tools_ok && [[ -f "$COMPOSE_FILE" ]]; then
    log "stopping docker compose client stack (free ports for native)…"
    (
      cd "$RUNTIME_DIR"
      export COMPOSE_PROJECT_NAME=context-os-client
      docker compose -f docker-compose.client.yml down 2>/dev/null || true
    )
  fi
  local c
  for c in cos-client-postgres cos-client-redis cos-client-milvus cos-client-etcd cos-client-milvus-minio; do
    docker rm -f "$c" >/dev/null 2>&1 || true
  done
}

native_pg_init_if_needed() {
  local pg_bin="$1"
  if [[ -f "$PGDATA_NATIVE/PG_VERSION" ]]; then
    return 0
  fi
  log "initdb native cluster at $PGDATA_NATIVE …"
  # Superuser = CLIENT_PG_USER; trust for local single-user desktop.
  "$pg_bin/initdb" \
    -D "$PGDATA_NATIVE" \
    -U "$CLIENT_PG_USER" \
    --auth-local=trust \
    --auth-host=scram-sha-256 \
    --encoding=UTF8 \
    --locale=C \
    -N
  {
    echo "listen_addresses = '127.0.0.1'"
    echo "port = ${CLIENT_PG_PORT}"
    echo "unix_socket_directories = '$RUN_DIR'"
    echo "max_connections = 40"
    echo "shared_buffers = 128MB"
  } >>"$PGDATA_NATIVE/postgresql.conf"
  # password for TCP (scram) — set via env after first start
  cat >"$PGDATA_NATIVE/pg_hba.conf" <<EOF
# TYPE  DATABASE        USER            ADDRESS                 METHOD
local   all             all                                     trust
host    all             all             127.0.0.1/32            trust
host    all             all             ::1/128                 trust
EOF
}

native_pg_start() {
  local pg_bin
  pg_bin="$(find_pg_bin)" || die "pg_ctl/initdb not found (install postgresql-16)"
  native_pg_init_if_needed "$pg_bin"

  if "$pg_bin/pg_ctl" -D "$PGDATA_NATIVE" status >/dev/null 2>&1; then
    log "native postgres already running"
  else
    log "starting native postgres :${CLIENT_PG_PORT} …"
    "$pg_bin/pg_ctl" -D "$PGDATA_NATIVE" -l "$PG_LOG" -w start \
      -o "-p ${CLIENT_PG_PORT} -c listen_addresses=127.0.0.1 -c unix_socket_directories=${RUN_DIR}"
  fi

  # Ensure database exists (user already superuser from initdb).
  local psql="$pg_bin/psql"
  if [[ ! -x "$psql" ]]; then
    psql="$(command -v psql)" || die "psql not found"
  fi
  export PGHOST=127.0.0.1
  export PGPORT="$CLIENT_PG_PORT"
  export PGUSER="$CLIENT_PG_USER"
  if ! "$psql" -d postgres -tAc "SELECT 1 FROM pg_database WHERE datname='${CLIENT_PG_DB}'" | grep -q 1; then
    log "creating database ${CLIENT_PG_DB}"
    "$pg_bin/createdb" -h 127.0.0.1 -p "$CLIENT_PG_PORT" -U "$CLIENT_PG_USER" "$CLIENT_PG_DB" \
      || "$psql" -d postgres -c "CREATE DATABASE ${CLIENT_PG_DB} OWNER ${CLIENT_PG_USER};"
  fi
  # Prefer password URL for clients that ignore trust; set password if empty.
  "$psql" -d "$CLIENT_PG_DB" -v ON_ERROR_STOP=0 -c \
    "ALTER USER ${CLIENT_PG_USER} WITH PASSWORD '${CLIENT_PG_PASSWORD}';" >/dev/null 2>&1 || true
  "$psql" -d "$CLIENT_PG_DB" -v ON_ERROR_STOP=0 -c \
    "CREATE EXTENSION IF NOT EXISTS vector;" >/dev/null 2>&1 || true
  unset PGHOST PGPORT PGUSER
}

native_pg_stop() {
  local pg_bin
  pg_bin="$(find_pg_bin 2>/dev/null)" || return 0
  if [[ -f "$PGDATA_NATIVE/PG_VERSION" ]] && "$pg_bin/pg_ctl" -D "$PGDATA_NATIVE" status >/dev/null 2>&1; then
    log "stopping native postgres …"
    "$pg_bin/pg_ctl" -D "$PGDATA_NATIVE" -m fast -w stop || true
  fi
}

native_redis_start() {
  local rbin
  rbin="$(find_redis_server)" || die "redis-server not found (install redis-server)"
  if port_open "$CLIENT_REDIS_PORT"; then
    log "redis already listening on :${CLIENT_REDIS_PORT}"
    return 0
  fi
  if [[ -f "$REDIS_PID" ]] && kill -0 "$(cat "$REDIS_PID" 2>/dev/null)" 2>/dev/null; then
    log "redis pidfile alive"
    return 0
  fi
  log "starting native redis-server :${CLIENT_REDIS_PORT} …"
  "$rbin" \
    --daemonize yes \
    --port "$CLIENT_REDIS_PORT" \
    --bind 127.0.0.1 \
    --dir "$REDIS_DIR_NATIVE" \
    --dbfilename dump.rdb \
    --appendonly yes \
    --pidfile "$REDIS_PID" \
    --logfile "$REDIS_LOG"
}

native_redis_stop() {
  if [[ -f "$REDIS_PID" ]]; then
    local pid
    pid="$(cat "$REDIS_PID" 2>/dev/null || true)"
    if [[ -n "${pid:-}" ]] && kill -0 "$pid" 2>/dev/null; then
      log "stopping native redis pid=$pid"
      if command -v redis-cli >/dev/null 2>&1; then
        redis-cli -h 127.0.0.1 -p "$CLIENT_REDIS_PORT" shutdown nosave 2>/dev/null || kill "$pid" 2>/dev/null || true
      else
        kill "$pid" 2>/dev/null || true
      fi
    fi
    rm -f "$REDIS_PID"
  elif port_open "$CLIENT_REDIS_PORT" && command -v redis-cli >/dev/null 2>&1; then
    # May be system redis — only shutdown if our data dir matches (best-effort skip).
    log "redis still on :${CLIENT_REDIS_PORT} (not our pidfile) — leave running"
  fi
}

native_up() {
  stop_docker_client_stack
  native_redis_start
  native_pg_start
}

native_down() {
  native_pg_stop
  native_redis_stop
}

wait_ports() {
  local deadline=$((SECONDS + ${1:-90}))
  local port name
  for spec in "${CLIENT_PG_PORT}:postgres" "${CLIENT_REDIS_PORT}:redis"; do
    port="${spec%%:*}"
    name="${spec##*:}"
    log "waiting for $name :$port …"
    while ! port_open "$port"; do
      if (( SECONDS >= deadline )); then
        die "timeout waiting for $name on :$port"
      fi
      sleep 0.5
    done
    log "$name :$port OK"
  done
}

# ── Docker backend (fallback) ────────────────────────────────────────────────

compose() {
  docker compose -f docker-compose.client.yml "$@"
}

docker_up() {
  docker_tools_ok || die "docker compose not available"
  [[ -f "$COMPOSE_FILE" ]] || die "missing $COMPOSE_FILE"
  # Free ports if native still holds them
  native_pg_stop 2>/dev/null || true
  native_redis_stop 2>/dev/null || true
  cd "$RUNTIME_DIR"
  export COMPOSE_PROJECT_NAME=context-os-client
  log "starting docker Postgres+pgvector + Redis …"
  compose up -d
}

docker_down() {
  if docker_tools_ok && [[ -f "$COMPOSE_FILE" ]]; then
    cd "$RUNTIME_DIR"
    export COMPOSE_PROJECT_NAME=context-os-client
    compose down || true
  fi
}

# ── Commands ─────────────────────────────────────────────────────────────────

cmd="${1:-status}"
MODE="$(resolve_mode)"

case "$cmd" in
  up)
    log "STACK_MODE=$MODE — starting data plane …"
    save_mode "$MODE"
    if [[ "$MODE" == "native" ]]; then
      native_up
    else
      docker_up
    fi
    wait_ports 90
    write_env "$MODE"
    print_env_hint
    log "stack up ($MODE)"
    ;;
  ensure)
    log "ensure: STACK_MODE=$MODE (up + migrate) …"
    save_mode "$MODE"
    if [[ "$MODE" == "native" ]]; then
      native_up
    else
      docker_up
    fi
    wait_ports 90
    write_env "$MODE"
    run_migrate
    print_env_hint
    log "ensure complete ($MODE)"
    ;;
  down)
    local_mode="$(load_mode 2>/dev/null || echo "$MODE")"
    log "stopping stack (recorded mode=$local_mode) …"
    # Stop both backends so ports are free either way.
    native_down
    docker_down
    log "stopped (data under $DATA_DIR retained)"
    ;;
  status)
    echo "STACK_MODE request=$STACK_MODE resolved=$MODE recorded=$(cat "$MODE_FILE" 2>/dev/null || echo none)"
    echo "native tools: pg=$(find_pg_bin 2>/dev/null || echo MISSING) redis=$(find_redis_server 2>/dev/null || echo MISSING)"
    echo "docker: $(docker_tools_ok && echo ok || echo unavailable)"
    echo
    for spec in "${CLIENT_PG_PORT}:postgres" "${CLIENT_REDIS_PORT}:redis"; do
      port="${spec%%:*}"
      name="${spec##*:}"
      if port_open "$port"; then
        echo "  $name :$port  OK"
      else
        echo "  $name :$port  DOWN"
      fi
    done
    if [[ -f "$ENV_FILE" ]]; then
      echo
      echo "  client.env: $ENV_FILE"
      grep -E '^(STACK_MODE|RETRIEVAL_BACKEND)=' "$ENV_FILE" 2>/dev/null || true
    fi
    print_env_hint
    ;;
  write-env|env)
    save_mode "$MODE"
    write_env "$MODE"
    print_env_hint
    ;;
  migrate)
    run_migrate
    ;;
  logs)
    if [[ "$(load_mode)" == "native" ]]; then
      echo "=== postgres ($PG_LOG) ==="
      tail -n 80 "$PG_LOG" 2>/dev/null || true
      echo "=== redis ($REDIS_LOG) ==="
      tail -n 40 "$REDIS_LOG" 2>/dev/null || true
    else
      cd "$RUNTIME_DIR"
      export COMPOSE_PROJECT_NAME=context-os-client
      compose logs -f --tail=100
    fi
    ;;
  *)
    die "usage: $0 {up|ensure|down|status|write-env|migrate|logs}
  STACK_MODE=auto|native|docker (default auto)"
    ;;
esac
