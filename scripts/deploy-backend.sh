#!/usr/bin/env bash
# Build avrag-api + avrag-worker and deploy artifacts to VPS.
# Source of truth: local git tree. VPS receives bins/migrations/prompts only.
#
# Env (from avrag-rs/.env): VPS_MAIN_HOST, VPS_MAIN_USER, VPS_MAIN_PASSWORD
# Optional:
#   SKIP_BUILD=1     use existing target/release binaries
#   ASSETS_ONLY=1    only sync migrations + prompts (no bin rebuild/restart)
#   NO_RESTART=1     upload only; do not recreate containers
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
ENV_FILE="$ROOT/avrag-rs/.env"
AVRAG_DIR="$ROOT/avrag-rs"
REMOTE_ROOT="/opt/avrag-rs"
STAGE="$(mktemp -d /tmp/avrag-be-deploy.XXXXXX)"
trap 'rm -rf "$STAGE"' EXIT

die() { echo "deploy-backend: $*" >&2; exit 1; }

if [[ -f "$ENV_FILE" ]]; then
  set -a
  # shellcheck disable=SC1090
  source "$ENV_FILE"
  set +a
fi

: "${VPS_MAIN_HOST:?set VPS_MAIN_HOST in avrag-rs/.env}"
: "${VPS_MAIN_USER:?set VPS_MAIN_USER}"
: "${VPS_MAIN_PASSWORD:?set VPS_MAIN_PASSWORD}"

SSH=(sshpass -p "$VPS_MAIN_PASSWORD" ssh -o StrictHostKeyChecking=no "${VPS_MAIN_USER}@${VPS_MAIN_HOST}")
SCP=(sshpass -p "$VPS_MAIN_PASSWORD" scp -o StrictHostKeyChecking=no)

cd "$ROOT"
REV="$(git rev-parse --short HEAD 2>/dev/null || echo unknown)"
DIRTY=""
if ! git diff --quiet 2>/dev/null || ! git diff --cached --quiet 2>/dev/null; then
  DIRTY="+dirty"
fi
RELEASE_ID="${REV}${DIRTY}"
BUILT_AT="$(date -u +%Y-%m-%dT%H:%M:%SZ)"

SKIP_BUILD="${SKIP_BUILD:-0}"
ASSETS_ONLY="${ASSETS_ONLY:-0}"
NO_RESTART="${NO_RESTART:-0}"

echo "deploy-backend: rev=${RELEASE_ID}"

API_BIN="$AVRAG_DIR/target/release/avrag-api"
WORKER_BIN="$AVRAG_DIR/target/release/avrag-worker"

if [[ "$ASSETS_ONLY" != "1" ]]; then
  if [[ "$SKIP_BUILD" != "1" ]]; then
    echo "deploy-backend: cargo build --release -p avrag-api -p avrag-worker"
    (
      cd "$AVRAG_DIR"
      cargo build --release -p avrag-api -p avrag-worker
    )
  else
    echo "deploy-backend: SKIP_BUILD=1 (using existing release bins)"
  fi
  [[ -x "$API_BIN" ]] || die "missing $API_BIN"
  [[ -x "$WORKER_BIN" ]] || die "missing $WORKER_BIN"
fi

[[ -d "$AVRAG_DIR/migrations" ]] || die "missing migrations/"
[[ -d "$AVRAG_DIR/prompts" ]] || die "missing prompts/"

mkdir -p "$STAGE/bin" "$STAGE/migrations" "$STAGE/prompts" "$STAGE/docker"

if [[ "$ASSETS_ONLY" != "1" ]]; then
  cp -a "$API_BIN" "$STAGE/bin/avrag-api"
  cp -a "$WORKER_BIN" "$STAGE/bin/avrag-worker"
  chmod 755 "$STAGE/bin/avrag-api" "$STAGE/bin/avrag-worker"
fi

# migrations: sql only (skip large _backups if present)
rsync -a --delete \
  --exclude '_backups/' \
  --exclude '*.md' \
  "$AVRAG_DIR/migrations/" "$STAGE/migrations/"

rsync -a --delete \
  --exclude '_backups/' \
  "$AVRAG_DIR/prompts/" "$STAGE/prompts/"

cp -a "$ROOT/deploy/docker/run-avrag-containers.sh" "$STAGE/docker/run-avrag-containers.sh"
chmod 755 "$STAGE/docker/run-avrag-containers.sh"

cat > "$STAGE/DEPLOY_META.backend.json" <<EOF
{
  "component": "backend",
  "git_rev": "${REV}",
  "dirty": $([[ -n "$DIRTY" ]] && echo true || echo false),
  "release_id": "${RELEASE_ID}",
  "built_at": "${BUILT_AT}",
  "host": "$(hostname -s 2>/dev/null || echo local)",
  "assets_only": $([[ "$ASSETS_ONLY" == "1" ]] && echo true || echo false),
  "skip_build": $([[ "$SKIP_BUILD" == "1" ]] && echo true || echo false)
}
EOF

TGZ="/tmp/avrag-backend-${RELEASE_ID//\//-}.tgz"
tar czf "$TGZ" -C "$STAGE" .
echo "deploy-backend: upload $(du -h "$TGZ" | awk '{print $1}')"

"${SCP[@]}" "$TGZ" "${VPS_MAIN_USER}@${VPS_MAIN_HOST}:/tmp/avrag-backend-deploy.tgz"

"${SSH[@]}" bash -s <<REMOTE
set -euo pipefail
REMOTE_ROOT="$REMOTE_ROOT"
RELEASE_ID="$RELEASE_ID"
BUILT_AT="$BUILT_AT"
REV="$REV"
ASSETS_ONLY="$ASSETS_ONLY"
NO_RESTART="$NO_RESTART"

STAGE=/tmp/avrag-backend-stage
rm -rf "\$STAGE"
mkdir -p "\$STAGE"
tar xzf /tmp/avrag-backend-deploy.tgz -C "\$STAGE"

mkdir -p "\$REMOTE_ROOT/bin" "\$REMOTE_ROOT/migrations" "\$REMOTE_ROOT/prompts" "\$REMOTE_ROOT/docker"

if [[ "\$ASSETS_ONLY" != "1" ]]; then
  install -m 755 "\$STAGE/bin/avrag-api" "\$REMOTE_ROOT/bin/avrag-api"
  install -m 755 "\$STAGE/bin/avrag-worker" "\$REMOTE_ROOT/bin/avrag-worker"
fi

# Preserve remote-only noise under migrations/_backups if any
rsync -a --delete \
  --exclude '_backups/' \
  "\$STAGE/migrations/" "\$REMOTE_ROOT/migrations/"
rsync -a --delete \
  --exclude '_backups/' \
  "\$STAGE/prompts/" "\$REMOTE_ROOT/prompts/"

install -m 755 "\$STAGE/docker/run-avrag-containers.sh" "\$REMOTE_ROOT/docker/run-avrag-containers.sh"
install -m 644 "\$STAGE/DEPLOY_META.backend.json" "\$REMOTE_ROOT/DEPLOY_META.backend.json"

# markitdown CLI: production document parsing runs in the worker by invoking
# the `markitdown` binary on PATH (env MARKITDOWN_BIN, MARKITDOWN_TIMEOUT_MS).
# Provision it here, idempotently: skip when already installed.
# NOTE: the old office parser service (:9090) and PDF renderer (:9091) are
# retired — they are no longer called and this script does not deploy them.
if command -v markitdown >/dev/null 2>&1; then
  echo "deploy-backend: markitdown present (\$(command -v markitdown)); install skipped"
else
  echo "deploy-backend: installing markitdown CLI"
  if command -v uv >/dev/null 2>&1; then
    uv tool install markitdown
  elif command -v python3 >/dev/null 2>&1; then
    python3 -m pip install --user 'markitdown[all]'
    # pip --user lands in ~/.local/bin, which non-login shells may not have on
    # PATH; expose it via /usr/local/bin when writable.
    if ! command -v markitdown >/dev/null 2>&1 && [[ -x "\$HOME/.local/bin/markitdown" ]]; then
      if [[ -w /usr/local/bin ]]; then
        ln -sf "\$HOME/.local/bin/markitdown" /usr/local/bin/markitdown
      else
        export PATH="\$HOME/.local/bin:\$PATH"
      fi
    fi
  else
    echo "deploy-backend: ERROR neither uv nor python3 available to install markitdown" >&2
    exit 1
  fi
fi
if ! command -v markitdown >/dev/null 2>&1; then
  echo "deploy-backend: ERROR markitdown CLI not on PATH after install" >&2
  exit 1
fi
markitdown --help >/dev/null 2>&1 || { echo "deploy-backend: ERROR 'markitdown --help' failed" >&2; exit 1; }
echo "deploy-backend: markitdown OK (\$(command -v markitdown))"

if [[ "\$NO_RESTART" != "1" && "\$ASSETS_ONLY" != "1" ]]; then
  bash "\$REMOTE_ROOT/docker/run-avrag-containers.sh"
elif [[ "\$NO_RESTART" != "1" && "\$ASSETS_ONLY" == "1" ]]; then
  # assets only: restart containers to pick up new migrations/prompts mounts
  docker restart avrag-api avrag-worker
  sleep 2
  curl -sS -m 8 -o /dev/null -w "api_health:%{http_code}\\n" http://127.0.0.1:8081/health || true
fi

META=/opt/avrag-rs/DEPLOYED.txt
{
  echo "backend_rev=\$REV"
  echo "backend_release_id=\$RELEASE_ID"
  echo "backend_built_at=\$BUILT_AT"
  echo "backend_deployed_at=\$(date -u +%Y-%m-%dT%H:%M:%SZ)"
  echo "backend_assets_only=\$ASSETS_ONLY"
} >> "\$META"
tail -n 40 "\$META" > "\$META.tmp" && mv "\$META.tmp" "\$META"

curl -sS -m 8 -o /dev/null -w "pub_health:%{http_code}\\n" https://app.contextlm.top/health || true
test -f "\$REMOTE_ROOT/DEPLOY_META.backend.json"
echo "deploy-backend: remote OK"
REMOTE

rm -f "$TGZ"
echo "deploy-backend: done rev=${RELEASE_ID}"
