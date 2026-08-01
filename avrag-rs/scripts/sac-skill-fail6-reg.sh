#!/usr/bin/env bash
# Skill-regression subset for knowledge-base progressive skills (P2).
#
# After editing prompts/clusters/knowledge-base/**, run this before full golden-14.
# Uses product E2E realistic_corpus with E2E_QUESTIONS filter — no golden text
# is embedded here (question indices only).
#
# Fail modes covered (1-based indices into golden_set_realistic.json order):
#   65  — high-variance / refusal-ish
#   86  — table sort-key (sticky)
#   88  — table multi-count / total_hits
#   105 — cross-doc similarity
#   106 — multi-claim half-coverage (sticky)
#   121 — joint dual-source
#
# Usage (from repo root or avrag-rs):
#   bash avrag-rs/scripts/sac-skill-fail6-reg.sh
#   QUESTIONS=86,106 bash avrag-rs/scripts/sac-skill-fail6-reg.sh   # tighter slice
#   DRY_RUN=1 bash avrag-rs/scripts/sac-skill-fail6-reg.sh          # print env only
#
# Env (optional):
#   QUESTIONS   default 65,86,88,105,106,121
#   LOG_DIR     default /tmp/sac_e2e
#   E2E_MODE    default nightly
#   extra cargo/test env passes through unchanged

set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
AVRAG_RS="${ROOT}/avrag-rs"
QUESTIONS="${QUESTIONS:-65,86,88,105,106,121}"
LOG_DIR="${LOG_DIR:-/tmp/sac_e2e}"
E2E_MODE="${E2E_MODE:-nightly}"
STAMP="$(date +%Y%m%d-%H%M%S)"
LOG="${LOG_DIR}/fail6_${STAMP}.log"

mkdir -p "${LOG_DIR}"

if [[ ! -f "${AVRAG_RS}/.env" ]]; then
  echo "missing ${AVRAG_RS}/.env (credentials); refuse to run" >&2
  exit 1
fi

# shellcheck disable=SC1091
set -a
# Prefer avrag-rs/.env for product keys; do not echo secrets.
source "${AVRAG_RS}/.env"
set +a

export E2E_MODE
export E2E_QUESTIONS="${QUESTIONS}"
# Judge-first scorecard; keep default v2 unless caller opts out.
export RAG_EVAL_V2="${RAG_EVAL_V2:-1}"
export RAG_EVAL_V2_ONLY="${RAG_EVAL_V2_ONLY:-1}"

echo "LOG=${LOG}"
echo "start $(date -Iseconds)"
echo "E2E_QUESTIONS=${E2E_QUESTIONS}"
echo "E2E_MODE=${E2E_MODE}"
echo "cwd=${AVRAG_RS}"

if [[ "${DRY_RUN:-0}" == "1" ]]; then
  echo "DRY_RUN=1 — not invoking cargo"
  exit 0
fi

{
  echo "LOG=${LOG}"
  echo "start $(date -Iseconds)"
  echo "E2E_QUESTIONS=${E2E_QUESTIONS}"
  cd "${AVRAG_RS}"
  # #[ignore] real-LLM test; needs product-e2e feature + --ignored (see rag_quality_prod.rs).
  # jobs=2 discipline for WSL — caller may wrap with taskset; keep --test-threads=1.
  cargo test -p app --test product_e2e realistic_corpus_full_eval \
    --features product-e2e \
    -- --ignored --test-threads=1 --nocapture
  ec=$?
  echo "end $(date -Iseconds) exit=${ec}"
  exit "${ec}"
} 2>&1 | tee "${LOG}"

# tee preserves pipeline status poorly without pipefail on the group; re-check tail.
if grep -qE 'test result: FAILED|error: test failed' "${LOG}"; then
  exit 1
fi
if ! grep -qE 'test result: ok\.' "${LOG}"; then
  echo "warn: no clear cargo ok line in ${LOG}" >&2
fi

echo "log: ${LOG}"
echo "tip: v2 labels in log lines 'v2: label=…'; PASS count is report-only"
