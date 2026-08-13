#!/usr/bin/env bash
# Fast Windows desktop iteration **without** re-running NSIS packaging.
#
# Modes:
#   hotswap  (default)  Build shell (+ optional frontend/sidecars), copy into
#                       %LOCALAPPDATA%\Context-OS Client\  and optionally launch.
#   run                 Build, stage next to release Context-OS.exe under
#                       desktop/src-tauri/target/.../release/, launch from there
#                       (no install dir; needs bundled runtime already staged).
#   shell-only          Only rebuild avrag-desktop / Context-OS.exe (fastest for
#                       lifecycle / ensure_native / IPC changes).
#
# Env:
#   SKIP_FRONTEND=1     Do not rebuild frontend_next/out
#   SKIP_SIDECARS=1     Do not rebuild/stage avrag-api/worker
#   LAUNCH=1            Start Context-OS.exe after copy (default 1 for hotswap)
#   INSTALL_DIR=...     Override install root (default Windows LocalAppData path via WSL)
#   CARGO_BUILD_JOBS=2
#
# Examples (from WSL monorepo root):
#   bash scripts/dev-windows-hotswap.sh shell-only
#   SKIP_FRONTEND=1 bash scripts/dev-windows-hotswap.sh hotswap
#   bash scripts/dev-windows-hotswap.sh run
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
DESKTOP="$ROOT/desktop"
TARGET="${TAURI_WINDOWS_TARGET:-x86_64-pc-windows-gnu}"
MODE="${1:-hotswap}"
SKIP_FRONTEND="${SKIP_FRONTEND:-0}"
SKIP_SIDECARS="${SKIP_SIDECARS:-1}"
LAUNCH="${LAUNCH:-1}"
CARGO_BUILD_JOBS="${CARGO_BUILD_JOBS:-2}"

die() { echo "dev-windows-hotswap: $*" >&2; exit 1; }
log() { echo "dev-windows-hotswap: $*"; }

default_install_dir() {
  # WSL → Windows user LocalAppData\Context-OS Client
  if command -v powershell.exe >/dev/null 2>&1; then
    local p
    p="$(powershell.exe -NoProfile -Command 'Join-Path $env:LOCALAPPDATA "Context-OS Client"' 2>/dev/null | tr -d '\r')"
    if [[ -n "$p" ]]; then
      # Convert C:\Users\... → /mnt/c/Users/...
      if [[ "$p" =~ ^([A-Za-z]):\\ ]]; then
        local drive="${BASH_REMATCH[1],,}"
        local rest="${p:3}"
        rest="${rest//\\//}"
        echo "/mnt/${drive}/${rest}"
        return
      fi
    fi
  fi
  echo "/mnt/c/Users/${WIN_USER:-xingc}/AppData/Local/Context-OS Client"
}

INSTALL_DIR="${INSTALL_DIR:-$(default_install_dir)}"
RELEASE_DIR="$DESKTOP/src-tauri/target/${TARGET}/release"

need_mingw() {
  command -v x86_64-w64-mingw32-gcc >/dev/null 2>&1 || die "mingw-w64 missing"
  rustup target add "$TARGET" >/dev/null 2>&1 || true
}

stage_sidecars() {
  if [[ "$SKIP_SIDECARS" == "1" ]]; then
    log "SKIP_SIDECARS=1"
    return
  fi
  log "stage Windows sidecars + MinGW DLLs…"
  export USERPROFILE="${USERPROFILE:-${HOME}/.cache/context-osv6/win-userprofile}"
  mkdir -p "$USERPROFILE"
  STAGE_TARGET_TRIPLE="$TARGET" STAGE_BUILD=1 CARGO_BUILD_JOBS="$CARGO_BUILD_JOBS" \
    bash "$ROOT/scripts/stage-desktop-sidecars.sh"
}

build_frontend() {
  if [[ "$SKIP_FRONTEND" == "1" ]]; then
    log "SKIP_FRONTEND=1 — need frontend_next/out"
    [[ -d "$ROOT/frontend_next/out" ]] || die "frontend_next/out missing"
    return
  fi
  log "pnpm build:desktop…"
  (
    cd "$ROOT/frontend_next"
    export NEXT_TELEMETRY_DISABLED=1 BUILD_TARGET=desktop
    pnpm build:desktop
  )
}

build_shell() {
  need_mingw
  log "cargo build desktop shell --target $TARGET (jobs=$CARGO_BUILD_JOBS)…"
  (
    cd "$DESKTOP/src-tauri"
    export CARGO_BUILD_JOBS
    # Shell only — no NSIS. Uses already-built frontendDist if present.
    cargo build --release --target "$TARGET"
  )
  # cargo emits avrag-desktop.exe; Tauri NSIS rename is Context-OS.exe (stale if we only cargo-build).
  if [[ -f "$RELEASE_DIR/avrag-desktop.exe" ]]; then
    :
  elif [[ -f "$RELEASE_DIR/Context-OS.exe" ]]; then
    :
  else
    die "missing $RELEASE_DIR/avrag-desktop.exe (and no Context-OS.exe)"
  fi
}

fresh_shell_exe() {
  local cargo_exe="$RELEASE_DIR/avrag-desktop.exe"
  local tauri_exe="$RELEASE_DIR/Context-OS.exe"
  if [[ -f "$cargo_exe" ]]; then
    # Prefer cargo output — it is what `cargo build` actually updates.
    echo "$cargo_exe"
    return
  fi
  echo "$tauri_exe"
}

copy_mingw_into() {
  local dest="$1"
  local dll
  for dll in libstdc++-6.dll libgcc_s_seh-1.dll libwinpthread-1.dll; do
    if [[ -f "$DESKTOP/src-tauri/binaries/$dll" ]]; then
      cp -f "$DESKTOP/src-tauri/binaries/$dll" "$dest/"
    elif [[ -f "$DESKTOP/runtime/mingw/$dll" ]]; then
      cp -f "$DESKTOP/runtime/mingw/$dll" "$dest/"
    fi
  done
}

copy_sidecars_into() {
  local dest="$1"
  local api="$DESKTOP/src-tauri/binaries/avrag-api-${TARGET}.exe"
  local worker="$DESKTOP/src-tauri/binaries/avrag-worker-${TARGET}.exe"
  [[ -f "$api" ]] && cp -f "$api" "$dest/avrag-api.exe"
  [[ -f "$worker" ]] && cp -f "$worker" "$dest/avrag-worker.exe"
  copy_mingw_into "$dest"
}

# Agent-loop runtime assets: the sidecars load modes/*.yaml + prompts/*.md at
# runtime (relative to their CWD = install dir). Ship them next to the exe.
copy_runtime_assets() {
  local dest="$1"
  cp -rf "$ROOT/avrag-rs/modes" "$dest/modes"
  cp -rf "$ROOT/avrag-rs/prompts" "$dest/prompts"
  log "runtime assets: $dest/{modes,prompts}"
}

launch_exe() {
  local exe="$1"
  [[ -f "$exe" ]] || die "cannot launch missing $exe"
  log "launch: $exe"
  if command -v powershell.exe >/dev/null 2>&1; then
    # Convert /mnt/c/... to Windows path when possible
    local win="$exe"
    if [[ "$exe" =~ ^/mnt/([a-zA-Z])/(.*)$ ]]; then
      win="${BASH_REMATCH[1]^^}:\\${BASH_REMATCH[2]//\//\\}"
    fi
    powershell.exe -NoProfile -Command "Start-Process -FilePath '$win'" 2>/dev/null || true
  else
    log "powershell.exe missing — start manually: $exe"
  fi
}

stop_install_procs() {
  log "stop running client processes (best-effort)…"
  powershell.exe -NoProfile -Command "
    Get-Process Context-OS,avrag-api,avrag-worker -ErrorAction SilentlyContinue | Stop-Process -Force
  " 2>/dev/null || true
  sleep 1
}

case "$MODE" in
  shell-only)
    SKIP_FRONTEND=1
    SKIP_SIDECARS=1
    build_shell
    stop_install_procs
    mkdir -p "$INSTALL_DIR"
    SHELL_EXE="$(fresh_shell_exe)"
    cp -f "$SHELL_EXE" "$INSTALL_DIR/Context-OS.exe"
    copy_mingw_into "$INSTALL_DIR"
    log "hotswapped shell $($SHELL_EXE) → $INSTALL_DIR/Context-OS.exe"
    [[ "$LAUNCH" == "1" ]] && launch_exe "$INSTALL_DIR/Context-OS.exe"
    ;;
  hotswap)
    stage_sidecars
    build_frontend
    build_shell
    stop_install_procs
    [[ -d "$INSTALL_DIR" ]] || die "install dir missing: $INSTALL_DIR (install once via setup.exe first)"
    SHELL_EXE="$(fresh_shell_exe)"
    cp -f "$SHELL_EXE" "$INSTALL_DIR/Context-OS.exe"
    copy_sidecars_into "$INSTALL_DIR"
    copy_runtime_assets "$INSTALL_DIR"
    # Web assets: Tauri release embeds frontendDist at build time into the binary
    # for production builds — shell-only/hotswap of .exe is enough for Rust IPC.
    # If you changed frontend_next only, prefer full tauri build once or use
    # Windows-native `pnpm tauri dev` (devUrl → localhost:3000).
    log "hotswapped into $INSTALL_DIR"
    [[ "$LAUNCH" == "1" ]] && launch_exe "$INSTALL_DIR/Context-OS.exe"
    ;;
  run)
    stage_sidecars
    build_frontend
    # Lightweight tauri build without nsis is just cargo + resources; use cargo
    # then copy sidecars next to exe for manual run.
    build_shell
    copy_sidecars_into "$RELEASE_DIR"
    # Point state at install dir if present, else monorepo desktop/runtime
    if [[ -d "$INSTALL_DIR" ]]; then
      log "tip: set CONTEXT_OS_CLIENT_HOME to install state when testing stack"
    fi
    SHELL_EXE="$(fresh_shell_exe)"
    cp -f "$SHELL_EXE" "$RELEASE_DIR/Context-OS.exe"
    log "run from: $RELEASE_DIR/Context-OS.exe (from $SHELL_EXE)"
    [[ "$LAUNCH" == "1" ]] && launch_exe "$RELEASE_DIR/Context-OS.exe"
    ;;
  *)
    die "unknown mode '$MODE' (use: hotswap | run | shell-only)"
    ;;
esac

log "done. For UI+HMR without packaging, on Windows install Node+Rust and run:"
log "  cd desktop && pnpm install && pnpm tauri dev"
log "Full NSIS only when shipping: bash scripts/build-windows.sh"
