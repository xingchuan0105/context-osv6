#!/usr/bin/env bash
# DR2 准部署 ladder: L1 → L2-core → L2-patho → L3-thin (optional).
#
# Exit codes:
#   0  — DR2 full (L3-thin ran and passed) OR DR2_PARTIAL (L1+L2 OK, L3 skipped/optional fail)
#   1  — hard fail at L1 / L2-core / L2-patho, or L3 failed with REQUIRE_L3=1
#
# Env:
#   REQUIRE_L3=1     L3-thin must pass (journey + llm, subject to availability)
#   SKIP_L3=1        never run L3
#   SKIP_L2_CORE=1   skip product smoke (patho-only after L1; not full DR1)
#   DR2_REPORT=path  write markdown report (default: docs/engineering/_reports/dr2-latest.md)
#
# Plan: docs/engineering/ACCEPTANCE_PYRAMID_STABILIZATION_PLAN_2026-07-10.md
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
# shellcheck source=pyramid-lib.sh
source "${ROOT}/scripts/pyramid-lib.sh"
cd "$ROOT"

REQUIRE_L3="${REQUIRE_L3:-0}"
SKIP_L3="${SKIP_L3:-0}"
SKIP_L2_CORE="${SKIP_L2_CORE:-0}"
REPORT="${DR2_REPORT:-${ROOT}/docs/engineering/_reports/dr2-latest.md}"

L1_STATUS=pending
L2_CORE_STATUS=pending
L2_PATHO_STATUS=pending
L3_JOURNEY_STATUS=skipped
L3_LLM_STATUS=skipped
DR_TIER=DR0
STARTED_AT="$(date -Iseconds 2>/dev/null || date)"

run_step() {
  local layer="$1"
  local signal="$2"
  local next="$3"
  shift 3
  echo ""
  echo "[PYRAMID] layer=${layer} begin"
  # Nested scripts suppress their own pyramid_ok (PYRAMID_NESTED=1).
  if PYRAMID_NESTED=1 "$@"; then
    pyramid_ok "$layer"
    return 0
  fi
  pyramid_fail "$layer" "$signal" "$next" "command: $*"
  return 1
}

write_report() {
  mkdir -p "$(dirname "$REPORT")"
  local overall="$1"
  local skip_warn=""
  if [[ "${SKIP_L2_CORE}" == "1" ]]; then
    skip_warn="
## Warning

**L2-core was SKIPPED** (\`SKIP_L2_CORE=1\`). Tier is **not** full DR1/DR2.
Operators must not treat patho-only green as product-smoke green.
"
  fi
  cat >"$REPORT" <<EOF
# DR2 status report

| Field | Value |
|-------|-------|
| generated | ${STARTED_AT} |
| overall | **${overall}** |
| tier | ${DR_TIER} |
| REQUIRE_L3 | ${REQUIRE_L3} |
| SKIP_L3 | ${SKIP_L3} |
| SKIP_L2_CORE | ${SKIP_L2_CORE} |
${skip_warn}
## Layers

| Layer | Status |
|-------|--------|
| L1 (DR0) | ${L1_STATUS} |
| L2-core (DR1) | ${L2_CORE_STATUS} |
| L2-patho (DR2) | ${L2_PATHO_STATUS} |
| L3-thin journey | ${L3_JOURNEY_STATUS} |
| L3-thin llm | ${L3_LLM_STATUS} |

## Next (if red)

See \`[PYRAMID] next=\` lines in the console for the failing layer.
Triage: docs/engineering/ACCEPTANCE_PYRAMID_STABILIZATION_PLAN_2026-07-10.md §4

## Re-run

\`\`\`bash
bash scripts/test-dr2.sh
REQUIRE_L3=1 bash scripts/test-dr2.sh
SKIP_L3=1 bash scripts/test-dr2.sh
\`\`\`
EOF
  echo "[PYRAMID] report=${REPORT}"
}

echo "======== DR2 准部署 ladder ========"
echo "[PYRAMID] started=${STARTED_AT}"
if [[ "$SKIP_L2_CORE" == "1" ]]; then
  echo "[PYRAMID] WARN SKIP_L2_CORE=1 — max tier this run is DR2-patho-only (not full DR1/DR2)"
  echo "[PYRAMID] WARN L2-core=skipped (product smoke / mechanisms not run)"
fi

# --- L1 ---
if run_step L1 S0/S1 "bash scripts/test-l1.sh" bash scripts/test-l1.sh; then
  L1_STATUS=ok
  DR_TIER=DR0
else
  L1_STATUS=FAIL
  write_report FAIL
  exit 1
fi

# --- L2-core ---
if [[ "$SKIP_L2_CORE" == "1" ]]; then
  echo "[PYRAMID] layer=L2-core skipped (SKIP_L2_CORE=1)"
  L2_CORE_STATUS=skipped
else
  if run_step L2-core S1/S2 \
    "bash scripts/test-l2-mechanisms.sh  # or cargo test -p agent-loop --lib" \
    bash scripts/test-l2-mechanisms.sh; then
    L2_CORE_STATUS=ok
    DR_TIER=DR1
  else
    L2_CORE_STATUS=FAIL
    write_report FAIL
    exit 1
  fi
fi

# --- L2-patho ---
if run_step L2-patho S4 \
  "bash scripts/test-l2-patho.sh  # or cargo test -p ingestion --lib patho_" \
  bash scripts/test-l2-patho.sh; then
  L2_PATHO_STATUS=ok
  # Never claim full DR2 when L2-core was skipped (patho alone ≠ mechanisms green).
  if [[ "$SKIP_L2_CORE" == "1" ]]; then
    DR_TIER=DR2-patho-only
  else
    DR_TIER=DR2
  fi
else
  L2_PATHO_STATUS=FAIL
  write_report FAIL
  exit 1
fi

# --- L3-thin ---
L3_OK=0
if [[ "$SKIP_L3" == "1" ]]; then
  echo "[PYRAMID] layer=L3-thin skipped (SKIP_L3=1)"
  L3_JOURNEY_STATUS=skipped
  L3_LLM_STATUS=skipped
else
  # Peek keys only in parent (no export flood). L3-llm loads keys itself if needed.
  if [[ -x scripts/test-l3-journey.sh ]] && pyramid_has_playwright; then
    echo "[PYRAMID] layer=L3-thin journey begin"
    if PYRAMID_NESTED=1 bash scripts/test-l3-journey.sh; then
      L3_JOURNEY_STATUS=ok
      L3_OK=1
      pyramid_ok L3-thin-journey
    else
      L3_JOURNEY_STATUS=FAIL
      echo "[PYRAMID] FAIL layer=L3-thin-journey signal=S3"
      echo "[PYRAMID] next= cd frontend_next && pnpm exec playwright test e2e/specs/smoke"
      if [[ "$REQUIRE_L3" == "1" ]]; then
        write_report FAIL
        exit 1
      fi
      echo "[PYRAMID] DR2_PARTIAL: journey failed (REQUIRE_L3=0)"
    fi
  else
    echo "[PYRAMID] layer=L3-thin journey skipped (no pnpm or script)"
    L3_JOURNEY_STATUS=skipped
  fi

  if [[ -x scripts/test-l3-llm.sh ]] && pyramid_has_llm_keys; then
    echo "[PYRAMID] layer=L3-thin llm begin"
    if PYRAMID_NESTED=1 bash scripts/test-l3-llm.sh; then
      L3_LLM_STATUS=ok
      L3_OK=1
      pyramid_ok L3-thin-llm
    else
      L3_LLM_STATUS=FAIL
      echo "[PYRAMID] FAIL layer=L3-thin-llm signal=S5"
      echo "[PYRAMID] next= E2E_MODE=nightly cargo test -p app --test product_e2e --features product-e2e llm_real -- --ignored --test-threads=1"
      if [[ "$REQUIRE_L3" == "1" ]]; then
        write_report FAIL
        exit 1
      fi
      echo "[PYRAMID] DR2_PARTIAL: llm sample failed (REQUIRE_L3=0)"
    fi
  else
    echo "[PYRAMID] layer=L3-thin llm skipped (no AGENT_LLM_API_KEY/DEEPSEEK_API_KEY/DMX_API_KEY)"
    L3_LLM_STATUS=skipped
  fi
fi

if [[ "$REQUIRE_L3" == "1" && "$L3_OK" -eq 0 ]]; then
  echo "[PYRAMID] DR2 FAIL: REQUIRE_L3=1 but L3-thin did not pass"
  write_report FAIL
  exit 1
fi

if [[ "$SKIP_L2_CORE" == "1" ]]; then
  # Do not claim full mechanism DR2 when product smoke was skipped.
  DR_TIER=DR2-patho-only
  write_report PARTIAL
  echo "[PYRAMID] DR2_PARTIAL (L1+L2-patho OK; L2-core SKIPPED — not full DR1/DR2)"
  echo "[PYRAMID] next= unset SKIP_L2_CORE && bash scripts/test-dr2.sh"
elif [[ "$L3_OK" -eq 1 && "$L3_JOURNEY_STATUS" == "ok" && "$L3_LLM_STATUS" == "ok" ]]; then
  DR_TIER=DR2-full
  write_report OK
  echo "[PYRAMID] DR2 OK (full: L1+L2+patho+L3 journey+llm)"
elif [[ "$L3_OK" -eq 1 ]]; then
  DR_TIER=DR2-partial-l3
  write_report PARTIAL
  echo "[PYRAMID] DR2_PARTIAL (L1+L2-core+patho OK; some L3 ok)"
else
  DR_TIER=DR2-mechanisms
  write_report PARTIAL
  echo "[PYRAMID] DR2_PARTIAL (L1+L2-core+L2-patho OK; L3 skipped or optional fail)"
  echo "[PYRAMID] For full 准部署 with UI+LLM: REQUIRE_L3=1 bash scripts/test-dr2.sh"
fi

echo "======== DR2 complete (${DR_TIER}) ========"
exit 0
