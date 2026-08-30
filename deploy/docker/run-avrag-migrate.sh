#!/usr/bin/env bash
# One-shot migration container (runs ON the VPS).
# Uses the MIGRATION role DSN from /etc/avrag-rs/migrate.env — never the runtime
# env file, so the api/worker env never carries migration privileges.
# Exit code: migrations success/failure (propagated by deploy-backend.sh).
set -euo pipefail

RUNTIME_IMAGE="${AVRAG_RUNTIME_IMAGE:-avrag-runtime:24.04}"
MIGRATE_ENV_FILE="${AVRAG_MIGRATE_ENV_FILE:-/etc/avrag-rs/migrate.env}"
OPT_ROOT="${AVRAG_OPT_ROOT:-/opt/avrag-rs}"

die() { echo "run-avrag-migrate: $*" >&2; exit 1; }

[[ -f "$MIGRATE_ENV_FILE" ]] || die "missing migrate env file: $MIGRATE_ENV_FILE"
[[ -x "$OPT_ROOT/bin/avrag-migrate" ]] || die "missing $OPT_ROOT/bin/avrag-migrate"
docker image inspect "$RUNTIME_IMAGE" >/dev/null 2>&1 || die "missing image $RUNTIME_IMAGE"

docker rm -f avrag-migrate >/dev/null 2>&1 || true

# Runs to completion then exits; --rm cleans up. Host network so it reaches PG.
docker run --rm \
  --name avrag-migrate \
  --network host \
  --ulimit nofile=65536:65536 \
  --env-file "$MIGRATE_ENV_FILE" \
  -e MIGRATION_DATABASE_URL \
  -v "${OPT_ROOT}:${OPT_ROOT}:ro" \
  "$RUNTIME_IMAGE" \
  "${OPT_ROOT}/bin/avrag-migrate"

echo "run-avrag-migrate: migrations complete"