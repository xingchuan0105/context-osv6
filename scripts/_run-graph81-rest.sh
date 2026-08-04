#!/usr/bin/env bash
# Sequential graph81 arms: B0 → B2 → B3 (B4 already done)
set -euo pipefail
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
LOG="$ROOT/output/runtime-logs/graph81_chain_rest_$(date -u +%Y%m%d-%H%M%S).log"
exec > >(tee -a "$LOG") 2>&1
echo "[chain] rest log=$LOG start=$(date -Is)"
for arm in B0 B2 B3; do
  echo "[chain] starting $arm $(date -Is)"
  bash "$ROOT/scripts/run-graph81-baseline.sh" "$arm"
  rc=$?
  echo "[chain] $arm exit=$rc $(date -Is)"
  latest=$(ls -t "$ROOT"/output/runtime-logs/graph81_${arm}_*.log 2>/dev/null | head -1 || true)
  if [[ -n "${latest:-}" ]]; then
    flog=$(grep '\[full149\] log:' "$latest" | head -1 | sed 's/.*log: //' || true)
    if [[ -n "${flog:-}" && -f "$flog" ]]; then
      grep 'labels:.*PASS=' "$flog" | tail -1 || true
      grep -oE 'v2: label=[A-Z_]+' "$flog" | sort | uniq -c | sort -rn || true
    fi
  fi
  if [[ $rc -ne 0 ]]; then
    echo "[chain] STOP on $arm"
    exit "$rc"
  fi
done
echo "[chain] ALL DONE $(date -Is)"
