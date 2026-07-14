#!/usr/bin/env bash
# Build AVRag Desktop Windows NSIS installer (setup.exe).
# Prefer running on Windows; on Ubuntu/WSL needs: mingw-w64, nsis, rust target x86_64-pc-windows-gnu.
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
DESKTOP="$ROOT/desktop"
FE="$ROOT/frontend_next"
TARGET="${TAURI_WINDOWS_TARGET:-x86_64-pc-windows-gnu}"
SKIP_FRONTEND="${SKIP_FRONTEND:-0}"

die() { echo "build-windows: $*" >&2; exit 1; }
log() { echo "build-windows: $*"; }

command -v x86_64-w64-mingw32-gcc >/dev/null 2>&1 || die "mingw-w64 missing (sudo apt-get install -y mingw-w64)"
command -v makensis >/dev/null 2>&1 || die "makensis missing (sudo apt-get install -y nsis)"
command -v rustup >/dev/null 2>&1 || die "rustup missing"

# Cargo linker for gnu target (idempotent block)
mkdir -p "${CARGO_HOME:-$HOME/.cargo}"
CFG="${CARGO_HOME:-$HOME/.cargo}/config.toml"
if ! grep -q '\[target\.x86_64-pc-windows-gnu\]' "$CFG" 2>/dev/null; then
  cat >> "$CFG" <<'EOF'

[target.x86_64-pc-windows-gnu]
linker = "x86_64-w64-mingw32-gcc"
EOF
fi

rustup target add "$TARGET" >/dev/null

SKIP_SIDECARS="${SKIP_SIDECARS:-0}"
if [[ "$SKIP_SIDECARS" != "1" ]]; then
  log "staging Windows product sidecars (avrag-api / avrag-worker) for NSIS externalBin…"
  export USERPROFILE="${USERPROFILE:-${HOME}/.cache/context-osv6/win-userprofile}"
  mkdir -p "$USERPROFILE"
  STAGE_TARGET_TRIPLE="$TARGET" STAGE_BUILD="${STAGE_BUILD:-1}" \
    bash "$ROOT/scripts/stage-desktop-sidecars.sh"
  API_BIN="$DESKTOP/src-tauri/binaries/avrag-api-${TARGET}.exe"
  WORKER_BIN="$DESKTOP/src-tauri/binaries/avrag-worker-${TARGET}.exe"
  [[ -f "$API_BIN" ]] || die "missing $API_BIN after stage-desktop-sidecars"
  [[ -f "$WORKER_BIN" ]] || die "missing $WORKER_BIN after stage-desktop-sidecars"
  log "sidecars: $API_BIN + $WORKER_BIN"
else
  log "SKIP_SIDECARS=1 — NSIS will not embed avrag-api/worker"
fi

if [[ "$SKIP_FRONTEND" != "1" ]]; then
  log "building frontend static export (BUILD_TARGET=desktop)…"
  (
    cd "$FE"
    export NEXT_TELEMETRY_DISABLED=1
    export BUILD_TARGET=desktop
    pnpm build:desktop
  )
else
  log "SKIP_FRONTEND=1 — expecting $FE/out"
  [[ -d "$FE/out" ]] || die "frontend_next/out missing"
fi

# Merge Tauri config: embed sidecars as externalBin (next to app after install)
# and ship compose/README as resources. Only applied for this Windows build so
# host linux/mac builds stay free of missing externalBin requirements.
TAURI_EXTRA_CONFIG='{"bundle":{"externalBin":["binaries/avrag-api","binaries/avrag-worker"],"resources":["../runtime/docker-compose.client.yml","../runtime/README.md"]}}'
if [[ "$SKIP_SIDECARS" == "1" ]]; then
  TAURI_EXTRA_CONFIG='{"bundle":{"resources":["../runtime/docker-compose.client.yml","../runtime/README.md"]}}'
fi

log "tauri build --target $TARGET --bundles nsis (+ sidecars)"
(
  cd "$DESKTOP"
  export CI=true
  export CARGO_BUILD_JOBS="${CARGO_BUILD_JOBS:-2}"
  # Ensure local cli
  if [[ ! -x node_modules/.bin/tauri ]]; then
    CI=true pnpm install
  fi
  if [[ "$SKIP_FRONTEND" == "1" ]]; then
    pnpm tauri build --target "$TARGET" --bundles nsis \
      --config '{"build":{"beforeBuildCommand":""}}' \
      --config "$TAURI_EXTRA_CONFIG"
  else
    pnpm tauri build --target "$TARGET" --bundles nsis \
      --config "$TAURI_EXTRA_CONFIG"
  fi
)

# Locate setup.exe — prefer Context-OS_* (newest product name), then newest mtime
NSIS_DIR="$DESKTOP/src-tauri/target/${TARGET}/release/bundle/nsis"
SETUP=""
if [[ -f "$NSIS_DIR/Context-OS_0.1.0_x64-setup.exe" ]]; then
  SETUP="$NSIS_DIR/Context-OS_0.1.0_x64-setup.exe"
elif [[ -f "$NSIS_DIR/Context-OS_${VERSION:-0.1.0}_x64-setup.exe" ]]; then
  SETUP="$NSIS_DIR/Context-OS_${VERSION}_x64-setup.exe"
fi
if [[ -z "$SETUP" || ! -f "$SETUP" ]]; then
  SETUP="$(find "$NSIS_DIR" -type f -name 'Context-OS*-setup.exe' 2>/dev/null | head -1 || true)"
fi
if [[ -z "$SETUP" || ! -f "$SETUP" ]]; then
  # newest setup by mtime
  SETUP="$(find "$NSIS_DIR" -type f -name '*-setup.exe' -printf '%T@ %p\n' 2>/dev/null | sort -nr | head -1 | cut -d' ' -f2- || true)"
fi
if [[ -z "$SETUP" || ! -f "$SETUP" ]]; then
  SETUP="$(find "$DESKTOP/src-tauri/target" -type f -name 'Context-OS*-setup.exe' 2>/dev/null | head -1 || true)"
fi

[[ -n "$SETUP" && -f "$SETUP" ]] || die "NSIS setup.exe not produced under $NSIS_DIR (check tauri build logs)"

# Verify sidecars were linked into the release tree (Tauri places externalBin next to the exe).
RELEASE_DIR="$DESKTOP/src-tauri/target/${TARGET}/release"
if [[ "$SKIP_SIDECARS" != "1" ]]; then
  if [[ -f "$RELEASE_DIR/avrag-api.exe" || -f "$RELEASE_DIR/avrag-api-${TARGET}.exe" ]]; then
    log "release tree contains avrag-api sidecar"
  else
    log "warning: avrag-api.exe not found next to release exe — check externalBin staging"
    ls -la "$RELEASE_DIR"/*.exe 2>/dev/null | head -20 || true
  fi
fi

log "OK setup: $SETUP ($(du -h "$SETUP" | awk '{print $1}'))"
log "next:"
log "  bash scripts/package-desktop-release.sh"
log "  bash scripts/publish-desktop-release.sh"
