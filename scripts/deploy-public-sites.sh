#!/usr/bin/env bash
# Build & publish public satellite sites to VPS (artifacts only).
# Sites live outside this monorepo; paths default to sibling dirs under $HOME.
#
# Usage:
#   bash scripts/deploy-public-sites.sh              # all: landing why canju
#   bash scripts/deploy-public-sites.sh landing why  # subset
#   SITES=landing,canju bash scripts/deploy-public-sites.sh
#
# Env (optional, avrag-rs/.env or shell):
#   LANDING_DIR  default $HOME/context-os-landing
#   WHY_DIR      default $HOME/whyiamright
#   CCHESS_DIR   default $HOME/cchess
#   SKIP_BUILD=1 use existing build outputs
#   WHY_API=0    skip why-api binary (frontend only)
#   CANJU_SERVER=1 also rebuild/upload cchess server binary (default off)
#   APPLY_NGINX=1 rsync deploy/nginx/{canju,context-os-landing,whyiamright}.conf + reload
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
ENV_FILE="$ROOT/avrag-rs/.env"

die() { echo "deploy-public-sites: $*" >&2; exit 1; }
log() { echo "deploy-public-sites: $*"; }

if [[ -f "$ENV_FILE" ]]; then
  set -a
  # shellcheck disable=SC1090
  source "$ENV_FILE"
  set +a
fi

: "${VPS_MAIN_HOST:?set VPS_MAIN_HOST in avrag-rs/.env}"
: "${VPS_MAIN_USER:?set VPS_MAIN_USER}"
: "${VPS_MAIN_PASSWORD:?set VPS_MAIN_PASSWORD}"

LANDING_DIR="${LANDING_DIR:-$HOME/context-os-landing}"
WHY_DIR="${WHY_DIR:-$HOME/whyiamright}"
CCHESS_DIR="${CCHESS_DIR:-$HOME/cchess}"
SKIP_BUILD="${SKIP_BUILD:-0}"
WHY_API="${WHY_API:-1}"
CANJU_SERVER="${CANJU_SERVER:-0}"
APPLY_NGINX="${APPLY_NGINX:-0}"

SSH=(sshpass -p "$VPS_MAIN_PASSWORD" ssh -o StrictHostKeyChecking=no "${VPS_MAIN_USER}@${VPS_MAIN_HOST}")
SCP=(sshpass -p "$VPS_MAIN_PASSWORD" scp -o StrictHostKeyChecking=no)
RSYNC=(sshpass -p "$VPS_MAIN_PASSWORD" rsync -az --delete -e "ssh -o StrictHostKeyChecking=no")

cd "$ROOT"
MONO_REV="$(git rev-parse --short HEAD 2>/dev/null || echo unknown)"
DIRTY=""
if ! git diff --quiet 2>/dev/null || ! git diff --cached --quiet 2>/dev/null; then
  DIRTY="+dirty"
fi
RELEASE_ID="${MONO_REV}${DIRTY}"
BUILT_AT="$(date -u +%Y-%m-%dT%H:%M:%SZ)"

# Resolve site list
if [[ $# -gt 0 ]]; then
  SITES=("$@")
elif [[ -n "${SITES:-}" ]]; then
  IFS=',' read -r -a SITES <<< "$SITES"
else
  SITES=(landing why canju)
fi

site_rev() {
  local dir="$1"
  if [[ -d "$dir/.git" ]]; then
    git -C "$dir" rev-parse --short HEAD 2>/dev/null || echo unknown
  else
    echo nagit
  fi
}

npm_or_pnpm_build() {
  local dir="$1"
  (
    cd "$dir"
    export NEXT_TELEMETRY_DISABLED=1
    if [[ -f pnpm-lock.yaml ]] && command -v pnpm >/dev/null 2>&1; then
      pnpm install --frozen-lockfile 2>/dev/null || pnpm install
      pnpm build
    elif [[ -f package-lock.json ]]; then
      npm ci 2>/dev/null || npm install
      npm run build
    else
      npm install
      npm run build
    fi
  )
}

append_deployed() {
  local lines="$1"
  "${SSH[@]}" bash -s <<REMOTE
set -euo pipefail
META=/opt/avrag-rs/DEPLOYED.txt
mkdir -p /opt/avrag-rs
{
$lines
} >> "\$META"
tail -n 60 "\$META" > "\$META.tmp" && mv "\$META.tmp" "\$META"
REMOTE
}

# ---------- landing (static Next export → /var/www/context-os-landing) ----------
deploy_landing() {
  log "=== landing ==="
  [[ -d "$LANDING_DIR" ]] || die "LANDING_DIR missing: $LANDING_DIR"
  local srev
  srev="$(site_rev "$LANDING_DIR")"
  if [[ "$SKIP_BUILD" != "1" ]]; then
    log "building landing (static export)…"
    npm_or_pnpm_build "$LANDING_DIR"
  fi
  local dist="$LANDING_DIR/dist"
  [[ -f "$dist/index.html" ]] || die "landing dist missing index.html (run build)"

  "${SSH[@]}" "mkdir -p /var/www/context-os-landing"
  "${RSYNC[@]}" "$dist/" "${VPS_MAIN_USER}@${VPS_MAIN_HOST}:/var/www/context-os-landing/"
  "${SSH[@]}" "chmod -R a+rX /var/www/context-os-landing"

  cat > /tmp/landing-DEPLOY_META.json <<EOF
{"component":"landing","site_rev":"$srev","mono_rev":"$MONO_REV","release_id":"$RELEASE_ID","built_at":"$BUILT_AT"}
EOF
  "${SCP[@]}" /tmp/landing-DEPLOY_META.json \
    "${VPS_MAIN_USER}@${VPS_MAIN_HOST}:/var/www/context-os-landing/DEPLOY_META.json"

  append_deployed "  echo \"landing_site_rev=$srev\"
  echo \"landing_mono_rev=$MONO_REV\"
  echo \"landing_release_id=$RELEASE_ID\"
  echo \"landing_deployed_at=\$(date -u +%Y-%m-%dT%H:%M:%SZ)\""

  local code
  code="$("${SSH[@]}" 'curl -sS -m 8 -o /dev/null -w "%{http_code}" https://contextlm.top/ || true')"
  log "landing pub_home:$code"
  [[ "$code" == "200" ]] || log "warn: landing HTTP $code (check CF/DNS)"
}

# ---------- why (standalone FE + optional Go API) ----------
deploy_why() {
  log "=== why ==="
  [[ -d "$WHY_DIR" ]] || die "WHY_DIR missing: $WHY_DIR"
  local srev fe_dir be_dir
  srev="$(site_rev "$WHY_DIR")"
  fe_dir="$WHY_DIR/frontend"
  be_dir="$WHY_DIR/backend"
  [[ -d "$fe_dir" ]] || die "why frontend missing: $fe_dir"

  if [[ "$SKIP_BUILD" != "1" ]]; then
    log "building why frontend (standalone)…"
    npm_or_pnpm_build "$fe_dir"
  fi

  local stage
  stage="$(mktemp -d /tmp/why-fe-deploy.XXXXXX)"
  # Flatten standalone so server.js is at frontend root (matches deploy/systemd/why-frontend.service)
  local server_src standalone_root
  if [[ -f "$fe_dir/.next/standalone/server.js" ]]; then
    standalone_root="$fe_dir/.next/standalone"
  else
    server_src="$(find "$fe_dir/.next/standalone" -name server.js 2>/dev/null | grep -v node_modules | head -1 || true)"
    [[ -n "$server_src" ]] || die "why standalone server.js not found — build first"
    standalone_root="$(dirname "$server_src")"
  fi
  cp -a "$standalone_root"/. "$stage/"
  mkdir -p "$stage/.next"
  [[ -d "$fe_dir/.next/static" ]] && cp -a "$fe_dir/.next/static" "$stage/.next/static"
  [[ -d "$fe_dir/public" ]] && cp -a "$fe_dir/public" "$stage/public"
  [[ -f "$stage/server.js" ]] || die "why packaged tree missing server.js"

  local tgz="/tmp/why-frontend-${RELEASE_ID//\//-}.tgz"
  tar czf "$tgz" -C "$stage" .
  rm -rf "$stage"

  "${SCP[@]}" "$tgz" "${VPS_MAIN_USER}@${VPS_MAIN_HOST}:/tmp/why-frontend-deploy.tgz"
  "${SCP[@]}" "$ROOT/deploy/systemd/why-frontend.service" \
    "${VPS_MAIN_USER}@${VPS_MAIN_HOST}:/tmp/why-frontend.service"
  "${SCP[@]}" "$ROOT/deploy/systemd/why-api.service" \
    "${VPS_MAIN_USER}@${VPS_MAIN_HOST}:/tmp/why-api.service"

  if [[ "$WHY_API" == "1" ]]; then
    local api_bin="$be_dir/why-api"
    if [[ "$SKIP_BUILD" != "1" ]]; then
      log "building why-api (go)…"
      (
        cd "$be_dir"
        go build -o why-api ./cmd/api 2>/dev/null || go build -o why-api .
      )
    fi
    [[ -x "$api_bin" ]] || die "missing $api_bin"
    "${SCP[@]}" "$api_bin" "${VPS_MAIN_USER}@${VPS_MAIN_HOST}:/tmp/why-api.bin"
  fi

  "${SSH[@]}" bash -s <<REMOTE
set -euo pipefail
systemctl stop why-frontend.service 2>/dev/null || true
rm -rf /opt/whyiamright/frontend
mkdir -p /opt/whyiamright/frontend /opt/whyiamright/bin
tar xzf /tmp/why-frontend-deploy.tgz -C /opt/whyiamright/frontend
test -f /opt/whyiamright/frontend/server.js

if [[ -f /tmp/why-api.bin ]]; then
  install -m 755 /tmp/why-api.bin /opt/whyiamright/bin/why-api
fi

install -m 644 /tmp/why-frontend.service /etc/systemd/system/why-frontend.service
install -m 644 /tmp/why-api.service /etc/systemd/system/why-api.service
# Ensure PORT=8082 in env file (cchess owns 8080)
if [[ -f /opt/whyiamright/why-backend.env ]]; then
  if grep -q '^PORT=' /opt/whyiamright/why-backend.env; then
    sed -i 's/^PORT=.*/PORT=8082/' /opt/whyiamright/why-backend.env
  else
    echo 'PORT=8082' >> /opt/whyiamright/why-backend.env
  fi
fi
systemctl daemon-reload
systemctl enable why-api.service why-frontend.service
systemctl restart why-api.service
systemctl restart why-frontend.service
sleep 2
systemctl is-active why-api.service
systemctl is-active why-frontend.service
curl -sS -m 8 -o /dev/null -w "why_fe:%{http_code}\\n" http://127.0.0.1:3004/ || true
curl -sS -m 8 -o /dev/null -w "why_api:%{http_code}\\n" http://127.0.0.1:8082/health 2>/dev/null || \
  curl -sS -m 8 -o /dev/null -w "why_api:%{http_code}\\n" http://127.0.0.1:8082/ || true
REMOTE

  rm -f "$tgz" /tmp/landing-DEPLOY_META.json 2>/dev/null || true

  append_deployed "  echo \"why_site_rev=$srev\"
  echo \"why_mono_rev=$MONO_REV\"
  echo \"why_release_id=$RELEASE_ID\"
  echo \"why_deployed_at=\$(date -u +%Y-%m-%dT%H:%M:%SZ)\""

  local code
  code="$("${SSH[@]}" 'curl -sS -m 8 -o /dev/null -w "%{http_code}" https://whyimright.contextlm.top/ || true')"
  log "why pub_home:$code"
}

# ---------- canju (vite static + optional cchess server) ----------
deploy_canju() {
  log "=== canju ==="
  [[ -d "$CCHESS_DIR" ]] || die "CCHESS_DIR missing: $CCHESS_DIR"
  local srev fe_dir
  srev="$(site_rev "$CCHESS_DIR")"
  fe_dir="$CCHESS_DIR/frontend"
  [[ -d "$fe_dir" ]] || die "cchess frontend missing: $fe_dir"

  if [[ "$SKIP_BUILD" != "1" ]]; then
    log "building canju frontend…"
    npm_or_pnpm_build "$fe_dir"
  fi
  [[ -f "$fe_dir/dist/index.html" ]] || die "canju dist missing index.html"

  "${SSH[@]}" "mkdir -p /var/www/canju"
  "${RSYNC[@]}" "$fe_dir/dist/" "${VPS_MAIN_USER}@${VPS_MAIN_HOST}:/var/www/canju/"
  "${SSH[@]}" "chmod -R a+rX /var/www/canju"

  if [[ "$CANJU_SERVER" == "1" ]]; then
    log "building cchess server (cmake)…"
    if [[ "$SKIP_BUILD" != "1" ]]; then
      (
        cd "$CCHESS_DIR"
        cmake -S . -B build -G Ninja -DCMAKE_BUILD_TYPE=Release
        cmake --build build --target server
      )
    fi
    local srv="$CCHESS_DIR/build/backend/server"
    [[ -x "$srv" ]] || srv="$CCHESS_DIR/build/server"
    [[ -x "$srv" ]] || die "cchess server binary not found"
    "${SCP[@]}" "$srv" "${VPS_MAIN_USER}@${VPS_MAIN_HOST}:/tmp/cchess-server.bin"
    if [[ -f "$CCHESS_DIR/deploy/cchess.service" ]]; then
      "${SCP[@]}" "$CCHESS_DIR/deploy/cchess.service" \
        "${VPS_MAIN_USER}@${VPS_MAIN_HOST}:/tmp/cchess.service"
    fi
    "${SSH[@]}" bash -s <<'REMOTE'
set -euo pipefail
mkdir -p /opt/cchess
install -m 755 /tmp/cchess-server.bin /opt/cchess/server
if [[ -f /tmp/cchess.service ]]; then
  install -m 644 /tmp/cchess.service /etc/systemd/system/cchess.service
  systemctl daemon-reload
  systemctl enable cchess.service
  systemctl restart cchess.service || true
fi
# if not under systemd, leave existing process alone
REMOTE
  fi

  append_deployed "  echo \"canju_site_rev=$srev\"
  echo \"canju_mono_rev=$MONO_REV\"
  echo \"canju_release_id=$RELEASE_ID\"
  echo \"canju_deployed_at=\$(date -u +%Y-%m-%dT%H:%M:%SZ)\""

  local code
  code="$("${SSH[@]}" 'curl -sS -m 8 -o /dev/null -w "%{http_code}" https://canju.contextlm.top/ || true')"
  log "canju pub_home:$code"
}

apply_nginx_if_requested() {
  [[ "$APPLY_NGINX" == "1" ]] || return 0
  log "=== apply nginx confs from deploy/nginx ==="
  local f
  for f in canju.conf context-os-landing.conf whyiamright.conf; do
    [[ -f "$ROOT/deploy/nginx/$f" ]] || continue
    "${SCP[@]}" "$ROOT/deploy/nginx/$f" \
      "${VPS_MAIN_USER}@${VPS_MAIN_HOST}:/tmp/$f"
    "${SSH[@]}" "install -m 644 /tmp/$f /etc/nginx/conf.d/$f"
  done
  "${SSH[@]}" 'nginx -t && systemctl reload nginx'
}

# ---------- main ----------
log "mono_rev=${RELEASE_ID} sites=${SITES[*]}"
log "LANDING_DIR=$LANDING_DIR"
log "WHY_DIR=$WHY_DIR"
log "CCHESS_DIR=$CCHESS_DIR"

for site in "${SITES[@]}"; do
  case "$site" in
    landing) deploy_landing ;;
    why)     deploy_why ;;
    canju)   deploy_canju ;;
    all)
      deploy_landing
      deploy_why
      deploy_canju
      ;;
    *) die "unknown site: $site (landing|why|canju)" ;;
  esac
done

apply_nginx_if_requested
log "done"
