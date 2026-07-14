#!/usr/bin/env bash
# Stage avrag-api / avrag-worker (+ optional compose/scripts) into the client
# runtime layout for monorepo dev and release packaging.
#
# Outputs:
#   desktop/runtime/bin/avrag-api[.exe]
#   desktop/runtime/bin/avrag-worker[.exe]
#   desktop/src-tauri/binaries/avrag-api-<triple>
#   desktop/src-tauri/binaries/avrag-worker-<triple>   (Tauri externalBin naming)
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
AVRAG_DIR="$ROOT/avrag-rs"
RUNTIME_BIN="$ROOT/desktop/runtime/bin"
TAURI_BIN="$ROOT/desktop/src-tauri/binaries"
BUILD="${STAGE_BUILD:-0}"   # 1 = cargo build --release if missing
TRIPLE="${STAGE_TARGET_TRIPLE:-$(rustc -vV 2>/dev/null | awk '/^host:/{print $2}')}"

die() { echo "stage-desktop-sidecars: $*" >&2; exit 1; }
log() { echo "stage-desktop-sidecars: $*"; }

mkdir -p "$RUNTIME_BIN" "$TAURI_BIN"

find_built() {
  local name="$1"
  local candidates=(
    "$AVRAG_DIR/target/release/${name}"
    "$AVRAG_DIR/target/release/${name}.exe"
    "$AVRAG_DIR/target/${TRIPLE}/release/${name}"
    "$AVRAG_DIR/target/${TRIPLE}/release/${name}.exe"
    "$AVRAG_DIR/target/debug/${name}"
    "$AVRAG_DIR/target/debug/${name}.exe"
  )
  local p
  for p in "${candidates[@]}"; do
    [[ -x "$p" || -f "$p" ]] && { echo "$p"; return 0; }
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
    die "missing $name binary (set STAGE_BUILD=1 to cargo build --release -p avrag-api -p avrag-worker)"
  fi
  log "building release avrag-api + avrag-worker…"
  (
    cd "$AVRAG_DIR"
    export CARGO_BUILD_JOBS="${CARGO_BUILD_JOBS:-2}"
    cargo build --release -p avrag-api -p avrag-worker
  )
  find_built "$name" || die "still missing $name after build"
}

API_SRC="$(ensure_built avrag-api)"
WORKER_SRC="$(ensure_built avrag-worker)"

api_dest_name="avrag-api"
worker_dest_name="avrag-worker"
if [[ "$API_SRC" == *.exe ]]; then
  api_dest_name="avrag-api.exe"
  worker_dest_name="avrag-worker.exe"
fi

cp -f "$API_SRC" "$RUNTIME_BIN/$api_dest_name"
cp -f "$WORKER_SRC" "$RUNTIME_BIN/$worker_dest_name"
chmod +x "$RUNTIME_BIN/$api_dest_name" "$RUNTIME_BIN/$worker_dest_name" 2>/dev/null || true

if [[ -n "$TRIPLE" ]]; then
  # Tauri 2 externalBin: binaries/<name>-<target-triple>
  if [[ "$api_dest_name" == *.exe ]]; then
    cp -f "$API_SRC" "$TAURI_BIN/avrag-api-${TRIPLE}.exe"
    cp -f "$WORKER_SRC" "$TAURI_BIN/avrag-worker-${TRIPLE}.exe"
  else
    cp -f "$API_SRC" "$TAURI_BIN/avrag-api-${TRIPLE}"
    cp -f "$WORKER_SRC" "$TAURI_BIN/avrag-worker-${TRIPLE}"
    chmod +x "$TAURI_BIN/avrag-api-${TRIPLE}" "$TAURI_BIN/avrag-worker-${TRIPLE}"
  fi
  log "tauri binaries: $TAURI_BIN/*-${TRIPLE}*"
fi

# Lightweight install layout marker for non-monorepo CLIENT_HOME
cat >"$ROOT/desktop/runtime/LAYOUT" <<EOF
context-os-client-runtime 1
api_port=18080
product_bins=bin/
compose=docker-compose.client.yml
EOF

log "staged:"
log "  $RUNTIME_BIN/$api_dest_name"
log "  $RUNTIME_BIN/$worker_dest_name"
log "  from api=$API_SRC"
log "  triple=${TRIPLE:-none}"
