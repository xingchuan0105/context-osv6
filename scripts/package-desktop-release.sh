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

if [[ -n "$NSIS" && -f "$NSIS" ]]; then
  FORMAT="nsis"
  SRC="$NSIS"
  OUT_NAME="AVRag-Desktop_${VERSION}_x64-setup.exe"
elif [[ -n "$PORTABLE" && -f "$PORTABLE" ]]; then
  FORMAT="portable"
  SRC="$PORTABLE"
  OUT_NAME="AVRag-Desktop_${VERSION}_x64.exe"
  echo "package-desktop-release: warning: no NSIS setup found; packaging portable exe" >&2
else
  die "no Windows artifact under $TAURI_TARGET_BASE (build with scripts/build-windows.sh first)"
fi

DEST="$STAGE/$OUT_NAME"
cp -f "$SRC" "$DEST"
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
  "product": "AVRag Desktop",
  "version": "${VERSION}",
  "published_at": "${PUBLISHED_AT}",
  "platforms": {
    "windows-x64": {
      "url": "${REL_URL}",
      "sha256": "${SHA}",
      "size_bytes": ${SIZE},
      "format": "${FORMAT}",
      "filename": "${OUT_NAME}"
    }
  },
  "min_os": "Windows 10 64-bit (WebView2)",
  "notes_url": "/desktop"
}
EOF

# Copy latest into version dir for archival
cp -f "$OUT_ROOT/latest.json" "$STAGE/latest.json"

echo "package-desktop-release: version=${VERSION}"
echo "  artifact: $DEST"
echo "  format:   $FORMAT"
echo "  sha256:   $SHA"
echo "  size:     $SIZE"
echo "  latest:   $OUT_ROOT/latest.json"
echo "  public:   $REL_URL"
