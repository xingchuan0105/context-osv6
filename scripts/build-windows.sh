#!/usr/bin/env bash
# Build Context-OS Windows NSIS installer (setup.exe).
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

# Portable PG+pgvector+Redis (BR2). Default embed; SKIP_BUNDLED_RUNTIME=1 for slim setup.
SKIP_BUNDLED_RUNTIME="${SKIP_BUNDLED_RUNTIME:-0}"
BUNDLED_WIN="$DESKTOP/runtime/bundled/windows-x64"
if [[ "$SKIP_BUNDLED_RUNTIME" != "1" ]]; then
  if [[ ! -f "$BUNDLED_WIN/pgsql/bin/pg_ctl.exe" || ! -f "$BUNDLED_WIN/redis/redis-server.exe" ]]; then
    log "bundled runtime missing — fetch from VPS (or run assemble)…"
    bash "$ROOT/scripts/stage-desktop-bundled-runtime.sh" fetch || true
  fi
  [[ -f "$BUNDLED_WIN/pgsql/bin/pg_ctl.exe" ]] || die "missing $BUNDLED_WIN/pgsql (run: bash scripts/stage-desktop-bundled-runtime.sh fetch|assemble)"
  [[ -f "$BUNDLED_WIN/redis/redis-server.exe" ]] || die "missing redis-server.exe in bundled runtime"
  [[ -f "$BUNDLED_WIN/pgsql/lib/vector.dll" ]] || die "missing vector.dll in bundled runtime"
  log "bundled runtime: $BUNDLED_WIN ($(du -sh "$BUNDLED_WIN" | awk '{print $1}'))"
else
  log "SKIP_BUNDLED_RUNTIME=1 — NSIS will not embed portable PG/Redis"
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

# Merge Tauri config: externalBin sidecars + resources map for portable runtime (BR2).
# Written to a temp file so nested JSON is not shell-escaped to death.
TAURI_EXTRA_FILE="$(mktemp "${TMPDIR:-/tmp}/tauri-extra-XXXXXX.json")"
cleanup_extra() { rm -f "$TAURI_EXTRA_FILE"; }
trap cleanup_extra EXIT

# Paths in this JSON are relative to desktop/src-tauri (Tauri convention).
python3 - "$TAURI_EXTRA_FILE" "$SKIP_SIDECARS" "$SKIP_BUNDLED_RUNTIME" "$DESKTOP" <<'PY'
import json, sys
from pathlib import Path

path, skip_sidecars, skip_rt, desktop = sys.argv[1], sys.argv[2] == "1", sys.argv[3] == "1", Path(sys.argv[4])
src_tauri = desktop / "src-tauri"
bundle = {}
if not skip_sidecars:
    bundle["externalBin"] = ["binaries/avrag-api", "binaries/avrag-worker"]
# Map form: source (rel to src-tauri) → install path (next to exe / under resources)
resources = {
    "../runtime/docker-compose.client.yml": "runtime/docker-compose.client.yml",
    "../runtime/README.md": "runtime/README.md",
}
# MinGW runtime for gnu-built avrag-api/worker (LoadLibrary looks next to the exe).
for dll in ("libstdc++-6.dll", "libgcc_s_seh-1.dll", "libwinpthread-1.dll"):
    for rel in (f"binaries/{dll}", f"../runtime/mingw/{dll}", f"../runtime/bin/{dll}"):
        if (src_tauri / rel).is_file():
            resources[rel] = dll
            break
# Python embeddable for the sandbox bridge: install as $INSTDIR/python/ next to
# avrag-api.exe (code-interpreter resolves <exe_dir>/python/python.exe).
if not skip_sidecars:
    if (desktop / "runtime/bin/python/python.exe").is_file():
        resources["../runtime/bin/python"] = "python/"
    else:
        print("build-windows: warning: runtime/bin/python not staged; sandbox bridge will fall back to PATH probing", file=sys.stderr)
    # Stdlib-only document parsers driven by the bundled python (MARKITDOWN_BIN /
    # ANYDOC_BIN written into client.env by native_stack when present).
    if (desktop / "runtime/parsers/markitdown-lite.cmd").is_file():
        resources["../runtime/parsers"] = "runtime/parsers"
    # Agent-loop runtime assets: avrag-api/worker load modes/*.yaml + prompts/*.md
    # relative to CWD (= install dir for spawned sidecars). Same channel as
    # hotswap's copy_runtime_assets.
    resources["../../avrag-rs/modes"] = "modes"
    resources["../../avrag-rs/prompts"] = "prompts"
if not skip_rt:
    resources.update({
        "../runtime/bundled/windows-x64/pgsql": "runtime/pgsql",
        "../runtime/bundled/windows-x64/redis": "runtime/redis",
        "../runtime/bundled/windows-x64/runtime.version": "runtime/runtime.version",
        "../runtime/bundled/windows-x64/THIRD_PARTY.txt": "runtime/THIRD_PARTY.txt",
        # migrations for AVRAG_RUN_MIGRATIONS / sqlx on first ensure
        "../../avrag-rs/migrations": "runtime/migrations",
    })
bundle["resources"] = resources
json.dump({"bundle": bundle}, open(path, "w"), indent=2)
print(path)
missing = [d for d in ("libstdc++-6.dll", "libgcc_s_seh-1.dll", "libwinpthread-1.dll") if d not in resources.values()]
if missing and not skip_sidecars:
    print("build-windows: warning: MinGW DLLs not staged:", ", ".join(missing), file=sys.stderr)
PY
log "tauri extra config: $TAURI_EXTRA_FILE"
log "tauri build --target $TARGET --bundles nsis (+ sidecars + bundled runtime)"
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
      --config "$TAURI_EXTRA_FILE"
  else
    pnpm tauri build --target "$TARGET" --bundles nsis \
      --config "$TAURI_EXTRA_FILE"
  fi
)

# Locate setup.exe — prefer Context-OS-Client_* (current product name), then legacy Context-OS_*
NSIS_DIR="$DESKTOP/src-tauri/target/${TARGET}/release/bundle/nsis"
SETUP=""
# Prefer package.json version (single source of truth with tauri.conf / Cargo.toml).
VER="${VERSION:-}"
if [[ -z "$VER" ]]; then
  VER="$(node -p "require('$DESKTOP/package.json').version" 2>/dev/null || true)"
fi
VER="${VER:-0.2.0}"
if [[ -f "$NSIS_DIR/Context-OS Client_${VER}_x64-setup.exe" ]]; then
  SETUP="$NSIS_DIR/Context-OS Client_${VER}_x64-setup.exe"
elif [[ -f "$NSIS_DIR/Context-OS-Client_${VER}_x64-setup.exe" ]]; then
  SETUP="$NSIS_DIR/Context-OS-Client_${VER}_x64-setup.exe"
elif [[ -f "$NSIS_DIR/Context-OS_${VER}_x64-setup.exe" ]]; then
  SETUP="$NSIS_DIR/Context-OS_${VER}_x64-setup.exe"
fi
if [[ -z "$SETUP" || ! -f "$SETUP" ]]; then
  # Prefer newest Context-OS* setup by mtime (avoid stale 0.1.x head -1 lexical order)
  SETUP="$(find "$NSIS_DIR" -type f \( -name 'Context-OS Client*-setup.exe' -o -name 'Context-OS*-setup.exe' \) -printf '%T@ %p\n' 2>/dev/null | sort -nr | head -1 | cut -d' ' -f2- || true)"
fi
if [[ -z "$SETUP" || ! -f "$SETUP" ]]; then
  SETUP="$(find "$NSIS_DIR" -type f -name '*-setup.exe' -printf '%T@ %p\n' 2>/dev/null | sort -nr | head -1 | cut -d' ' -f2- || true)"
fi
if [[ -z "$SETUP" || ! -f "$SETUP" ]]; then
  SETUP="$(find "$DESKTOP/src-tauri/target" -type f -name 'Context-OS*-setup.exe' -printf '%T@ %p\n' 2>/dev/null | sort -nr | head -1 | cut -d' ' -f2- || true)"
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
