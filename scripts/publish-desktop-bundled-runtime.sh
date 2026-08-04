#!/usr/bin/env bash
# Upload dist/desktop-runtime → VPS /var/www/releases/desktop/runtime/
# Credentials: avrag-rs/.env VPS_MAIN_* (same as publish-desktop-release.sh)
#
# Prerequisites:
#   bash scripts/stage-desktop-bundled-runtime.sh pack
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
OUT_ROOT="${DESKTOP_RUNTIME_OUT:-$ROOT/dist/desktop-runtime}"
ENV_FILE="$ROOT/avrag-rs/.env"
REMOTE_ROOT="/var/www/releases/desktop/runtime"

die() { echo "publish-desktop-bundled-runtime: $*" >&2; exit 1; }
log() { echo "publish-desktop-bundled-runtime: $*"; }

[[ -d "$OUT_ROOT" ]] || die "missing $OUT_ROOT (run stage-desktop-bundled-runtime.sh pack first)"
[[ -f "$OUT_ROOT/manifest.json" ]] || die "missing $OUT_ROOT/manifest.json"

if [[ -f "$ENV_FILE" ]]; then
  set -a
  # shellcheck disable=SC1090
  source "$ENV_FILE"
  set +a
fi

: "${VPS_MAIN_HOST:?set VPS_MAIN_HOST}"
: "${VPS_MAIN_USER:?set VPS_MAIN_USER}"
: "${VPS_MAIN_PASSWORD:?set VPS_MAIN_PASSWORD}"

SSH=(sshpass -p "$VPS_MAIN_PASSWORD" ssh -o StrictHostKeyChecking=no "${VPS_MAIN_USER}@${VPS_MAIN_HOST}")
RSYNC=(sshpass -p "$VPS_MAIN_PASSWORD" rsync -az --info=progress2 -e "ssh -o StrictHostKeyChecking=no")

log "→ ${VPS_MAIN_USER}@${VPS_MAIN_HOST}:${REMOTE_ROOT}"
"${SSH[@]}" "mkdir -p '$REMOTE_ROOT'"

# Sync entire dist tree (manifest.json + platform zips + .sha256)
"${RSYNC[@]}" "$OUT_ROOT/" "${VPS_MAIN_USER}@${VPS_MAIN_HOST}:${REMOTE_ROOT}/"

"${SSH[@]}" "chmod -R a+rX '$REMOTE_ROOT'"

# Smoke: local file + optional HTTP via nginx
log "smoke"
"${SSH[@]}" "test -f '$REMOTE_ROOT/manifest.json' && echo ok_manifest"
if command -v node >/dev/null 2>&1; then
  RID="$(node -p "JSON.parse(require('fs').readFileSync('$OUT_ROOT/manifest.json','utf8')).runtime_id" 2>/dev/null || echo unknown)"
else
  RID=unknown
fi

log "done runtime_id=${RID}"
log "  public: https://app.contextlm.top/releases/desktop/runtime/manifest.json"
log "  tree:   /var/www/releases/desktop/runtime/"
log "builder fetch: bash scripts/stage-desktop-bundled-runtime.sh fetch"
