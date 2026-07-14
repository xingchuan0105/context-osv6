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

log "tauri build --target $TARGET --bundles nsis"
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
      --config '{"build":{"beforeBuildCommand":""}}'
  else
    pnpm tauri build --target "$TARGET" --bundles nsis
  fi
)

# Locate setup.exe
NSIS_DIR="$DESKTOP/src-tauri/target/${TARGET}/release/bundle/nsis"
SETUP="$(find "$NSIS_DIR" -type f -name '*-setup.exe' 2>/dev/null | head -1 || true)"
if [[ -z "$SETUP" || ! -f "$SETUP" ]]; then
  # broader search
  SETUP="$(find "$DESKTOP/src-tauri/target" -type f \( -name '*-setup.exe' -o -name '*setup.exe' \) 2>/dev/null | head -1 || true)"
fi

[[ -n "$SETUP" && -f "$SETUP" ]] || die "NSIS setup.exe not produced under $NSIS_DIR (check tauri build logs)"

log "OK setup: $SETUP ($(du -h "$SETUP" | awk '{print $1}'))"
log "next:"
log "  bash scripts/package-desktop-release.sh"
log "  bash scripts/publish-desktop-release.sh"
