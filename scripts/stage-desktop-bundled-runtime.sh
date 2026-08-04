#!/usr/bin/env bash
# Stage / pack / fetch Context-OS portable data-plane runtime (PG16+pgvector+Redis).
# Design: docs/desktop/2026-08-04-portable-runtime-design.md §7.3 / §15.1
#
# Usage:
#   bash scripts/stage-desktop-bundled-runtime.sh status
#   bash scripts/stage-desktop-bundled-runtime.sh fetch
#   bash scripts/stage-desktop-bundled-runtime.sh pack
#   bash scripts/stage-desktop-bundled-runtime.sh verify
#   bash scripts/stage-desktop-bundled-runtime.sh assemble   # large download
#
# Does not commit binaries. Cache: ~/.cache/context-osv6/bundled-runtime/
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
BUNDLED_DIR="$ROOT/desktop/runtime/bundled"
PINS_FILE="$BUNDLED_DIR/pins.env"
PLATFORM="${COS_RUNTIME_PLATFORM:-windows-x64}"
STAGE_DIR="${COS_RUNTIME_STAGE:-$BUNDLED_DIR/$PLATFORM}"
CACHE_DIR="${COS_RUNTIME_CACHE:-${XDG_CACHE_HOME:-$HOME/.cache}/context-osv6/bundled-runtime}"
OUT_ROOT="${DESKTOP_RUNTIME_OUT:-$ROOT/dist/desktop-runtime}"
PUBLIC_BASE="${DESKTOP_RUNTIME_PUBLIC_BASE:-https://app.contextlm.top/releases/desktop/runtime}"

die() { echo "stage-desktop-bundled-runtime: $*" >&2; exit 1; }
log() { echo "stage-desktop-bundled-runtime: $*"; }

load_pins() {
  [[ -f "$PINS_FILE" ]] || die "missing $PINS_FILE"
  # shellcheck disable=SC1090
  set -a
  source "$PINS_FILE"
  set +a
  PLATFORM="${COS_RUNTIME_PLATFORM:-windows-x64}"
  STAGE_DIR="${COS_RUNTIME_STAGE:-$BUNDLED_DIR/$PLATFORM}"
  PUBLIC_BASE="${DESKTOP_RUNTIME_PUBLIC_BASE:-$PUBLIC_BASE}"
  : "${COS_RUNTIME_ID:?pins.env must set COS_RUNTIME_ID}"
  : "${COS_RUNTIME_ZIP_NAME:?pins.env must set COS_RUNTIME_ZIP_NAME}"
}

need_cmd() {
  command -v "$1" >/dev/null 2>&1 || die "need command: $1"
}

sha256_of() {
  local f="$1"
  if command -v sha256sum >/dev/null 2>&1; then
    sha256sum "$f" | awk '{print $1}'
  elif command -v shasum >/dev/null 2>&1; then
    shasum -a 256 "$f" | awk '{print $1}'
  else
    die "need sha256sum or shasum"
  fi
}

verify_sha() {
  local f="$1" expect="$2" label="${3:-file}"
  [[ -n "$expect" ]] || {
    log "warn: no expected sha256 for $label — skip verify (set in pins.env after first download)"
    return 0
  }
  local got
  got="$(sha256_of "$f")"
  [[ "$got" == "$expect" ]] || die "$label sha256 mismatch: got=$got expect=$expect"
  log "sha256 ok: $label"
}

download() {
  local url="$1" dest="$2"
  mkdir -p "$(dirname "$dest")"
  if [[ -f "$dest" ]]; then
    log "cache hit: $dest"
    return 0
  fi
  log "download $url → $dest"
  need_cmd curl
  curl -fL --retry 3 --retry-delay 2 -o "$dest.partial" "$url"
  mv -f "$dest.partial" "$dest"
}

# ── Required tree (Windows install / NSIS material) ──────────────────────────

pg_ctl_name() {
  if [[ "$PLATFORM" == windows-x64 ]]; then
    echo "pg_ctl.exe"
  else
    echo "pg_ctl"
  fi
}

redis_name() {
  if [[ "$PLATFORM" == windows-x64 ]]; then
    echo "redis-server.exe"
  else
    echo "redis-server"
  fi
}

cmd_verify() {
  load_pins
  local ok=1
  local pg="$STAGE_DIR/pgsql/bin/$(pg_ctl_name)"
  local initdb="$STAGE_DIR/pgsql/bin/initdb"
  [[ "$PLATFORM" == windows-x64 ]] && initdb="$STAGE_DIR/pgsql/bin/initdb.exe"
  local redis="$STAGE_DIR/redis/$(redis_name)"
  local ver="$STAGE_DIR/runtime.version"

  [[ -f "$pg" ]] || { log "missing $pg"; ok=0; }
  [[ -f "$initdb" ]] || { log "missing $initdb"; ok=0; }
  [[ -f "$redis" ]] || { log "missing $redis"; ok=0; }
  [[ -f "$ver" ]] || { log "missing $ver"; ok=0; }

  local vector_dll="$STAGE_DIR/pgsql/lib/vector.dll"
  local vector_so="$STAGE_DIR/pgsql/lib/vector.so"
  local vector_control="$STAGE_DIR/pgsql/share/extension/vector.control"
  if [[ "$PLATFORM" == windows-x64 ]]; then
    if [[ ! -f "$vector_dll" ]]; then
      log "warn: missing $vector_dll (CREATE EXTENSION vector will fail until placed)"
      ok=0
    fi
  else
    if [[ ! -f "$vector_so" ]]; then
      log "warn: missing $vector_so"
      ok=0
    fi
  fi
  [[ -f "$vector_control" ]] || { log "warn: missing $vector_control"; ok=0; }

  if [[ "$ok" -eq 1 ]]; then
    log "verify OK: $STAGE_DIR"
    cat "$ver" 2>/dev/null || true
    return 0
  fi
  die "verify failed for $STAGE_DIR (see messages above)"
}

cmd_status() {
  load_pins
  echo "pins:        $PINS_FILE"
  echo "runtime_id:  ${COS_RUNTIME_ID}"
  echo "platform:    $PLATFORM"
  echo "stage_dir:   $STAGE_DIR"
  echo "cache_dir:   $CACHE_DIR"
  echo "out_root:    $OUT_ROOT"
  echo "public_base: $PUBLIC_BASE"
  echo "zip_name:    $COS_RUNTIME_ZIP_NAME"
  if [[ -d "$STAGE_DIR" ]]; then
    echo "stage:       present"
    [[ -f "$STAGE_DIR/runtime.version" ]] && echo "  version:   $(tr -d '\n' <"$STAGE_DIR/runtime.version")"
    du -sh "$STAGE_DIR" 2>/dev/null | awk '{print "  size:      " $1}'
    local pg="$STAGE_DIR/pgsql/bin/$(pg_ctl_name)"
    local redis="$STAGE_DIR/redis/$(redis_name)"
    echo "  pg_ctl:    $([[ -f "$pg" ]] && echo yes || echo NO) $pg"
    echo "  redis:     $([[ -f "$redis" ]] && echo yes || echo NO) $redis"
    if [[ -f "$STAGE_DIR/pgsql/lib/vector.dll" || -f "$STAGE_DIR/pgsql/lib/vector.so" ]]; then
      echo "  pgvector:  yes"
    else
      echo "  pgvector:  NO"
    fi
  else
    echo "stage:       empty (run fetch or assemble)"
  fi
  if [[ -f "$OUT_ROOT/$COS_RUNTIME_ZIP_NAME" ]]; then
    echo "pack zip:    $OUT_ROOT/$COS_RUNTIME_ZIP_NAME ($(du -h "$OUT_ROOT/$COS_RUNTIME_ZIP_NAME" | awk '{print $1}'))"
  else
    echo "pack zip:    (none — run pack)"
  fi
  if [[ -f "$CACHE_DIR/manifest.json" ]]; then
    echo "cache man:   $CACHE_DIR/manifest.json"
  fi
}

write_stage_meta() {
  mkdir -p "$STAGE_DIR"
  printf '%s\n' "$COS_RUNTIME_ID" >"$STAGE_DIR/runtime.version"
  cat >"$STAGE_DIR/THIRD_PARTY.txt" <<EOF
Context-OS Client — bundled data-plane third-party components
Runtime id: ${COS_RUNTIME_ID}

PostgreSQL ${PG_WIN_VERSION:-16.x}
  License: PostgreSQL License
  Source: ${PG_WIN_URL:-EDB Windows binaries}

pgvector ${PGVECTOR_VERSION:-}
  License: PostgreSQL License
  Source: https://github.com/pgvector/pgvector

Redis for Windows ${REDIS_WIN_VERSION:-}
  Port: tporadowski/redis (historical Windows port; review license before upgrade)
  Source: ${REDIS_WIN_URL:-}

Do not redistribute without retaining upstream notices under each tree (pgsql/, redis/).
EOF
}

# ── fetch: pull published cos-runtime zip from VPS / public base ─────────────

cmd_fetch() {
  load_pins
  need_cmd curl
  mkdir -p "$CACHE_DIR" "$STAGE_DIR"

  local man_url="$PUBLIC_BASE/manifest.json"
  local man_local="$CACHE_DIR/manifest.json"
  log "fetch manifest $man_url"
  if curl -fL --retry 2 -o "$man_local.partial" "$man_url" 2>/dev/null; then
    mv -f "$man_local.partial" "$man_local"
  else
    log "warn: manifest not reachable; trying direct zip path"
    rm -f "$man_local.partial"
  fi

  local zip_name="$COS_RUNTIME_ZIP_NAME"
  local zip_sha=""
  if [[ -f "$man_local" ]] && command -v node >/dev/null 2>&1; then
    # Prefer manifest fields when present
    local m_name m_sha
    m_name="$(node -e "
      const m=JSON.parse(require('fs').readFileSync(process.argv[1],'utf8'));
      const p=m.platforms&&m.platforms['$PLATFORM'];
      if(p&&p.file) process.stdout.write(p.file);
    " "$man_local" 2>/dev/null || true)"
    m_sha="$(node -e "
      const m=JSON.parse(require('fs').readFileSync(process.argv[1],'utf8'));
      const p=m.platforms&&m.platforms['$PLATFORM'];
      if(p&&p.sha256) process.stdout.write(p.sha256);
    " "$man_local" 2>/dev/null || true)"
    [[ -n "$m_name" ]] && zip_name="$m_name"
    [[ -n "$m_sha" ]] && zip_sha="$m_sha"
  fi

  local zip_url="$PUBLIC_BASE/$PLATFORM/$zip_name"
  local zip_path="$CACHE_DIR/$zip_name"
  download "$zip_url" "$zip_path"
  if [[ -n "$zip_sha" ]]; then
    verify_sha "$zip_path" "$zip_sha" "$zip_name"
  elif [[ -f "$zip_path.sha256" ]]; then
    verify_sha "$zip_path" "$(awk '{print $1}' "$zip_path.sha256")" "$zip_name"
  else
    # sidecar on server
    if curl -fL --retry 1 -o "$zip_path.sha256" "$zip_url.sha256" 2>/dev/null; then
      verify_sha "$zip_path" "$(awk '{print $1}' "$zip_path.sha256")" "$zip_name"
    else
      log "warn: no sha256 for $zip_name — continuing without verify"
    fi
  fi

  need_cmd unzip
  log "unpack → $STAGE_DIR"
  rm -rf "${STAGE_DIR}.tmp"
  mkdir -p "${STAGE_DIR}.tmp"
  unzip -q -o "$zip_path" -d "${STAGE_DIR}.tmp"
  # Zip may contain top-level pgsql/ redis/ or a single root folder
  if [[ -d "${STAGE_DIR}.tmp/pgsql" ]]; then
    rm -rf "$STAGE_DIR"
    mv "${STAGE_DIR}.tmp" "$STAGE_DIR"
  elif [[ -d "${STAGE_DIR}.tmp/runtime/pgsql" ]]; then
    rm -rf "$STAGE_DIR"
    mv "${STAGE_DIR}.tmp/runtime" "$STAGE_DIR"
    rm -rf "${STAGE_DIR}.tmp"
  else
    # single top dir
    local top
    top="$(find "${STAGE_DIR}.tmp" -mindepth 1 -maxdepth 1 -type d | head -1)"
    if [[ -n "$top" && -d "$top/pgsql" ]]; then
      rm -rf "$STAGE_DIR"
      mv "$top" "$STAGE_DIR"
      rm -rf "${STAGE_DIR}.tmp"
    else
      die "unexpected zip layout under ${STAGE_DIR}.tmp"
    fi
  fi
  cmd_verify
  log "fetch done → $STAGE_DIR"
}

# ── pack: zip staged tree for VPS ────────────────────────────────────────────

cmd_pack() {
  load_pins
  need_cmd zip
  cmd_verify
  mkdir -p "$OUT_ROOT/$PLATFORM"
  local zip_path="$OUT_ROOT/$PLATFORM/$COS_RUNTIME_ZIP_NAME"
  local sha_path="${zip_path}.sha256"
  rm -f "$zip_path" "$sha_path"
  log "zip $STAGE_DIR → $zip_path"
  (
    cd "$STAGE_DIR"
    zip -qr "$zip_path" .
  )
  local sum
  sum="$(sha256_of "$zip_path")"
  printf '%s  %s\n' "$sum" "$(basename "$zip_path")" >"$sha_path"
  # also flat path for convenience
  cp -f "$zip_path" "$OUT_ROOT/$COS_RUNTIME_ZIP_NAME"
  cp -f "$sha_path" "$OUT_ROOT/${COS_RUNTIME_ZIP_NAME}.sha256"

  local size_bytes
  size_bytes="$(wc -c <"$zip_path" | tr -d ' ')"
  cat >"$OUT_ROOT/manifest.json" <<EOF
{
  "schema": 1,
  "runtime_id": "${COS_RUNTIME_ID}",
  "generated_at": "$(date -u +%Y-%m-%dT%H:%M:%SZ)",
  "public_base": "/releases/desktop/runtime",
  "platforms": {
    "${PLATFORM}": {
      "file": "${COS_RUNTIME_ZIP_NAME}",
      "path": "${PLATFORM}/${COS_RUNTIME_ZIP_NAME}",
      "sha256": "${sum}",
      "bytes": ${size_bytes},
      "components": {
        "postgresql": "${PG_WIN_VERSION:-}",
        "pgvector": "${PGVECTOR_VERSION:-}",
        "redis": "${REDIS_WIN_VERSION:-}"
      }
    }
  },
  "notes": "Builders only. End users download full setup.exe with runtime embedded."
}
EOF
  cp -f "$OUT_ROOT/manifest.json" "$CACHE_DIR/manifest.json" 2>/dev/null || true
  log "pack ok"
  log "  zip:      $zip_path ($(du -h "$zip_path" | awk '{print $1}'))"
  log "  sha256:   $sum"
  log "  manifest: $OUT_ROOT/manifest.json"
  log "next: bash scripts/publish-desktop-bundled-runtime.sh"
}

# ── assemble: build stage from upstream component zips ───────────────────────

cmd_assemble() {
  load_pins
  need_cmd curl
  need_cmd unzip
  mkdir -p "$CACHE_DIR" "$STAGE_DIR"

  local pg_zip="$CACHE_DIR/postgresql-${PG_WIN_VERSION}-win-x64-binaries.zip"
  local redis_zip="$CACHE_DIR/Redis-x64-${REDIS_WIN_VERSION}.zip"

  download "$PG_WIN_URL" "$pg_zip"
  verify_sha "$pg_zip" "${PG_WIN_SHA256:-}" "postgresql zip"
  download "$REDIS_WIN_URL" "$redis_zip"
  verify_sha "$redis_zip" "${REDIS_WIN_SHA256:-}" "redis zip"

  log "assemble clean $STAGE_DIR"
  rm -rf "$STAGE_DIR"
  mkdir -p "$STAGE_DIR/pgsql" "$STAGE_DIR/redis"

  local pg_extract="$CACHE_DIR/extract-pg-$$"
  local redis_extract="$CACHE_DIR/extract-redis-$$"
  rm -rf "$pg_extract" "$redis_extract"
  mkdir -p "$pg_extract" "$redis_extract"
  unzip -q -o "$pg_zip" -d "$pg_extract"
  unzip -q -o "$redis_zip" -d "$redis_extract"

  # EDB zip usually has pgsql/ at top or nested
  if [[ -d "$pg_extract/pgsql" ]]; then
    cp -a "$pg_extract/pgsql/." "$STAGE_DIR/pgsql/"
  elif [[ -d "$pg_extract/bin" ]]; then
    cp -a "$pg_extract/." "$STAGE_DIR/pgsql/"
  else
    local pg_top
    pg_top="$(find "$pg_extract" -type d -name pgsql | head -1)"
    [[ -n "$pg_top" ]] || die "cannot find pgsql/ in $pg_zip"
    cp -a "$pg_top/." "$STAGE_DIR/pgsql/"
  fi

  # Prune bulk we do not need in install package
  rm -rf \
    "$STAGE_DIR/pgsql/doc" \
    "$STAGE_DIR/pgsql/pgAdmin"* \
    "$STAGE_DIR/pgsql/StackBuilder"* \
    "$STAGE_DIR/pgsql/include" \
    "$STAGE_DIR/pgsql/symbols" \
    2>/dev/null || true
  # Drop most locales keep C/en
  if [[ -d "$STAGE_DIR/pgsql/share/locale" ]]; then
    find "$STAGE_DIR/pgsql/share/locale" -mindepth 1 -maxdepth 1 -type d \
      ! -name 'en*' ! -name 'C' -exec rm -rf {} + 2>/dev/null || true
  fi

  # Redis: find redis-server.exe
  local rbin
  rbin="$(find "$redis_extract" -type f \( -name 'redis-server.exe' -o -name 'redis-server' \) | head -1)"
  [[ -n "$rbin" ]] || die "redis-server not found in $redis_zip"
  cp -f "$rbin" "$STAGE_DIR/redis/"
  local rcli
  rcli="$(find "$redis_extract" -type f \( -name 'redis-cli.exe' -o -name 'redis-cli' \) | head -1 || true)"
  [[ -n "$rcli" ]] && cp -f "$rcli" "$STAGE_DIR/redis/"

  # pgvector optional input
  if [[ -n "${PGVECTOR_WIN_ZIP:-}" ]]; then
    local vz="$PGVECTOR_WIN_ZIP"
    if [[ ! -f "$vz" ]]; then
      download "$vz" "$CACHE_DIR/$(basename "$vz")"
      vz="$CACHE_DIR/$(basename "$vz")"
    fi
    verify_sha "$vz" "${PGVECTOR_WIN_SHA256:-}" "pgvector zip"
    local vx="$CACHE_DIR/extract-vector-$$"
    rm -rf "$vx" && mkdir -p "$vx"
    unzip -q -o "$vz" -d "$vx"
    find "$vx" -name 'vector.dll' -exec cp -f {} "$STAGE_DIR/pgsql/lib/" \;
    find "$vx" -name 'vector.control' -exec cp -f {} "$STAGE_DIR/pgsql/share/extension/" \;
    find "$vx" -name 'vector--*.sql' -exec cp -f {} "$STAGE_DIR/pgsql/share/extension/" \;
    rm -rf "$vx"
  elif [[ -f "$CACHE_DIR/vector.dll" ]]; then
    mkdir -p "$STAGE_DIR/pgsql/lib" "$STAGE_DIR/pgsql/share/extension"
    cp -f "$CACHE_DIR/vector.dll" "$STAGE_DIR/pgsql/lib/"
    [[ -f "$CACHE_DIR/vector.control" ]] && cp -f "$CACHE_DIR/vector.control" "$STAGE_DIR/pgsql/share/extension/"
    cp -f "$CACHE_DIR"/vector--*.sql "$STAGE_DIR/pgsql/share/extension/" 2>/dev/null || true
    log "placed pgvector from $CACHE_DIR/vector.*"
  else
    log "warn: no pgvector yet — drop vector.dll + vector.control + vector--*.sql into"
    log "      $CACHE_DIR/ or set PGVECTOR_WIN_ZIP, then re-run assemble / copy manually"
    mkdir -p "$STAGE_DIR/pgsql/lib" "$STAGE_DIR/pgsql/share/extension"
  fi

  write_stage_meta
  rm -rf "$pg_extract" "$redis_extract"

  log "assembled tree size: $(du -sh "$STAGE_DIR" | awk '{print $1}')"
  log "recorded sha256 of component zips (paste into pins.env if empty):"
  log "  PG_WIN_SHA256=$(sha256_of "$pg_zip")"
  log "  REDIS_WIN_SHA256=$(sha256_of "$redis_zip")"

  if [[ -f "$STAGE_DIR/pgsql/lib/vector.dll" || -f "$STAGE_DIR/pgsql/lib/vector.so" ]]; then
    cmd_verify
  else
    log "partial assemble OK (missing pgvector) — not pack-ready until vector is present"
    local pg="$STAGE_DIR/pgsql/bin/$(pg_ctl_name)"
    local redis="$STAGE_DIR/redis/$(redis_name)"
    [[ -f "$pg" && -f "$redis" ]] || die "assemble incomplete: pg or redis missing"
  fi
  log "assemble done → $STAGE_DIR"
}

usage() {
  cat <<EOF
Usage: bash scripts/stage-desktop-bundled-runtime.sh <command>

Commands:
  status     Show pins, stage dir, cache, pack outputs
  fetch      Download cos-runtime zip from VPS public URL → stage dir (sha256)
  pack       Zip stage dir → dist/desktop-runtime/ + manifest.json
  verify     Check stage dir has pg_ctl + redis + pgvector + runtime.version
  assemble   Build stage from PG/Redis (and optional vector) component downloads

Env:
  DESKTOP_RUNTIME_PUBLIC_BASE  default $PUBLIC_BASE
  COS_RUNTIME_CACHE            default ~/.cache/context-osv6/bundled-runtime
  COS_RUNTIME_STAGE            override stage directory
  DESKTOP_RUNTIME_OUT          default dist/desktop-runtime
  PGVECTOR_WIN_ZIP             path or URL for prebuilt vector (assemble)
EOF
}

main() {
  local cmd="${1:-status}"
  case "$cmd" in
    status) cmd_status ;;
    fetch) cmd_fetch ;;
    pack) cmd_pack ;;
    verify) load_pins; cmd_verify ;;
    assemble) cmd_assemble ;;
    -h|--help|help) usage ;;
    *) usage; die "unknown command: $cmd" ;;
  esac
}

main "$@"
