#!/usr/bin/env bash
# Mirror WSL monorepo → C:\dev\context-osv6 for Windows-native `pnpm tauri dev`.
# Excludes heavy caches. Run after editing in WSL, before Windows-side tauri dev.
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
DEST="${WIN_DEV_ROOT:-/mnt/c/dev/context-osv6}"
mkdir -p "$DEST"
rsync -a \
  --exclude 'node_modules' \
  --exclude 'target' \
  --exclude '.next' \
  --exclude 'out' \
  --exclude 'avrag-rs/target' \
  --exclude 'avrag-rs/volumes' \
  --exclude 'avrag-rs/storage' \
  --exclude 'avrag-rs/crates/app/tests' \
  --exclude 'desktop/runtime/data' \
  --exclude 'desktop/runtime/logs' \
  --exclude 'desktop/runtime/run' \
  --exclude '.git' \
  --exclude 'dist' \
  --exclude 'logs' \
  "$ROOT/desktop" \
  "$ROOT/frontend_next" \
  "$ROOT/contracts" \
  "$ROOT/avrag-rs" \
  "$DEST/"
echo "sync-windows-dev: -> $DEST"
ls -la "$DEST"
