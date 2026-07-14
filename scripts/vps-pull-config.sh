#!/usr/bin/env bash
# Pull nginx + systemd units from VPS into deploy/ for diff (no secrets).
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
ENV_FILE="$ROOT/avrag-rs/.env"
OUT="$ROOT/deploy/pulled"
STAMP="$(date +%Y%m%d-%H%M%S)"

if [[ -f "$ENV_FILE" ]]; then
  set -a
  # shellcheck disable=SC1090
  source "$ENV_FILE"
  set +a
fi

: "${VPS_MAIN_HOST:?}"
: "${VPS_MAIN_USER:?}"
: "${VPS_MAIN_PASSWORD:?}"

SSH=(sshpass -p "$VPS_MAIN_PASSWORD" ssh -o StrictHostKeyChecking=no "${VPS_MAIN_USER}@${VPS_MAIN_HOST}")
SCP=(sshpass -p "$VPS_MAIN_PASSWORD" scp -o StrictHostKeyChecking=no)

mkdir -p "$OUT/$STAMP"

echo "vps-pull-config: fetching from ${VPS_MAIN_HOST} → deploy/pulled/${STAMP}"

"${SSH[@]}" 'mkdir -p /tmp/vps-config-export &&
  cp -a /etc/nginx/conf.d/app-contextlm.conf /tmp/vps-config-export/ 2>/dev/null || true
  cp -a /etc/nginx/conf.d/canju.conf /etc/nginx/conf.d/ghost.conf /tmp/vps-config-export/ 2>/dev/null || true
  cp -a /etc/nginx/conf.d/whyiamright.conf /etc/nginx/conf.d/context-os-landing.conf /tmp/vps-config-export/ 2>/dev/null || true
  cp -a /etc/systemd/system/avrag-frontend.service /etc/systemd/system/why-*.service /tmp/vps-config-export/ 2>/dev/null || true
  ls /tmp/vps-config-export'

"${SCP[@]}" -r "${VPS_MAIN_USER}@${VPS_MAIN_HOST}:/tmp/vps-config-export/." "$OUT/$STAMP/"

# Refresh canonical copies under deploy/ when present
if [[ -f "$OUT/$STAMP/avrag-frontend.service" ]]; then
  cp -f "$OUT/$STAMP/avrag-frontend.service" "$ROOT/deploy/systemd/avrag-frontend.service"
fi
if [[ -f "$OUT/$STAMP/app-contextlm.conf" ]]; then
  cp -f "$OUT/$STAMP/app-contextlm.conf" "$ROOT/deploy/nginx/app-contextlm.conf"
fi

# Ignore pulled snapshots in git by default (keep canonical deploy/nginx + systemd tracked)
echo "vps-pull-config: wrote $OUT/$STAMP"
echo "  updated deploy/systemd/avrag-frontend.service and deploy/nginx/app-contextlm.conf if present"
ls -la "$OUT/$STAMP"
