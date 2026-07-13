#!/usr/bin/env bash
# L3-thin-llm — real LLM four-mode sample (chat / rag / search / write), one each.
#
# Single cargo process so standard-doc cold ingest is shared (fixtures/standard_doc.rs).
# Does NOT run rag_quality_prod / smoke_v5 / staging PDF (use test-l3-quality.sh).
#
# Env:
#   L3_LLM_EXT=1     also multi_turn + format_real (reuse same standard doc)
#   L3_LLM_FULL=1    entire llm_real module (legacy / nightly dump)
#   E2E_MODE         default nightly
set -euo pipefail
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
# shellcheck source=pyramid-lib.sh
source "${ROOT}/scripts/pyramid-lib.sh"
cd "$ROOT/avrag-rs"

echo "[PYRAMID] layer=L3-thin-llm begin"
export E2E_MODE="${E2E_MODE:-nightly}"
pyramid_export_llm_keys_if_needed

# cargo test filter is substring match; use --skip to carve thin/full while
# keeping one process (OnceCell standard-doc corpus).
COMMON=(
  cargo test -p app --test product_e2e --features product-e2e llm_real
  -- --ignored --test-threads=1 --nocapture
)

if [[ "${L3_LLM_FULL:-0}" == "1" ]]; then
  echo "==> L3 llm_real FULL module E2E_MODE=$E2E_MODE (quality included; stop external workers)"
  "${COMMON[@]}" \
    || pyramid_fail L3-thin-llm S5 \
      "E2E_MODE=nightly L3_LLM_FULL=1 bash scripts/test-l3-llm.sh" \
      "check keys / preflight worker"
elif [[ "${L3_LLM_EXT:-0}" == "1" ]]; then
  echo "==> L3 llm_real EXT (thin + multi_turn + format) E2E_MODE=$E2E_MODE"
  "${COMMON[@]}" \
    --skip rag_quality_prod \
    --skip pdf_corpus \
    --skip pdf_rag_e2e \
    --skip cost_report \
    --skip real_llm_rag_complex_query \
    --skip real_llm_rag_staging \
    || pyramid_fail L3-thin-llm S5 \
      "E2E_MODE=nightly L3_LLM_EXT=1 bash scripts/test-l3-llm.sh" \
      "check AGENT_LLM_API_KEY"
else
  echo "==> L3 llm_real THIN four-mode E2E_MODE=$E2E_MODE (shared antifragile.txt ingest)"
  "${COMMON[@]}" \
    --skip rag_quality_prod \
    --skip pdf_corpus \
    --skip pdf_rag_e2e \
    --skip cost_report \
    --skip real_llm_rag_complex_query \
    --skip real_llm_rag_staging \
    --skip multi_turn \
    --skip format_real \
    || pyramid_fail L3-thin-llm S5 \
      "E2E_MODE=nightly bash scripts/test-l3-llm.sh" \
      "check AGENT_LLM_API_KEY / network"
fi

pyramid_ok L3-thin-llm
echo "L3 llm sample OK"
