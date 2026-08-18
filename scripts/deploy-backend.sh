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

# Optional: propagate the platform price rates JSON into the VPS env file.
# base64 survives ssh/heredoc quoting; the remote side writes it as an
# unquoted single line (docker --env-file compatible).
RATES_JSON_B64=""
if [[ -n "${PLATFORM_OFFICIAL_RATES_JSON:-}" ]]; then
  RATES_JSON_B64="$(printf '%s' "$PLATFORM_OFFICIAL_RATES_JSON" | base64 -w0)"
fi

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
[[ -d "$AVRAG_DIR/modes" ]] || die "missing modes/"

mkdir -p "$STAGE/bin" "$STAGE/migrations" "$STAGE/prompts" "$STAGE/modes" "$STAGE/docker" "$STAGE/scripts"

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

# Agent mode YAML (chat/rag/search/write_refine) — required at runtime CWD /opt/avrag-rs
rsync -a --delete \
  --include '*/' \
  --include '*.yaml' \
  --exclude '*' \
  "$AVRAG_DIR/modes/" "$STAGE/modes/"

cp -a "$ROOT/deploy/docker/run-avrag-containers.sh" "$STAGE/docker/run-avrag-containers.sh"
chmod 755 "$STAGE/docker/run-avrag-containers.sh"
cp -a "$ROOT/deploy/docker/avrag-runtime.Dockerfile" "$STAGE/docker/avrag-runtime.Dockerfile"

# anydoc-extract package (baked into avrag-runtime image).
rsync -a --delete \
  --exclude '__pycache__/' \
  --exclude '*.egg-info/' \
  --exclude '.pytest_cache/' \
  "$AVRAG_DIR/scripts/anydoc-extract/" "$STAGE/scripts/anydoc-extract/"

# lit (liteparse PDF CLI, --no-default-features build) + official pdfium lib —
# baked into avrag-runtime so the worker can parse PDFs. Local build artifacts:
#   lit:        cargo install liteparse --version 2.10.0 --no-default-features
#   libpdfium:  ~/.cache/pdfium-rs/chromium_7897/pdfium-linux-x64/lib/libpdfium.so
LIT_BIN="${LIT_BIN:-$HOME/e2e-vps-cargo/bin/lit}"
PDFIUM_LIB="${PDFIUM_LIB:-$HOME/.cache/pdfium-rs/chromium_7897/pdfium-linux-x64/lib/libpdfium.so}"
[[ -x "$LIT_BIN" ]] || die "missing lit binary at $LIT_BIN (set LIT_BIN)"
[[ -f "$PDFIUM_LIB" ]] || die "missing libpdfium.so at $PDFIUM_LIB (set PDFIUM_LIB)"
cp -a "$LIT_BIN" "$STAGE/docker/lit"
cp -a "$PDFIUM_LIB" "$STAGE/docker/libpdfium.so"

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
RATES_JSON_B64="$RATES_JSON_B64"

STAGE=/tmp/avrag-backend-stage
rm -rf "\$STAGE"
mkdir -p "\$STAGE"
tar xzf /tmp/avrag-backend-deploy.tgz -C "\$STAGE"

mkdir -p "\$REMOTE_ROOT/bin" "\$REMOTE_ROOT/migrations" "\$REMOTE_ROOT/prompts" "\$REMOTE_ROOT/modes" "\$REMOTE_ROOT/docker"

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
rsync -a --delete \
  "\$STAGE/modes/" "\$REMOTE_ROOT/modes/"

install -m 755 "\$STAGE/docker/run-avrag-containers.sh" "\$REMOTE_ROOT/docker/run-avrag-containers.sh"
install -m 644 "\$STAGE/DEPLOY_META.backend.json" "\$REMOTE_ROOT/DEPLOY_META.backend.json"

# Rebuild avrag-runtime so the worker container has parser CLIs (markitdown /
# anydoc-extract). Host-path venvs are NOT visible inside the minimal runtime;
# bake tools into the image instead. markitdown CLI must not be probed with
# `--help` (it treats unknown flags as convert inputs).
echo "deploy-backend: rebuilding avrag-runtime:24.04 with parser CLIs"
RUNTIME_BUILD=/tmp/avrag-runtime-build
rm -rf "\$RUNTIME_BUILD"
mkdir -p "\$RUNTIME_BUILD"
cp "\$STAGE/docker/avrag-runtime.Dockerfile" "\$RUNTIME_BUILD/Dockerfile"
if [[ ! -d "\$STAGE/scripts/anydoc-extract" ]]; then
  echo "deploy-backend: ERROR stage missing scripts/anydoc-extract" >&2
  exit 1
fi
cp -a "\$STAGE/scripts/anydoc-extract" "\$RUNTIME_BUILD/anydoc-extract"
cp -a "\$STAGE/docker/lit" "\$RUNTIME_BUILD/lit"
cp -a "\$STAGE/docker/libpdfium.so" "\$RUNTIME_BUILD/libpdfium.so"
docker build -t avrag-runtime:24.04 "\$RUNTIME_BUILD"
docker run --rm avrag-runtime:24.04 bash -lc \
  'command -v markitdown && markitdown -h >/dev/null && command -v anydoc-extract && command -v lit && ldconfig -p | grep -q libpdfium' \
  || { echo "deploy-backend: ERROR runtime image missing parser CLIs" >&2; exit 1; }
echo "deploy-backend: runtime image OK (markitdown+anydoc-extract+lit+pdfium)"

# Point env at in-image binaries.
if [[ -f /etc/avrag-rs/avrag.env ]]; then
  export RATES_JSON_B64
  python3 - <<'PY'
import base64, os
from pathlib import Path
p = Path("/etc/avrag-rs/avrag.env")
text = p.read_text()
updates = {
    "MARKITDOWN_BIN": "markitdown",
    "ANYDOC_BIN": "anydoc-extract",
}
rates_b64 = os.environ.get("RATES_JSON_B64", "").strip()
if rates_b64:
    updates["PLATFORM_OFFICIAL_RATES_JSON"] = base64.b64decode(rates_b64).decode()
lines = text.splitlines()
out = []
seen = set()
for line in lines:
    if not line.strip() or line.lstrip().startswith("#") or "=" not in line:
        out.append(line)
        continue
    k, v = line.split("=", 1)
    if k in updates:
        out.append(f"{k}={updates[k]}")
        seen.add(k)
    else:
        out.append(line)
for k, v in updates.items():
    if k not in seen:
        out.append(f"{k}={v}")
p.write_text("\\n".join(out) + "\\n")
print("deploy-backend: env parser bins -> in-image PATH")
PY
fi

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
