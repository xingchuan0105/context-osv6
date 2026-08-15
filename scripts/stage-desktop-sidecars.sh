#!/usr/bin/env bash
# Stage avrag-api / avrag-worker / context-os-mcp / context-os into client runtime + Tauri externalBin layout.
#
# Outputs:
#   desktop/runtime/bin/avrag-api[.exe]
#   desktop/runtime/bin/avrag-worker[.exe]
#   desktop/runtime/bin/context-os-mcp[.exe]   # stdio MCP for coding agents (not a Tauri sidecar)
#   desktop/runtime/bin/context-os[.exe]      # thin CLI (status/ingest/ask/sources)
#   desktop/runtime/bin/python/               # python embeddable bundle (windows triple only)
#   desktop/runtime/parsers/lit/              # lit.exe + pdfium.dll (windows triple only)
#   desktop/src-tauri/binaries/avrag-api-<triple>[.exe]
#   desktop/src-tauri/binaries/avrag-worker-<triple>[.exe]
#
# Env:
#   STAGE_TARGET_TRIPLE  default: host (use x86_64-pc-windows-gnu for NSIS)
#   STAGE_BUILD=1        cargo build --release [--target] if missing
#   CARGO_BUILD_JOBS     default 2
#   PYTHON_EMBED_ZIP     path to a local python-embed zip (skips download;
#                        sha256 is still verified against the pin below)
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

# Pinned CPython embeddable for the Windows sandbox (code-interpreter resolves
# <exe_dir>/python/python.exe at spawn time). Flat layout: python.exe + DLLs +
# python312.zip; keep python312._pth untouched. Embeddable has no pip — SaC
# sandbox only needs the stdlib (asyncio/json/threading/socket).
PYTHON_EMBED_VERSION="3.12.10"
PYTHON_EMBED_SHA256="4acbed6dd1c744b0376e3b1cf57ce906f9dc9e95e68824584c8099a63025a3c3"
PYTHON_EMBED_URL="https://www.python.org/ftp/python/${PYTHON_EMBED_VERSION}/python-${PYTHON_EMBED_VERSION}-embed-amd64.zip"

# liteparse CLI for the Windows PDF route (worker spawns LITEPARSE_BIN).
# pdfium-sys runtime-loads pdfium.dll from the exe dir — both files ship together.
LITEPARSE_VERSION="2.10.0"

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
    die "missing $name for triple=$TRIPLE (set STAGE_BUILD=1 to cargo build --release --target $TRIPLE -p avrag-api -p avrag-worker -p context-os)"
  fi
  log "building release avrag-api + avrag-worker + context-os (target=$TRIPLE, jobs=${CARGO_BUILD_JOBS:-2})…"
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
      cargo build --release --target "$TRIPLE" -p avrag-api -p avrag-worker -p context-os
    else
      cargo build --release -p avrag-api -p avrag-worker -p context-os
    fi
  )
  find_built "$name" || die "still missing $name after build (triple=$TRIPLE). Check cross-linker (mingw) and crate windows support."
}

API_SRC="$(ensure_built avrag-api)"
WORKER_SRC="$(ensure_built avrag-worker)"
MCP_SRC="$(ensure_built context-os-mcp)"
CLI_SRC="$(ensure_built context-os)"

api_dest_name="avrag-api"
worker_dest_name="avrag-worker"
mcp_dest_name="context-os-mcp"
cli_dest_name="context-os"
if [[ "$IS_WINDOWS_TRIPLE" == "1" ]]; then
  api_dest_name="avrag-api.exe"
  worker_dest_name="avrag-worker.exe"
  mcp_dest_name="context-os-mcp.exe"
  cli_dest_name="context-os.exe"
fi

# When cross-staging Windows from Linux, put .exe under runtime/bin for companion pack
# (overwrites host bins — re-run without STAGE_TARGET_TRIPLE for linux host bins).
cp -f "$API_SRC" "$RUNTIME_BIN/$api_dest_name"
cp -f "$WORKER_SRC" "$RUNTIME_BIN/$worker_dest_name"
cp -f "$MCP_SRC" "$RUNTIME_BIN/$mcp_dest_name"
cp -f "$CLI_SRC" "$RUNTIME_BIN/$cli_dest_name"
chmod +x "$RUNTIME_BIN/$api_dest_name" "$RUNTIME_BIN/$worker_dest_name" "$RUNTIME_BIN/$mcp_dest_name" "$RUNTIME_BIN/$cli_dest_name" 2>/dev/null || true

# Tauri 2 externalBin: binaries/<name>-<target-triple>[.exe]
# (context-os-mcp is agent-facing, not launched by Tauri — only runtime/bin.)
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

# MinGW runtime DLLs (gnu target): avrag-api/worker need these next to the exe on Windows.
# Without them Windows shows "找不到 libstdc++-6.dll" and API never binds :18080.
if [[ "$IS_WINDOWS_TRIPLE" == "1" ]]; then
  MINGW_DEST="$ROOT/desktop/runtime/mingw"
  mkdir -p "$MINGW_DEST" "$RUNTIME_BIN" "$TAURI_BIN"
  copy_mingw_dll() {
    local name="$1"
    local src=""
    local c
    for c in \
      "/usr/lib/gcc/x86_64-w64-mingw32/13-posix/${name}" \
      "/usr/lib/gcc/x86_64-w64-mingw32/13-win32/${name}" \
      "/usr/lib/gcc/x86_64-w64-mingw32/12-posix/${name}" \
      "/usr/lib/gcc/x86_64-w64-mingw32/12-win32/${name}" \
      "/usr/x86_64-w64-mingw32/lib/${name}" \
      "/usr/x86_64-w64-mingw32/bin/${name}"; do
      if [[ -f "$c" ]]; then
        src="$c"
        break
      fi
    done
    if [[ -z "$src" ]]; then
      # last resort: locate
      src="$(find /usr/lib/gcc/x86_64-w64-mingw32 /usr/x86_64-w64-mingw32 -name "$name" 2>/dev/null | head -1 || true)"
    fi
    if [[ -n "$src" && -f "$src" ]]; then
      cp -f "$src" "$MINGW_DEST/$name"
      cp -f "$src" "$RUNTIME_BIN/$name"
      cp -f "$src" "$TAURI_BIN/$name"
      log "mingw dll: $name <- $src"
    else
      log "warning: missing MinGW DLL $name (Windows sidecars may fail to start)"
    fi
  }
  copy_mingw_dll "libstdc++-6.dll"
  copy_mingw_dll "libgcc_s_seh-1.dll"
  copy_mingw_dll "libwinpthread-1.dll"

  # Python embeddable bundle → runtime/bin/python/ (next to avrag-api.exe;
  # NSIS installs it via the bundle.resources map in tauri.conf.json).
  stage_python_bundle() {
    local dest="$RUNTIME_BIN/python"
    local zip="${PYTHON_EMBED_ZIP:-$ROOT/desktop/runtime/vendor/python-${PYTHON_EMBED_VERSION}-embed-amd64.zip}"
    if [[ ! -f "$zip" ]]; then
      mkdir -p "$(dirname "$zip")"
      log "downloading ${PYTHON_EMBED_URL} …"
      curl -fL --retry 3 -o "$zip" "$PYTHON_EMBED_URL" || die "python embed download failed"
    fi
    echo "$PYTHON_EMBED_SHA256  $zip" | sha256sum -c - >/dev/null \
      || die "python embed sha256 mismatch: $zip (expected $PYTHON_EMBED_SHA256)"
    rm -rf "$dest"
    mkdir -p "$dest"
    unzip -q -o "$zip" -d "$dest" || die "python embed unzip failed: $zip"
    [[ -f "$dest/python.exe" ]] || die "python bundle incomplete: $dest/python.exe missing"
    log "python bundle: $dest (python $PYTHON_EMBED_VERSION embed amd64)"
  }
  stage_python_bundle

  # liteparse CLI (lit.exe) + pdfium.dll → runtime/parsers/lit/ (PDF ingest;
  # native_stack writes LITEPARSE_BIN into client.env when lit.exe is present).
  stage_lit_parser() {
    local dest="$ROOT/desktop/runtime/parsers/lit"
    local install_root="$ROOT/desktop/runtime/vendor/lit-${LITEPARSE_VERSION}-${TRIPLE}"
    # pdfium-sys build.rs resolves its cache dir via USERPROFILE/LOCALAPPDATA
    # when the target OS is windows — provide a WSL-side stand-in.
    export USERPROFILE="${USERPROFILE:-$HOME/.cache/context-osv6/win-userprofile}"
    export LOCALAPPDATA="${LOCALAPPDATA:-$USERPROFILE/AppData/Local}"
    mkdir -p "$LOCALAPPDATA"
    if [[ ! -f "$install_root/bin/lit.exe" ]]; then
      log "cross-building liteparse $LITEPARSE_VERSION for $TRIPLE …"
      # --no-default-features: drop the `tesseract` feature (native Tesseract
      # link is not available for windows targets; scanned PDFs route to
      # PaddleOCR anyway, lit is always invoked with --no-ocr).
      # Stub libdl.a: part of the dep tree emits -ldl even for windows-gnu;
      # the symbols are dead on Windows, an empty archive satisfies ld.
      local fakelibs="$ROOT/desktop/runtime/vendor/fakelibs-$TRIPLE"
      mkdir -p "$fakelibs"
      [[ -f "$fakelibs/libdl.a" ]] || x86_64-w64-mingw32-ar rcs "$fakelibs/libdl.a"
      CARGO_TARGET_DIR="$ROOT/desktop/runtime/vendor/lit-target-$TRIPLE" \
      RUSTFLAGS="-L $fakelibs" \
      cargo install liteparse --version "$LITEPARSE_VERSION" --no-default-features \
        --target "$TRIPLE" --root "$install_root" -j"${CARGO_BUILD_JOBS:-2}" \
        || die "liteparse cross-build failed (target=$TRIPLE)"
    fi
    local pdfium_dll
    pdfium_dll="$(ls -t "$LOCALAPPDATA"/pdfium-rs/*/pdfium-win-x64/bin/pdfium.dll 2>/dev/null | head -1)"
    [[ -n "$pdfium_dll" && -f "$pdfium_dll" ]] \
      || die "pdfium.dll not found under $LOCALAPPDATA/pdfium-rs (pdfium-sys download missing)"
    rm -rf "$dest"
    mkdir -p "$dest"
    cp -f "$install_root/bin/lit.exe" "$dest/lit.exe"
    cp -f "$pdfium_dll" "$dest/pdfium.dll"
    log "lit parser: $dest (liteparse $LITEPARSE_VERSION + $(basename "$pdfium_dll"))"
  }
  stage_lit_parser
fi

cat >"$ROOT/desktop/runtime/LAYOUT" <<EOF
context-os-client-runtime 1
api_port=18080
product_bins=bin/
mcp_stdio_bin=bin/context-os-mcp
cli_bin=bin/context-os
compose=docker-compose.client.yml
stage_triple=${TRIPLE}
cross=${CROSS}
EOF

log "staged:"
log "  $RUNTIME_BIN/$api_dest_name  ($(file -b "$RUNTIME_BIN/$api_dest_name" 2>/dev/null | head -c 80 || true))"
log "  $RUNTIME_BIN/$worker_dest_name"
log "  $RUNTIME_BIN/$mcp_dest_name"
log "  $RUNTIME_BIN/$cli_dest_name"
log "  from api=$API_SRC mcp=$MCP_SRC cli=$CLI_SRC"
log "  triple=${TRIPLE} cross=${CROSS}"
