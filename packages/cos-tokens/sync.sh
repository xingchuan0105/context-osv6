#!/usr/bin/env bash
# Sync Cream × Void tokens + mark to product and satellite sites.
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
SRC="$ROOT/packages/cos-tokens"
TOKENS="$SRC/tokens.css"
MARK="$SRC/mark.svg"

die() { echo "sync: $*" >&2; exit 1; }
[[ -f "$TOKENS" ]] || die "missing $TOKENS"

copy_tokens() {
  local dest="$1"
  mkdir -p "$(dirname "$dest")"
  cp -f "$TOKENS" "$dest"
  echo "  tokens → $dest"
}

copy_mark() {
  local dest="$1"
  mkdir -p "$(dirname "$dest")"
  cp -f "$MARK" "$dest"
  echo "  mark   → $dest"
}

echo "== cos-tokens sync =="

# App (in monorepo)
copy_tokens "$ROOT/frontend_next/app/design-tokens.css"
copy_mark "$ROOT/frontend_next/public/brand/context-os-mark.svg"

# Sibling repos (optional if present)
if [[ -d /home/chuan/context-os-landing ]]; then
  copy_tokens /home/chuan/context-os-landing/styles/cos-tokens.css
  copy_mark /home/chuan/context-os-landing/public/brand/context-os-mark.svg
fi
if [[ -d /home/chuan/whyiamright/frontend ]]; then
  copy_tokens /home/chuan/whyiamright/frontend/src/styles/cos-tokens.css
  copy_mark /home/chuan/whyiamright/frontend/public/brand/context-os-mark.svg
fi
if [[ -d /home/chuan/context-os-theme ]]; then
  copy_tokens /home/chuan/context-os-theme/assets/css/tokens.css
  copy_mark /home/chuan/context-os-theme/assets/images/context-os-mark.svg
fi
if [[ -d /home/chuan/cchess/frontend ]]; then
  copy_tokens /home/chuan/cchess/frontend/src/cos-tokens.css
  copy_mark /home/chuan/cchess/frontend/public/brand/context-os-mark.svg
fi

echo "done."
