#!/usr/bin/env bash
# Build frontend_next standalone and deploy to VPS.
# Source of truth: local git tree. VPS receives artifacts only.
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
ENV_FILE="$ROOT/avrag-rs/.env"
FE_DIR="$ROOT/frontend_next"
REMOTE_DIR="/opt/avrag-rs/frontend"
STAGE="$(mktemp -d /tmp/avrag-fe-deploy.XXXXXX)"
trap 'rm -rf "$STAGE"' EXIT

die() { echo "deploy-frontend: $*" >&2; exit 1; }

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

echo "deploy-frontend: rev=${RELEASE_ID}"
echo "deploy-frontend: building…"
cd "$FE_DIR"
export NEXT_TELEMETRY_DISABLED=1
pnpm build

echo "deploy-frontend: packaging standalone…"
# Flatten so server.js is at REMOTE_DIR root (matches deploy/systemd/avrag-frontend.service)
if [[ ! -f "$FE_DIR/.next/standalone/server.js" ]]; then
  # monorepo nesting: find server.js under standalone
  SERVER_SRC="$(find "$FE_DIR/.next/standalone" -name server.js | grep -v node_modules | head -1)"
  [[ -n "$SERVER_SRC" ]] || die "standalone server.js not found"
  STANDALONE_ROOT="$(dirname "$SERVER_SRC")"
else
  STANDALONE_ROOT="$FE_DIR/.next/standalone"
fi

cp -a "$STANDALONE_ROOT"/. "$STAGE/"
mkdir -p "$STAGE/.next"
cp -a "$FE_DIR/.next/static" "$STAGE/.next/static"
if [[ -d "$FE_DIR/public" ]]; then
  cp -a "$FE_DIR/public" "$STAGE/public"
fi
[[ -f "$STAGE/server.js" ]] || die "packaged tree missing server.js"

# Metadata for VPS
cat > "$STAGE/DEPLOY_META.json" <<EOF
{
  "component": "frontend",
  "git_rev": "${REV}",
  "dirty": $([[ -n "$DIRTY" ]] && echo true || echo false),
  "release_id": "${RELEASE_ID}",
  "built_at": "${BUILT_AT}",
  "host": "$(hostname -s 2>/dev/null || echo local)"
}
EOF

TGZ="/tmp/avrag-frontend-${RELEASE_ID//\//-}.tgz"
tar czf "$TGZ" -C "$STAGE" .
echo "deploy-frontend: upload $(du -h "$TGZ" | awk '{print $1}')"

"${SCP[@]}" "$TGZ" "${VPS_MAIN_USER}@${VPS_MAIN_HOST}:/tmp/avrag-frontend-deploy.tgz"
"${SCP[@]}" "$ROOT/deploy/systemd/avrag-frontend.service" \
  "${VPS_MAIN_USER}@${VPS_MAIN_HOST}:/tmp/avrag-frontend.service"

"${SSH[@]}" bash -s <<REMOTE
set -euo pipefail
REMOTE_DIR="$REMOTE_DIR"
RELEASE_ID="$RELEASE_ID"
BUILT_AT="$BUILT_AT"
REV="$REV"

systemctl stop avrag-frontend.service 2>/dev/null || true
rm -rf "\$REMOTE_DIR"
mkdir -p "\$REMOTE_DIR"
tar xzf /tmp/avrag-frontend-deploy.tgz -C "\$REMOTE_DIR"
install -m 644 /tmp/avrag-frontend.service /etc/systemd/system/avrag-frontend.service
systemctl daemon-reload
systemctl enable avrag-frontend.service
systemctl start avrag-frontend.service
sleep 2
systemctl is-active avrag-frontend.service

# DEPLOYED.txt append/update frontend line
META=/opt/avrag-rs/DEPLOYED.txt
{
  echo "frontend_rev=\$REV"
  echo "frontend_release_id=\$RELEASE_ID"
  echo "frontend_built_at=\$BUILT_AT"
  echo "frontend_deployed_at=\$(date -u +%Y-%m-%dT%H:%M:%SZ)"
} >> "\$META"
# Keep last 40 lines
tail -n 40 "\$META" > "\$META.tmp" && mv "\$META.tmp" "\$META"

curl -sS -m 8 -o /dev/null -w "local_fe:%{http_code}\\n" http://127.0.0.1:3001/ || true
curl -sS -m 8 -o /dev/null -w "pub_desktop:%{http_code}\\n" https://app.contextlm.top/desktop || true
curl -sS -m 8 -o /dev/null -w "pub_login:%{http_code}\\n" https://app.contextlm.top/login || true
test -f "\$REMOTE_DIR/server.js"
test -f "\$REMOTE_DIR/DEPLOY_META.json"
echo "deploy-frontend: remote OK"
REMOTE

rm -f "$TGZ"
echo "deploy-frontend: done rev=${RELEASE_ID}"
