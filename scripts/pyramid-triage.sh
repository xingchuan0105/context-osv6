#!/usr/bin/env bash
# Map failure text → [PYRAMID] next= commands (fast localization).
#
# Usage:
#   bash scripts/pyramid-triage.sh "missing field user_id"
#   bash scripts/test-l1.sh 2>&1 | bash scripts/pyramid-triage.sh
#   cat /tmp/fail.log | bash scripts/pyramid-triage.sh
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

if [[ $# -gt 0 ]]; then
  INPUT="$*"
else
  INPUT="$(cat)"
fi

if [[ -z "${INPUT// }" ]]; then
  echo "usage: bash scripts/pyramid-triage.sh <failure text or stdin>" >&2
  exit 2
fi

echo "======== pyramid triage ========"
echo "[PYRAMID] input_chars=${#INPUT}"

# Order: more specific first
emit() {
  local layer="$1" signal="$2" next="$3" why="$4"
  echo ""
  echo "[PYRAMID] match=${why}"
  echo "[PYRAMID] FAIL layer=${layer} signal=${signal}"
  echo "[PYRAMID] next= ${next}"
}

HIT=0

if echo "$INPUT" | grep -qiE 'patho_|L2-patho|zero indexed chunks|refusing completed|document_locked|cl100k|micro.block|chunk_plan'; then
  emit L2-patho S4 \
    "bash scripts/test-l2-patho.sh" \
    "ingestion patho / terminal / lock / scale"
  emit L2-patho S4 \
    "bash scripts/ingest-doc-dump.sh <document_uuid>" \
    "document state dump"
  HIT=1
fi

if echo "$INPUT" | grep -qiE 'workspace_scope_mismatch|workspace_key_cannot|account_key_cannot|cross_tenant|cross.owner|permission_denied|unauthorized|missing field .user_id'; then
  emit L1 S1 \
    "cargo test -p transport-http --lib patho_authz -- --test-threads=2" \
    "authz / AuthContext wire"
  emit L1 S1 \
    "cargo test -p agent-loop --lib -- --test-threads=1" \
    "agent-loop auth fixtures"
  HIT=1
fi

if echo "$INPUT" | grep -qiE 'budget_exhausted|max_iterations|ReAct|tool_catalog|dispatch_tool'; then
  emit L1 S1 \
    "cargo test -p agent-loop --lib patho_chat -- --test-threads=1" \
    "chat loop budget"
  emit L2-core S2 \
    "bash avrag-rs/scripts/run-product-smoke-e2e.sh  # smoke::chat_smoke" \
    "chat smoke mock"
  HIT=1
fi

if echo "$INPUT" | grep -qiE 'empty_write_topic|write_refine|bands_satisfied|WRITE_MODE|write_smoke'; then
  emit L2-patho S4 \
    "cargo test -p write-core --lib patho_ -- --test-threads=2" \
    "write terminal / topic"
  emit L2-core S2 \
    "bash avrag-rs/scripts/run-product-smoke-e2e.sh  # smoke::write_smoke" \
    "write smoke mock"
  HIT=1
fi

if echo "$INPUT" | grep -qiE 'SSE|event order|first SSE|last SSE|ChatEvent::|stream.*done'; then
  emit L1 S4 \
    "cargo test -p transport-http --lib patho_stream -- --test-threads=2" \
    "SSE order contract"
  emit L2-core S2 \
    "cargo test -p transport-http --test chat_stream_contract -- --test-threads=1" \
    "HTTP stream contract"
  HIT=1
fi

if echo "$INPUT" | grep -qiE 'graph_retrieval|dense_search|citation|rag_smoke|Milvus'; then
  emit L2-patho S4 \
    "cargo test -p avrag-rag-core patho_ -- --test-threads=1" \
    "RAG cross-owner / graph"
  emit L2-core S2 \
    "bash avrag-rs/scripts/run-product-smoke-e2e.sh  # smoke::rag_smoke" \
    "rag smoke mock"
  HIT=1
fi

if echo "$INPUT" | grep -qiE 'playwright|e2e/specs|locator\.|Timeout.*waiting for'; then
  emit L3-thin-journey S3 \
    "cd frontend_next && pnpm exec playwright test e2e/specs/smoke" \
    "UI smoke"
  HIT=1
fi

if echo "$INPUT" | grep -qiE 'llm_real|AGENT_LLM|DEEPSEEK|rate.?limit|model_error|401.*api'; then
  emit L3-thin-llm S5 \
    "E2E_MODE=nightly cargo test -p app --test product_e2e --features product-e2e llm_real -- --ignored --test-threads=1" \
    "real LLM sample / keys"
  HIT=1
fi

if echo "$INPUT" | grep -qiE 'error\[E|could not compile|tsc|Type error'; then
  emit L1 S0 \
    "bash scripts/test-l1.sh" \
    "compile / types"
  HIT=1
fi

if [[ "$HIT" -eq 0 ]]; then
  echo ""
  echo "[PYRAMID] no specific match — default ladder"
  echo "[PYRAMID] next= bash scripts/test-l1.sh"
  echo "[PYRAMID] next= bash scripts/test-l2-patho.sh"
  echo "[PYRAMID] next= SKIP_L3=1 bash scripts/test-dr2.sh"
  echo "[PYRAMID] triage= see docs/engineering/ACCEPTANCE_PYRAMID_STABILIZATION_PLAN_2026-07-10.md §4"
fi

echo ""
echo "======== triage done ========"
