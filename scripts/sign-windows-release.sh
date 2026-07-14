#!/usr/bin/env bash
# Authenticode-sign a Windows PE (NSIS setup / portable exe) via osslsigncode.
#
# Preferred (production OV/EV):
#   WINDOWS_CERTIFICATE_FILE=/path/to/codesign.pfx
#   WINDOWS_CERTIFICATE_PASSWORD=...
#
# Dev fallback (self-signed, SmartScreen still warns "unknown publisher"):
#   SIGN_ALLOW_SELF_SIGNED=1  → uses/creates desktop/signing/dev-codesign.pfx
#
# Optional:
#   WINDOWS_TIMESTAMP_URL=http://timestamp.digicert.com
#   WINDOWS_SIGN_DESCRIPTION="Context-OS"
#   WINDOWS_SIGN_URL=https://app.contextlm.top
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
die() { echo "sign-windows-release: $*" >&2; exit 1; }
log() { echo "sign-windows-release: $*"; }

INPUT="${1:-}"
[[ -n "$INPUT" && -f "$INPUT" ]] || die "usage: $0 <path-to.exe>"

command -v osslsigncode >/dev/null 2>&1 || die "osslsigncode missing (sudo apt-get install -y osslsigncode)"

CERT_FILE="${WINDOWS_CERTIFICATE_FILE:-}"
CERT_PASS="${WINDOWS_CERTIFICATE_PASSWORD:-}"
TS_URL="${WINDOWS_TIMESTAMP_URL:-http://timestamp.digicert.com}"
DESC="${WINDOWS_SIGN_DESCRIPTION:-Context-OS Client}"
URL="${WINDOWS_SIGN_URL:-https://app.contextlm.top}"
ALLOW_SELF="${SIGN_ALLOW_SELF_SIGNED:-0}"
SIGNING_DIR="$ROOT/desktop/signing"
DEV_PFX="$SIGNING_DIR/dev-codesign.pfx"
DEV_PASS_FILE="$SIGNING_DIR/dev-codesign.pass"

ensure_dev_cert() {
  mkdir -p "$SIGNING_DIR"
  chmod 700 "$SIGNING_DIR"
  if [[ -f "$DEV_PFX" && -f "$DEV_PASS_FILE" ]]; then
    return 0
  fi
  log "generating self-signed code-signing cert (dev only)…"
  local key="$SIGNING_DIR/dev-key.pem" cert="$SIGNING_DIR/dev-cert.pem" pass
  pass="$(openssl rand -base64 24 | tr -d '/+=' | head -c 24)"
  openssl req -x509 -newkey rsa:4096 -sha256 -days 825 \
    -keyout "$key" -out "$cert" -nodes \
    -subj "/CN=Context-OS Desktop Dev/O=Context-OS/C=CN" \
    -addext "extendedKeyUsage=codeSigning" \
    -addext "keyUsage=digitalSignature" >/dev/null 2>&1
  openssl pkcs12 -export -out "$DEV_PFX" -inkey "$key" -in "$cert" \
    -passout "pass:${pass}" -name "Context-OS Desktop Dev"
  printf '%s' "$pass" > "$DEV_PASS_FILE"
  chmod 600 "$DEV_PFX" "$DEV_PASS_FILE" "$key" "$cert"
  # Do not commit these; desktop/signing/ is gitignored
  log "wrote $DEV_PFX"
}

if [[ -z "$CERT_FILE" || ! -f "$CERT_FILE" ]]; then
  if [[ "$ALLOW_SELF" == "1" ]]; then
    ensure_dev_cert
    CERT_FILE="$DEV_PFX"
    CERT_PASS="$(cat "$DEV_PASS_FILE")"
    log "using self-signed cert (SmartScreen will still warn until OV/EV cert)"
  else
    die "set WINDOWS_CERTIFICATE_FILE (+ PASSWORD), or SIGN_ALLOW_SELF_SIGNED=1 for dev cert"
  fi
fi

# osslsigncode refuses to overwrite an existing -out path
TMP="$(mktemp -u "${INPUT}.sign.XXXXXX.exe")"
cleanup() { rm -f "$TMP"; }
trap cleanup EXIT

run_sign() {
  local use_ts="$1"
  local -a args=(
    sign
    -pkcs12 "$CERT_FILE"
    -n "$DESC"
    -i "$URL"
    -in "$INPUT"
    -out "$TMP"
  )
  if [[ "$use_ts" == "1" ]]; then
    args+=(-t "$TS_URL")
  fi
  if [[ -n "$CERT_PASS" ]]; then
    args+=(-pass "$CERT_PASS")
  fi
  rm -f "$TMP"
  osslsigncode "${args[@]}"
}

log "signing $(basename "$INPUT")…"
if ! run_sign 1; then
  log "timestamp failed; retry without timestamp server…"
  run_sign 0
fi

# verify (self-signed may fail chain; PE signature presence still OK)
if ! osslsigncode verify -in "$TMP" >/dev/null 2>&1; then
  log "warn: full chain verify failed (common for self-signed); checking PE signature blob…"
  osslsigncode extract-signature -in "$TMP" -out "${TMP}.sig" >/dev/null 2>&1 \
    || die "no Authenticode signature written"
  rm -f "${TMP}.sig"
fi

mv -f "$TMP" "$INPUT"
trap - EXIT
log "signed in place: $INPUT"
