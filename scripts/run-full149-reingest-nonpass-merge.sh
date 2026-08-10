#!/usr/bin/env bash
# Full-149 with force re-ingest, then non-PASS re-run, merge stats.
# Agent: Ollama Cloud deepseek-v4-flash:0731-cloud
# Judge: OpenCode Go
set -uo pipefail
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT/avrag-rs"
set -a
# shellcheck disable=SC1091
source "$ROOT/avrag-rs/.env"
set +a

LOG_DIR="$ROOT/output/runtime-logs"
mkdir -p "$LOG_DIR"
STAMP="$(date -u +%Y%m%d-%H%M%S)"
RUN_TAG="full149_reingest_${STAMP}"
LOG="$LOG_DIR/${RUN_TAG}.log"
MERGE_JSON="$LOG_DIR/${RUN_TAG}_merged.json"
STATE_DIR="$LOG_DIR/${RUN_TAG}_state"
mkdir -p "$STATE_DIR"

# --- Models (explicit; do not print secrets) ---
export AGENT_LLM_BASE_URL="${AGENT_LLM_BASE_URL_OVERRIDE:-https://ollama.com/v1}"
export AGENT_LLM_API_KEY="${OLLAMA_API_KEY:?OLLAMA_API_KEY required}"
export AGENT_LLM_MODEL="${AGENT_LLM_MODEL_OVERRIDE:-deepseek-v4-flash:0731-cloud}"
export AGENT_LLM_API_STYLE="${AGENT_LLM_API_STYLE:-openai}"
export AGENT_LLM_ENABLE_THINKING="${AGENT_LLM_ENABLE_THINKING:-false}"

# Keep e2e helper LLM slots aligned with agent
export E2E_LLM_BASE_URL="$AGENT_LLM_BASE_URL"
export E2E_LLM_API_KEY="$AGENT_LLM_API_KEY"
export E2E_LLM_MODEL="$AGENT_LLM_MODEL"
export CHAT_LLM_BASE_URL="$AGENT_LLM_BASE_URL"
export CHAT_LLM_API_KEY="$AGENT_LLM_API_KEY"
export CHAT_LLM_MODEL="$AGENT_LLM_MODEL"

export JUDGE_LLM_BASE_URL="${OPENCODE_GO_BASE_URL:?OPENCODE_GO_BASE_URL required}"
export JUDGE_LLM_API_KEY="${OPENCODE_GO_API_KEY:?OPENCODE_GO_API_KEY required}"
export JUDGE_LLM_MODEL="${JUDGE_LLM_MODEL_OVERRIDE:-${QWEN_CHAT_MODEL:-deepseek-v4-flash}}"

export E2E_MODE=nightly
export E2E_CONCURRENCY="${E2E_CONCURRENCY:-8}"
export E2E_FORCE_INGEST=1
# Complete the full slate even if consecutive non-PASS (we re-run nonpass after)
export E2E_ABORT_AFTER_CONSECUTIVE_FAILS="${E2E_ABORT_AFTER_CONSECUTIVE_FAILS:-0}"
export CARGO_BUILD_JOBS="${CARGO_BUILD_JOBS:-2}"
export RAG_EVAL_V2="${RAG_EVAL_V2:-1}"

SILENT_CAP="${E2E_FULL149_SILENT_CAP_SECS:-900}"
TOTAL_CAP="${E2E_FULL149_TOTAL_CAP_SECS:-14400}"

{
  echo "[run] stamp=$STAMP"
  echo "[run] agent=$AGENT_LLM_BASE_URL model=$AGENT_LLM_MODEL"
  echo "[run] judge=$JUDGE_LLM_BASE_URL model=$JUDGE_LLM_MODEL"
  echo "[run] concurrency=$E2E_CONCURRENCY force_ingest=1 breaker=$E2E_ABORT_AFTER_CONSECUTIVE_FAILS"
  echo "[run] log=$LOG"
} | tee -a "$LOG"

# --- Phase 1: full 149 with re-ingest ---
rc1=0
timeout "$TOTAL_CAP" "$ROOT/scripts/with-watchdog.sh" "$LOG" "$SILENT_CAP" -- \
  cargo test -p app --test product_e2e realistic_corpus_full_eval \
  --features product-e2e -- --ignored --test-threads=1 --nocapture || rc1=$?
echo "[run] phase1_exit=$rc1" | tee -a "$LOG"

# Locate newest rag_eval_v2 run dir created during this window
RUN_DIR=$(ls -1dt "$ROOT/avrag-rs/crates/app/tests/e2e_output/rag_eval_v2"/v2_* 2>/dev/null | head -1 || true)
echo "[run] phase1_run_dir=$RUN_DIR" | tee -a "$LOG"
if [ -z "$RUN_DIR" ] || [ ! -f "$RUN_DIR/summary.json" ]; then
  echo "[run] ERROR: no summary.json after phase1" | tee -a "$LOG"
  exit "${rc1:-1}"
fi
cp -a "$RUN_DIR/summary.json" "$STATE_DIR/phase1_summary.json"
echo "$RUN_DIR" > "$STATE_DIR/phase1_run_dir.txt"

# Extract non-PASS question numbers from judge artifacts
NONPASS_CSV=$(python3 - <<'PY' "$RUN_DIR"
import json, sys
from pathlib import Path
run = Path(sys.argv[1])
non = []
for p in sorted(run.glob("q*.judge.json")):
    try:
        j = json.loads(p.read_text())
    except Exception:
        continue
    label = None
    if isinstance(j, dict):
        label = j.get("label") or (j.get("score_v2") or {}).get("label") or (j.get("score_v2") or {}).get("final_label")
        if label is None and "summary" in j:
            label = j["summary"].get("label")
    # also try artifact
    if not label:
        art = run / p.name.replace(".judge.json", ".artifact.json")
        if art.exists():
            try:
                a = json.loads(art.read_text())
                label = (a.get("score_v2") or {}).get("label") or (a.get("score_v2") or {}).get("final_label")
            except Exception:
                pass
    if label and str(label).upper() not in ("PASS",):
        qid = p.name.split(".")[0]  # q001
        num = qid.lstrip("q").lstrip("0") or "0"
        try:
            non.append(int(num))
        except ValueError:
            pass
# Also parse summary label map if present
sj = run / "summary.json"
# per-query files are authoritative
non = sorted(set(non))
print(",".join(str(n) for n in non))
print(f"[run] nonpass_count={len(non)} ids={non}", file=sys.stderr)
PY
)
echo "[run] nonpass_csv=$NONPASS_CSV" | tee -a "$LOG"
echo "$NONPASS_CSV" > "$STATE_DIR/nonpass.csv"

if [ -z "$NONPASS_CSV" ]; then
  echo "[run] all PASS on phase1; skip phase2" | tee -a "$LOG"
  python3 - <<'PY' "$STATE_DIR" "$MERGE_JSON" "$RUN_DIR"
import json, sys
from pathlib import Path
state, out, run = Path(sys.argv[1]), Path(sys.argv[2]), Path(sys.argv[3])
s1 = json.loads((state/"phase1_summary.json").read_text())
merged = {
  "phase1_run_dir": str(run),
  "phase2_run_dir": None,
  "nonpass_ids": [],
  "phase1": s1,
  "phase2": None,
  "merged_label_counts": (s1.get("summary") or s1).get("label_counts"),
  "note": "phase2 skipped — all PASS",
}
out.write_text(json.dumps(merged, ensure_ascii=False, indent=2))
print("wrote", out)
PY
  exit 0
fi

# --- Phase 2: re-run non-PASS only (no re-ingest) ---
unset E2E_FORCE_INGEST || true
export E2E_QUESTIONS="$NONPASS_CSV"
LOG2="$LOG_DIR/${RUN_TAG}_nonpass.log"
echo "[run] phase2 E2E_QUESTIONS=$E2E_QUESTIONS log=$LOG2" | tee -a "$LOG"
rc2=0
timeout 7200 "$ROOT/scripts/with-watchdog.sh" "$LOG2" "$SILENT_CAP" -- \
  cargo test -p app --test product_e2e realistic_corpus_full_eval \
  --features product-e2e -- --ignored --test-threads=1 --nocapture || rc2=$?
echo "[run] phase2_exit=$rc2" | tee -a "$LOG"

RUN_DIR2=$(ls -1dt "$ROOT/avrag-rs/crates/app/tests/e2e_output/rag_eval_v2"/v2_* 2>/dev/null | head -1 || true)
echo "[run] phase2_run_dir=$RUN_DIR2" | tee -a "$LOG"
if [ -n "$RUN_DIR2" ] && [ -f "$RUN_DIR2/summary.json" ]; then
  cp -a "$RUN_DIR2/summary.json" "$STATE_DIR/phase2_summary.json"
  echo "$RUN_DIR2" > "$STATE_DIR/phase2_run_dir.txt"
fi

# --- Merge: phase1 labels overridden by phase2 for re-run questions ---
python3 - <<'PY' "$STATE_DIR" "$MERGE_JSON" "$NONPASS_CSV"
import json, sys
from pathlib import Path
from collections import Counter

state = Path(sys.argv[1])
out = Path(sys.argv[2])
nonpass_csv = sys.argv[3]
non_ids = {int(x) for x in nonpass_csv.split(",") if x.strip()}

def load_labels(run_dir: Path) -> dict[int, str]:
    labels = {}
    if not run_dir or not run_dir.is_dir():
        return labels
    for p in run_dir.glob("q*.artifact.json"):
        try:
            a = json.loads(p.read_text())
        except Exception:
            continue
        label = (a.get("score_v2") or {}).get("label") or (a.get("score_v2") or {}).get("final_label")
        if not label:
            jpath = run_dir / p.name.replace(".artifact.json", ".judge.json")
            if jpath.exists():
                try:
                    j = json.loads(jpath.read_text())
                    label = j.get("label") or (j.get("score_v2") or {}).get("label")
                except Exception:
                    pass
        if label:
            q = p.name.split(".")[0]
            num = int(q.lstrip("q") or "0")
            labels[num] = str(label).upper()
    return labels

p1_dir = Path((state / "phase1_run_dir.txt").read_text().strip()) if (state / "phase1_run_dir.txt").exists() else None
p2_dir = Path((state / "phase2_run_dir.txt").read_text().strip()) if (state / "phase2_run_dir.txt").exists() else None
p1 = load_labels(p1_dir) if p1_dir else {}
p2 = load_labels(p2_dir) if p2_dir else {}

merged_labels = dict(p1)
for qid, lab in p2.items():
    if qid in non_ids or qid in p2:
        merged_labels[qid] = lab

counts = Counter(merged_labels.values())
# Prefer phase2 means only for note; full means from phase1 summary if present
s1 = json.loads((state / "phase1_summary.json").read_text()) if (state / "phase1_summary.json").exists() else {}
s2 = json.loads((state / "phase2_summary.json").read_text()) if (state / "phase2_summary.json").exists() else {}

nonpass_after = sorted(q for q, lab in merged_labels.items() if lab != "PASS")
pass_n = counts.get("PASS", 0)
total = len(merged_labels)

result = {
    "phase1_run_dir": str(p1_dir) if p1_dir else None,
    "phase2_run_dir": str(p2_dir) if p2_dir else None,
    "nonpass_ids_phase1": sorted(non_ids),
    "merged_total_labeled": total,
    "merged_pass": pass_n,
    "merged_pass_rate": (pass_n / total) if total else None,
    "merged_label_counts": dict(counts),
    "still_nonpass_after_rerun": nonpass_after,
    "phase1_summary": s1.get("summary") or s1,
    "phase2_summary": s2.get("summary") or s2,
    "per_question_merged": {str(k): v for k, v in sorted(merged_labels.items())},
}
out.write_text(json.dumps(result, ensure_ascii=False, indent=2))
print(json.dumps({
    "merged_pass": pass_n,
    "total": total,
    "label_counts": dict(counts),
    "still_nonpass": nonpass_after,
    "out": str(out),
}, ensure_ascii=False, indent=2))
PY

echo "[run] merge → $MERGE_JSON" | tee -a "$LOG"
echo "[run] done phase1=$rc1 phase2=${rc2:-na}" | tee -a "$LOG"
# Prefer non-zero if either phase failed hard
if [ "${rc1:-0}" -ne 0 ] && [ "${rc1:-0}" -ne 101 ]; then
  exit "$rc1"
fi
exit 0
