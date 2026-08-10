#!/usr/bin/env bash
# Re-run full-149 non-PASS questions:
#   Agent  = OpenCode Go (deepseek-v4-flash)
#   Judge  = Ollama Cloud (deepseek-v4-flash:0731-cloud)
# Merges into BASELINE_MERGED when provided (per_question_merged or phase1_run_dir).
#
# Usage:
#   BASELINE_MERGED=.../baseline_merged.json \
#   E2E_QUESTIONS=23,46,... E2E_CONCURRENCY=2 \
#     bash scripts/run-full149-nonpass-opencode-agent-ollama-judge.sh
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
RUN_TAG="full149_nonpass_opencode_ollama_${STAMP}"
LOG="$LOG_DIR/${RUN_TAG}.log"
MERGE_JSON="$LOG_DIR/${RUN_TAG}_merged.json"
STATE_DIR="$LOG_DIR/${RUN_TAG}_state"
mkdir -p "$STATE_DIR"

export AGENT_LLM_BASE_URL="${OPENCODE_GO_BASE_URL:?OPENCODE_GO_BASE_URL required}"
export AGENT_LLM_API_KEY="${OPENCODE_GO_API_KEY:?OPENCODE_GO_API_KEY required}"
export AGENT_LLM_MODEL="${AGENT_LLM_MODEL_OVERRIDE:-deepseek-v4-flash}"
export AGENT_LLM_API_STYLE="${AGENT_LLM_API_STYLE:-openai}"
export AGENT_LLM_ENABLE_THINKING=false

export E2E_LLM_BASE_URL="$AGENT_LLM_BASE_URL"
export E2E_LLM_API_KEY="$AGENT_LLM_API_KEY"
export E2E_LLM_MODEL="$AGENT_LLM_MODEL"
export CHAT_LLM_BASE_URL="$AGENT_LLM_BASE_URL"
export CHAT_LLM_API_KEY="$AGENT_LLM_API_KEY"
export CHAT_LLM_MODEL="$AGENT_LLM_MODEL"

OLLAMA_V1="${OLLAMA_HOST:-https://ollama.com}"
OLLAMA_V1="${OLLAMA_V1%/}"
case "$OLLAMA_V1" in
  */v1) ;;
  *) OLLAMA_V1="${OLLAMA_V1}/v1" ;;
esac
export JUDGE_LLM_BASE_URL="$OLLAMA_V1"
export JUDGE_LLM_API_KEY="${OLLAMA_API_KEY:?OLLAMA_API_KEY required}"
export JUDGE_LLM_MODEL="${JUDGE_LLM_MODEL_OVERRIDE:-${OLLAMA_MODEL:-deepseek-v4-flash:0731-cloud}}"
export JUDGE_LLM_TIMEOUT_MS="${JUDGE_LLM_TIMEOUT_MS:-120000}"

export E2E_MODE=nightly
export E2E_CONCURRENCY="${E2E_CONCURRENCY:-2}"
export E2E_ABORT_AFTER_CONSECUTIVE_FAILS="${E2E_ABORT_AFTER_CONSECUTIVE_FAILS:-0}"
export CARGO_BUILD_JOBS="${CARGO_BUILD_JOBS:-2}"
export RAG_EVAL_V2="${RAG_EVAL_V2:-1}"
unset E2E_FORCE_INGEST || true

if [ -z "${E2E_QUESTIONS:-}" ]; then
  echo "[run] ERROR: E2E_QUESTIONS required (comma-separated 1-based ids)" | tee -a "$LOG"
  exit 2
fi

BASELINE_MERGED="${BASELINE_MERGED:-}"
SILENT_CAP="${E2E_FULL149_SILENT_CAP_SECS:-900}"
TOTAL_CAP="${E2E_FULL149_TOTAL_CAP_SECS:-7200}"

{
  echo "[run] stamp=$STAMP tag=$RUN_TAG"
  echo "[run] agent=$AGENT_LLM_BASE_URL model=$AGENT_LLM_MODEL thinking=$AGENT_LLM_ENABLE_THINKING"
  echo "[run] judge=$JUDGE_LLM_BASE_URL model=$JUDGE_LLM_MODEL"
  echo "[run] concurrency=$E2E_CONCURRENCY questions=$E2E_QUESTIONS"
  echo "[run] baseline=${BASELINE_MERGED:-none}"
  echo "[run] log=$LOG"
} | tee "$LOG"

rc=0
timeout "$TOTAL_CAP" "$ROOT/scripts/with-watchdog.sh" "$LOG" "$SILENT_CAP" -- \
  cargo test -p app --test product_e2e realistic_corpus_full_eval \
  --features product-e2e -- --ignored --test-threads=1 --nocapture || rc=$?
echo "[run] phase_exit=$rc" | tee -a "$LOG"

RUN_DIR=$(ls -1dt "$ROOT/avrag-rs/crates/app/tests/e2e_output/rag_eval_v2"/v2_* 2>/dev/null | head -1 || true)
echo "[run] run_dir=$RUN_DIR" | tee -a "$LOG"
if [ -z "$RUN_DIR" ] || [ ! -f "$RUN_DIR/summary.json" ]; then
  echo "[run] ERROR: no summary.json" | tee -a "$LOG"
  exit "${rc:-1}"
fi
cp -a "$RUN_DIR/summary.json" "$STATE_DIR/rerun_summary.json"
echo "$RUN_DIR" > "$STATE_DIR/rerun_run_dir.txt"
echo "$E2E_QUESTIONS" > "$STATE_DIR/questions.csv"

python3 - <<'PY' "$STATE_DIR" "$MERGE_JSON" "$E2E_QUESTIONS" "${BASELINE_MERGED:-}"
import json, sys
from pathlib import Path
from collections import Counter

state = Path(sys.argv[1])
out = Path(sys.argv[2])
qcsv = sys.argv[3]
baseline_path = sys.argv[4] if len(sys.argv) > 4 else ""
rerun_ids = {int(x) for x in qcsv.split(",") if x.strip()}

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

def load_labels_tsv(run_dir: Path) -> dict[int, str]:
    tsv = run_dir / "per_query.tsv"
    if not tsv.is_file():
        return {}
    lines = tsv.read_text().splitlines()
    if not lines:
        return {}
    hdr = lines[0].split("\t")
    try:
        i_n, i_lab = hdr.index("n"), hdr.index("label")
    except ValueError:
        return {}
    out = {}
    for line in lines[1:]:
        cols = line.split("\t")
        if len(cols) <= max(i_n, i_lab):
            continue
        try:
            out[int(cols[i_n])] = cols[i_lab].upper()
        except ValueError:
            pass
    return out

rerun_dir = Path((state / "rerun_run_dir.txt").read_text().strip())
rerun = load_labels(rerun_dir) or load_labels_tsv(rerun_dir)
s_rerun = json.loads((state / "rerun_summary.json").read_text())

merged = {}
if baseline_path:
    bp = Path(baseline_path)
    if bp.is_file():
        baseline_meta = json.loads(bp.read_text())
        pq = baseline_meta.get("per_question_merged") or {}
        for k, v in pq.items():
            try:
                merged[int(k)] = str(v).upper()
            except ValueError:
                pass
        if not merged:
            p1 = baseline_meta.get("phase1_run_dir")
            if p1:
                p1p = Path(p1)
                merged = load_labels(p1p) or load_labels_tsv(p1p)

for qid, lab in rerun.items():
    merged[qid] = lab

if not merged:
    merged = dict(rerun)

counts = Counter(merged.values())
pass_n = counts.get("PASS", 0)
total = len(merged)
still = sorted(q for q, lab in merged.items() if lab != "PASS")
improved = sorted(q for q in rerun_ids if merged.get(q) == "PASS")
still_from_rerun = sorted(q for q in rerun_ids if merged.get(q) != "PASS")

result = {
    "agent": "opencode-go",
    "judge": "ollama-cloud",
    "rerun_run_dir": str(rerun_dir),
    "baseline_merged": baseline_path or None,
    "rerun_ids": sorted(rerun_ids),
    "rerun_label_counts": dict(Counter(rerun.values())),
    "rerun_pass": sum(1 for q in rerun_ids if rerun.get(q) == "PASS"),
    "rerun_total": len(rerun_ids),
    "improved_to_pass": improved,
    "still_nonpass_from_rerun": still_from_rerun,
    "merged_total_labeled": total,
    "merged_pass": pass_n,
    "merged_pass_rate": (pass_n / total) if total else None,
    "merged_label_counts": dict(counts),
    "still_nonpass_after_rerun": still,
    "rerun_summary": s_rerun.get("summary") or s_rerun,
    "per_question_merged": {str(k): v for k, v in sorted(merged.items())},
    "per_question_rerun": {str(k): v for k, v in sorted(rerun.items())},
}
out.write_text(json.dumps(result, ensure_ascii=False, indent=2))
print(json.dumps({
    "rerun_pass": result["rerun_pass"],
    "rerun_total": result["rerun_total"],
    "improved_to_pass": improved,
    "still_nonpass_from_rerun": still_from_rerun,
    "merged_pass": pass_n,
    "merged_total": total,
    "merged_pass_rate": result["merged_pass_rate"],
    "merged_label_counts": dict(counts),
    "still_nonpass_count": len(still),
    "out": str(out),
}, ensure_ascii=False, indent=2))
PY

echo "[run] merge → $MERGE_JSON" | tee -a "$LOG"
echo "[run] done exit=$rc" | tee -a "$LOG"
exit 0
