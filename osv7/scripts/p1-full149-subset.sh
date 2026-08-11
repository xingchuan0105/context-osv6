#!/usr/bin/env bash
# P1 closeout: Layer-A retrieval subset against golden_set_realistic (full-149).
# Default mode=available (only cases with gold needles present in local PG).
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

MODE="${1:-available}"   # available | all
TOOLS="${TOOLS:-lexical,dense}"
K="${K:-15}"
STAMP="$(date -u +%Y%m%dT%H%M%SZ)"
OUT_DIR="$ROOT/docs/_reports"
mkdir -p "$OUT_DIR" bin
OUT="$OUT_DIR/p1-full149-${MODE}-${STAMP}.json"

echo "==> build retrieval-eval"
go build -o bin/retrieval-eval ./cmd/retrieval-eval

GOLDEN="${GOLDEN:-$ROOT/../avrag-rs/tests/rag_quality/golden_set_realistic.json}"
echo "==> run mode=$MODE tools=$TOOLS k=$K golden=$GOLDEN"
set +e
./bin/retrieval-eval \
  -golden "$GOLDEN" \
  -mode "$MODE" \
  -tools "$TOOLS" \
  -k "$K" \
  -out "$OUT" \
  -fail-below "${FAIL_BELOW:-0.5}"
EC=$?
set -e

echo "==> report: $OUT"
# print compact summary from report
python3 - <<PY
import json
p="$OUT"
d=json.load(open(p))
print(f"eligible={d['eligible']} ran={d['ran']} skipped={d['skipped']} hits={d['hits']} hit_rate={d['hit_rate']:.3f} mean_recall={d['mean_recall']:.3f}")
hits=[c for c in d['cases'] if c.get('hit')]
miss=[c for c in d['cases'] if not c.get('skipped') and not c.get('hit') and not c.get('error')]
print("HITS:")
for c in hits:
    print(f"  + {c['subset']}: {c['query'][:60]}")
print("MISSES:")
for c in miss[:20]:
    print(f"  - {c['subset']}: recall={c['recall']:.2f} q={c['query'][:60]}")
PY

# also refresh latest symlink-ish copy
cp -f "$OUT" "$OUT_DIR/p1-full149-${MODE}-latest.json"
echo "==> latest: $OUT_DIR/p1-full149-${MODE}-latest.json"
exit $EC
