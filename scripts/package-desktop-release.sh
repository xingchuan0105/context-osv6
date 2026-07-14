#!/usr/bin/env bash
# Collect Tauri Windows artifacts → dist/desktop-release/v{version}/ + latest.json
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
DESKTOP="$ROOT/desktop"
TAURI_TARGET_BASE="$DESKTOP/src-tauri/target"
VERSION="$(node -p "require('$DESKTOP/package.json').version")"
OUT_ROOT="${DESKTOP_RELEASE_OUT:-$ROOT/dist/desktop-release}"
STAGE="$OUT_ROOT/v${VERSION}"
PUBLIC_BASE="${DESKTOP_PUBLIC_BASE:-/releases/desktop}"

die() { echo "package-desktop-release: $*" >&2; exit 1; }

mkdir -p "$STAGE"

# Prefer NSIS setup under any target triple, then portable exe
find_nsis() {
  find "$TAURI_TARGET_BASE" -type f \( -name '*-setup.exe' -o -name '*setup.exe' \) 2>/dev/null \
    | grep -E 'bundle/nsis|bundle\\nsis' | head -1 || true
}

find_portable() {
  # Prefer release avrag-desktop.exe (not deps/)
  local candidates
  candidates="$(find "$TAURI_TARGET_BASE" -type f -path '*/release/avrag-desktop.exe' 2>/dev/null | head -5)"
  echo "$candidates" | head -1
}

NSIS="$(find_nsis)"
PORTABLE="$(find_portable)"

FORMAT=""
SRC=""
OUT_NAME=""

# Prefer NSIS. Set ALLOW_PORTABLE=1 only for emergency fallback.
ALLOW_PORTABLE="${ALLOW_PORTABLE:-0}"

if [[ -n "$NSIS" && -f "$NSIS" ]]; then
  FORMAT="nsis"
  SRC="$NSIS"
  OUT_NAME="Context-OS-Client_${VERSION}_x64-setup.exe"
elif [[ "$ALLOW_PORTABLE" == "1" && -n "$PORTABLE" && -f "$PORTABLE" ]]; then
  FORMAT="portable"
  SRC="$PORTABLE"
  OUT_NAME="Context-OS-Client_${VERSION}_x64.exe"
  echo "package-desktop-release: warning: packaging portable exe (ALLOW_PORTABLE=1)" >&2
else
  die "no NSIS *-setup.exe under $TAURI_TARGET_BASE (run: bash scripts/build-windows.sh). Portable fallback: ALLOW_PORTABLE=1"
fi

DEST="$STAGE/$OUT_NAME"
cp -f "$SRC" "$DEST"

# Authenticode (optional). Production: WINDOWS_CERTIFICATE_FILE + PASSWORD.
# Dev: SIGN_ALLOW_SELF_SIGNED=1 (default when SIGN_WINDOWS=1 and no pfx).
SIGN_WINDOWS="${SIGN_WINDOWS:-1}"
SIGNED_JSON=false
if [[ "$SIGN_WINDOWS" == "1" ]]; then
  if [[ -z "${WINDOWS_CERTIFICATE_FILE:-}" && -z "${SIGN_ALLOW_SELF_SIGNED:-}" ]]; then
    export SIGN_ALLOW_SELF_SIGNED=1
  fi
  if bash "$ROOT/scripts/sign-windows-release.sh" "$DEST"; then
    SIGNED_JSON=true
  else
    echo "package-desktop-release: warning: signing failed; shipping unsigned" >&2
  fi
fi

SIZE="$(wc -c < "$DEST" | tr -d ' ')"
SHA="$(sha256sum "$DEST" | awk '{print $1}')"

# SHA256SUMS
(
  cd "$STAGE"
  sha256sum "$OUT_NAME" > SHA256SUMS
)

REL_URL="${PUBLIC_BASE}/v${VERSION}/${OUT_NAME}"
PUBLISHED_AT="$(date -u +%Y-%m-%dT%H:%M:%SZ)"

# latest.json (root of release out)
cat > "$OUT_ROOT/latest.json" <<EOF
{
  "product": "Context-OS Client",
  "version": "${VERSION}",
  "published_at": "${PUBLISHED_AT}",
  "platforms": {
    "windows-x64": {
      "url": "${REL_URL}",
      "sha256": "${SHA}",
      "size_bytes": ${SIZE},
      "format": "${FORMAT}",
      "filename": "${OUT_NAME}",
      "authenticode": ${SIGNED_JSON}
    }
  },
  "min_os": "Windows 10 64-bit (WebView2)",
  "notes_url": "/desktop"
}
EOF

# Copy latest into version dir for archival
cp -f "$OUT_ROOT/latest.json" "$STAGE/latest.json"

# Stage product sidecars + runtime layout next to installer (companion pack).
# Windows NSIS already embeds externalBin when built via build-windows.sh;
# this folder is a fallback / monorepo pack and for CONTEXT_OS_CLIENT_HOME.
SIDECAR_STAGE="$STAGE/runtime-sidecars"
mkdir -p "$SIDECAR_STAGE/bin"
WIN_TRIPLE="${STAGE_TARGET_TRIPLE:-x86_64-pc-windows-gnu}"
# Prefer already-staged Windows sidecars (from build-windows); else host stage.
if [[ -f "$ROOT/desktop/src-tauri/binaries/avrag-api-${WIN_TRIPLE}.exe" ]]; then
  cp -f "$ROOT/desktop/src-tauri/binaries/avrag-api-${WIN_TRIPLE}.exe" "$SIDECAR_STAGE/bin/avrag-api.exe" || true
  cp -f "$ROOT/desktop/src-tauri/binaries/avrag-worker-${WIN_TRIPLE}.exe" "$SIDECAR_STAGE/bin/avrag-worker.exe" || true
elif bash "$ROOT/scripts/stage-desktop-sidecars.sh" 2>/dev/null; then
  if [[ -d "$ROOT/desktop/runtime/bin" ]]; then
    cp -a "$ROOT/desktop/runtime/bin/." "$SIDECAR_STAGE/bin/" 2>/dev/null || true
  fi
fi
cp -f "$ROOT/desktop/runtime/docker-compose.client.yml" "$SIDECAR_STAGE/" 2>/dev/null || true
cp -f "$ROOT/desktop/runtime/README.md" "$SIDECAR_STAGE/" 2>/dev/null || true
cp -f "$ROOT/scripts/desktop-local-stack.sh" "$SIDECAR_STAGE/" 2>/dev/null || true
cp -f "$ROOT/scripts/desktop-local-product.sh" "$SIDECAR_STAGE/" 2>/dev/null || true
cat >"$SIDECAR_STAGE/INSTALL.txt" <<'SIDEEOF'
Context-OS Client — companion runtime (data plane + product binaries)

1. Install Docker Desktop (Windows/Mac) or Docker Engine (Linux).
   Windows: https://docs.docker.com/desktop/setup/install/windows-install/
2. Start Docker until the engine is Running.
3. The NSIS installer embeds avrag-api.exe / avrag-worker.exe next to the app
   when built with scripts/build-windows.sh (externalBin).
4. Optional: set CONTEXT_OS_CLIENT_HOME to this folder for script-based control.
5. API: http://127.0.0.1:18080 · local user local@context-os.client (no cloud login).

Settings → 本机数据栈 shows Docker probe + install guide if the engine is missing.
SIDEEOF

# Tauri externalBin staging dir (windows triple + host if present)
if [[ -d "$ROOT/desktop/src-tauri/binaries" ]]; then
  mkdir -p "$STAGE/tauri-binaries"
  cp -a "$ROOT/desktop/src-tauri/binaries/." "$STAGE/tauri-binaries/" 2>/dev/null || true
fi

echo "package-desktop-release: version=${VERSION}"
echo "  artifact: $DEST"
echo "  format:   $FORMAT"
echo "  signed:   $SIGNED_JSON"
echo "  sha256:   $SHA"
echo "  size:     $SIZE"
echo "  latest:   $OUT_ROOT/latest.json"
echo "  public:   $REL_URL"
echo "  sidecars: $SIDECAR_STAGE"
