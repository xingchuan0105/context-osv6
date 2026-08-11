#!/usr/bin/env bash
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT"

if [[ -f "$ROOT/../avrag-rs/.env" ]]; then
  set -a
  # shellcheck disable=SC1091
  source "$ROOT/../avrag-rs/.env"
  set +a
fi

: "${DATABASE_URL:?DATABASE_URL required}"

mkdir -p bin
echo "==> build retrieval-mcp"
go build -o bin/retrieval-mcp ./cmd/retrieval-mcp
echo "==> build retrieval-client"
go build -o bin/retrieval-client ./cmd/retrieval-client

# workspace with sample CJK corpus (override as needed)
export OSV7_WORKSPACE_ID="${OSV7_WORKSPACE_ID:-0c8391f1-8bfb-415f-9a7f-10624b7cfb4d}"
export RETRIEVAL_MCP_BIN="$ROOT/bin/retrieval-mcp"

echo "==> run client (card gate + lexical + dense + handles)"
./bin/retrieval-client "${1:-滴灌通}"
echo "==> P1 smoke OK"
