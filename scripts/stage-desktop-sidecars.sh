#!/usr/bin/env bash
# Stage avrag-api / avrag-worker into client runtime + Tauri externalBin layout.
#
# Outputs:
#   desktop/runtime/bin/avrag-api[.exe]
#   desktop/runtime/bin/avrag-worker[.exe]
#   desktop/src-tauri/binaries/avrag-api-<triple>[.exe]
#   desktop/src-tauri/binaries/avrag-worker-<triple>[.exe]
#
# Env:
#   STAGE_TARGET_TRIPLE  default: host (use x86_64-pc-windows-gnu for NSIS)
#   STAGE_BUILD=1        cargo build --release [--target] if missing
#   CARGO_BUILD_JOBS     default 2
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
AVRAG_DIR="$ROOT/avrag-rs"
RUNTIME_BIN="$ROOT/desktop/runtime/bin"
TAURI_BIN="$ROOT/desktop/src-tauri/binaries"
BUILD="${STAGE_BUILD:-0}"
HOST_TRIPLE="$(rustc -vV 2>/dev/null | awk '/^host:/{print $2}')"
TRIPLE="${STAGE_TARGET_TRIPLE:-$HOST_TRIPLE}"
CROSS=0
[[ "$TRIPLE" != "$HOST_TRIPLE" ]] && CROSS=1
IS_WINDOWS_TRIPLE=0
case "$TRIPLE" in
  *windows*) IS_WINDOWS_TRIPLE=1 ;;
esac

die() { echo "stage-desktop-sidecars: $*" >&2; exit 1; }
# Always stderr — ensure_built is used inside $(...) capture.
log() { echo "stage-desktop-sidecars: $*" >&2; }

mkdir -p "$RUNTIME_BIN" "$TAURI_BIN"

# Only accept artifacts that match the requested triple (never rename host ELF to .exe).
find_built() {
  local name="$1"
  local candidates=()
  if [[ "$CROSS" == "1" ]]; then
    candidates=(
      "$AVRAG_DIR/target/${TRIPLE}/release/${name}.exe"
      "$AVRAG_DIR/target/${TRIPLE}/release/${name}"
      "$AVRAG_DIR/target/${TRIPLE}/debug/${name}.exe"
      "$AVRAG_DIR/target/${TRIPLE}/debug/${name}"
    )
  else
    candidates=(
      "$AVRAG_DIR/target/release/${name}"
      "$AVRAG_DIR/target/release/${name}.exe"
      "$AVRAG_DIR/target/${TRIPLE}/release/${name}"
      "$AVRAG_DIR/target/${TRIPLE}/release/${name}.exe"
      "$AVRAG_DIR/target/debug/${name}"
      "$AVRAG_DIR/target/debug/${name}.exe"
    )
  fi
  local p
  for p in "${candidates[@]}"; do
    [[ -f "$p" ]] || continue
    # Reject obvious cross-mismatch: Windows triple must be PE (.exe path or file).
    if [[ "$IS_WINDOWS_TRIPLE" == "1" && "$p" != *.exe ]]; then
      continue
    fi
    if [[ "$IS_WINDOWS_TRIPLE" != "1" && "$p" == *.exe ]]; then
      continue
    fi
    echo "$p"
    return 0
  done
  return 1
}

ensure_built() {
  local name="$1"
  if find_built "$name" >/dev/null; then
    find_built "$name"
    return 0
  fi
  if [[ "$BUILD" != "1" ]]; then
    die "missing $name for triple=$TRIPLE (set STAGE_BUILD=1 to cargo build --release --target $TRIPLE -p avrag-api -p avrag-worker)"
  fi
  log "building release avrag-api + avrag-worker (target=$TRIPLE, jobs=${CARGO_BUILD_JOBS:-2})…"
  (
    cd "$AVRAG_DIR"
    export CARGO_BUILD_JOBS="${CARGO_BUILD_JOBS:-2}"
    # liteparse-pdfium-sys and other crates expect Windows-style env when targeting windows-gnu.
    if [[ "$IS_WINDOWS_TRIPLE" == "1" ]]; then
      export USERPROFILE="${USERPROFILE:-${HOME}/.cache/context-osv6/win-userprofile}"
      mkdir -p "$USERPROFILE"
    fi
    rustup target add "$TRIPLE" >/dev/null 2>&1 || true
    if [[ "$CROSS" == "1" ]]; then
      cargo build --release --target "$TRIPLE" -p avrag-api -p avrag-worker
    else
      cargo build --release -p avrag-api -p avrag-worker
    fi
  )
  find_built "$name" || die "still missing $name after build (triple=$TRIPLE). Check cross-linker (mingw) and crate windows support."
}

API_SRC="$(ensure_built avrag-api)"
WORKER_SRC="$(ensure_built avrag-worker)"

api_dest_name="avrag-api"
worker_dest_name="avrag-worker"
if [[ "$IS_WINDOWS_TRIPLE" == "1" ]]; then
  api_dest_name="avrag-api.exe"
  worker_dest_name="avrag-worker.exe"
fi

# When cross-staging Windows from Linux, put .exe under runtime/bin for companion pack
# (overwrites host bins — re-run without STAGE_TARGET_TRIPLE for linux host bins).
cp -f "$API_SRC" "$RUNTIME_BIN/$api_dest_name"
cp -f "$WORKER_SRC" "$RUNTIME_BIN/$worker_dest_name"
chmod +x "$RUNTIME_BIN/$api_dest_name" "$RUNTIME_BIN/$worker_dest_name" 2>/dev/null || true

# Tauri 2 externalBin: binaries/<name>-<target-triple>[.exe]
if [[ -n "$TRIPLE" ]]; then
  if [[ "$IS_WINDOWS_TRIPLE" == "1" ]]; then
    cp -f "$API_SRC" "$TAURI_BIN/avrag-api-${TRIPLE}.exe"
    cp -f "$WORKER_SRC" "$TAURI_BIN/avrag-worker-${TRIPLE}.exe"
  else
    cp -f "$API_SRC" "$TAURI_BIN/avrag-api-${TRIPLE}"
    cp -f "$WORKER_SRC" "$TAURI_BIN/avrag-worker-${TRIPLE}"
    chmod +x "$TAURI_BIN/avrag-api-${TRIPLE}" "$TAURI_BIN/avrag-worker-${TRIPLE}"
  fi
  log "tauri externalBin: $TAURI_BIN/avrag-*-${TRIPLE}*"
fi

cat >"$ROOT/desktop/runtime/LAYOUT" <<EOF
context-os-client-runtime 1
api_port=18080
product_bins=bin/
compose=docker-compose.client.yml
stage_triple=${TRIPLE}
cross=${CROSS}
EOF

log "staged:"
log "  $RUNTIME_BIN/$api_dest_name  ($(file -b "$RUNTIME_BIN/$api_dest_name" 2>/dev/null | head -c 80 || true))"
log "  $RUNTIME_BIN/$worker_dest_name"
log "  from api=$API_SRC"
log "  triple=${TRIPLE} cross=${CROSS}"
