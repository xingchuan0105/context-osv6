#!/usr/bin/env bash
# P3: dual producers ingest → retrieval visible.
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT"

if [[ -f "$ROOT/../avrag-rs/.env" ]]; then
  set -a
  # shellcheck disable=SC1091
  source "$ROOT/../avrag-rs/.env"
  set +a
fi
: "${DATABASE_URL:?DATABASE_URL}"
: "${EMBEDDING_API_KEY:?EMBEDDING_API_KEY}"

# fresh workspace for isolation
WS=$(python3 -c 'import uuid; print(uuid.uuid4())')
echo "workspace=$WS"
mkdir -p bin /tmp/osv7-p3
echo "==> build"
go test ./internal/ingest/ -count=1
go build -o bin/ingest-cli ./cmd/ingest-cli
go build -o bin/ingest-mcp ./cmd/ingest-mcp
go build -o bin/retrieval-cli ./cmd/retrieval-cli

# --- producer A: agent package ---
cat > /tmp/osv7-p3/agent-package.json <<'JSON'
{
  "title": "P3 agent package sample",
  "doc_type": "markdown",
  "primary_backend": "agent_package",
  "summary": "本文说明 osv7 P3 摄入契约与检索可见性验证。",
  "blocks": [
    {
      "block_type": "heading",
      "text": "osv7 摄入腿 P3"
    },
    {
      "block_type": "paragraph",
      "text": "唯一不重复标记 ALPHA-P3-AGENT-PACKAGE-9917 用于验证 agent 制备包写入后可被 lexical 命中。"
    },
    {
      "block_type": "paragraph",
      "text": "DocumentIr 经硬校验与 embedding 后进入 rag_text_chunks。"
    }
  ]
}
JSON

echo "==> agent-package producer"
./bin/ingest-cli agent-package --workspace "$WS" --file /tmp/osv7-p3/agent-package.json | tee /tmp/osv7-p3/agent-out.json
DOC_A=$(python3 -c 'import json;print(json.load(open("/tmp/osv7-p3/agent-out.json"))["doc_id"])')
echo "doc_a=$DOC_A"

# --- producer B: server parse text ---
cat > /tmp/osv7-p3/server-doc.md <<'MD'
# P3 零配置解析样本

第二段包含唯一标记 BETA-P3-SERVER-PARSE-8826，验证服务端文本解析生产者。

第三段说明 markitdown/anydoc 适配器后续可挂；本 smoke 使用纯文本切分。
MD

echo "==> server-parse producer"
./bin/ingest-cli server-parse --workspace "$WS" --file /tmp/osv7-p3/server-doc.md | tee /tmp/osv7-p3/server-out.json
DOC_B=$(python3 -c 'import json;print(json.load(open("/tmp/osv7-p3/server-out.json"))["doc_id"])')
echo "doc_b=$DOC_B"

# --- retrieval visible ---
export OSV7_RETRIEVAL_STATE=/tmp/osv7-p3/ret-state.json
rm -f "$OSV7_RETRIEVAL_STATE"
./bin/retrieval-cli set-card --workspace "$WS" --actions lexical >/dev/null
echo "==> lexical ALPHA"
./bin/retrieval-cli lexical --query "ALPHA-P3-AGENT-PACKAGE-9917" --limit 5 | tee /tmp/osv7-p3/lex-a.json
echo "==> lexical BETA"
./bin/retrieval-cli lexical --query "BETA-P3-SERVER-PARSE-8826" --limit 5 | tee /tmp/osv7-p3/lex-b.json

python3 - <<'PY'
import json
for tag, path in [("ALPHA","/tmp/osv7-p3/lex-a.json"),("BETA","/tmp/osv7-p3/lex-b.json")]:
    d=json.load(open(path))
    n=d.get("total_hits") or 0
    print(tag, "hits", n)
    assert n>=1, d
    blob=json.dumps(d, ensure_ascii=False)
    assert tag.split("-")[0] in blob or tag in blob or "P3" in blob
print("dual producers retrieval-visible OK")
PY

echo "==> P3 ingest smoke OK workspace=$WS"
