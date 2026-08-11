#!/usr/bin/env bash
# P0: build MCP server, call lexical against v6 rag_text_chunks.
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT"

if [[ -f "$ROOT/../avrag-rs/.env" ]]; then
  set -a
  # shellcheck disable=SC1091
  source "$ROOT/../avrag-rs/.env"
  set +a
fi

if [[ -z "${DATABASE_URL:-}" ]]; then
  echo "DATABASE_URL missing (source avrag-rs/.env)" >&2
  exit 1
fi

mkdir -p bin
echo "==> build hello-retrieval-mcp"
go build -o bin/hello-retrieval-mcp ./cmd/hello-retrieval-mcp
echo "==> build hello-retrieval-client"
go build -o bin/hello-retrieval-client ./cmd/hello-retrieval-client

QUERY="${1:-滴灌通}"
echo "==> CallTool lexical query=$QUERY"
HELLO_RETRIEVAL_MCP_BIN="$ROOT/bin/hello-retrieval-mcp" ./bin/hello-retrieval-client "$QUERY"
echo "==> OK"
