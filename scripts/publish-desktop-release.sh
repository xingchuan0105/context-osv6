#!/usr/bin/env bash
# Upload dist/desktop-release to VPS /var/www/releases/desktop/
# Credentials: avrag-rs/.env VPS_MAIN_* (same as other deploy scripts)
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
OUT_ROOT="${DESKTOP_RELEASE_OUT:-$ROOT/dist/desktop-release}"
ENV_FILE="$ROOT/avrag-rs/.env"

die() { echo "publish-desktop-release: $*" >&2; exit 1; }

[[ -d "$OUT_ROOT" ]] || die "missing $OUT_ROOT (run package-desktop-release.sh first)"
[[ -f "$OUT_ROOT/latest.json" ]] || die "missing $OUT_ROOT/latest.json"

if [[ -f "$ENV_FILE" ]]; then
  set -a
  # shellcheck disable=SC1090
  source "$ENV_FILE"
  set +a
fi

: "${VPS_MAIN_HOST:?set VPS_MAIN_HOST}"
: "${VPS_MAIN_USER:?set VPS_MAIN_USER}"
: "${VPS_MAIN_PASSWORD:?set VPS_MAIN_PASSWORD}"

REMOTE_ROOT="/var/www/releases/desktop"
SSH=(sshpass -p "$VPS_MAIN_PASSWORD" ssh -o StrictHostKeyChecking=no "${VPS_MAIN_USER}@${VPS_MAIN_HOST}")
RSYNC=(sshpass -p "$VPS_MAIN_PASSWORD" rsync -az --info=progress2 -e "ssh -o StrictHostKeyChecking=no")

echo "publish-desktop-release: → ${VPS_MAIN_USER}@${VPS_MAIN_HOST}:${REMOTE_ROOT}"

"${SSH[@]}" "mkdir -p '$REMOTE_ROOT'"

# Sync version directories + latest.json
"${RSYNC[@]}" "$OUT_ROOT/" "${VPS_MAIN_USER}@${VPS_MAIN_HOST}:${REMOTE_ROOT}/"

# Permissions for nginx
"${SSH[@]}" "chmod -R a+rX '$REMOTE_ROOT'"

# Smoke
VERSION="$(node -p "JSON.parse(require('fs').readFileSync('$OUT_ROOT/latest.json','utf8')).version")"
echo "publish-desktop-release: smoke latest.json"
"${SSH[@]}" "curl -sS -m 5 -o /dev/null -w 'latest:%{http_code}\n' http://127.0.0.1/releases/desktop/latest.json 2>/dev/null || curl -sS -m 5 file://$REMOTE_ROOT/latest.json >/dev/null; test -f $REMOTE_ROOT/latest.json && echo ok_local_file"

echo "publish-desktop-release: done v${VERSION}"
echo "  public path: /releases/desktop/latest.json"
echo "  version dir: /releases/desktop/v${VERSION}/"
