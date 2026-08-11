#!/usr/bin/env bash
# 全量 149 真实语料评测（realistic_corpus_full_eval）——看门狗 + 熔断的标准跑法。
#
# 默认行为:
#   - 复用已灌语料（不设 E2E_FORCE_INGEST；要重灌就显式 E2E_FORCE_INGEST=1）
#   - E2E_CONCURRENCY=8；E2E_ABORT_AFTER_CONSECUTIVE_FAILS=8（连续非 PASS 熔断，0 禁用）
#   - 沉默看门狗 900s：真实 LLM 单题可能数分钟无输出——这是挂死探测器，不是慢速警察
#   - 总时长帽 4h（历史全量 ~133min 的 ~2 倍）
#   - 日志 output/runtime-logs/full149_<UTC>.log（[WATCHDOG] 行 + 每题进度行即心跳）
#
# 预算 / 观测（Lead+Workers 多 agent）:
#   - 默认 **关闭** 产品轮次墙：E2E_UNLIMITED_BUDGET=1（max_iterations=255；
#     worker SaC step_cap=32；rag/search YAML 本就无 token 墙）——测「自然」用量基线。
#   - 恢复产品预算：E2E_UNLIMITED_BUDGET=0 bash scripts/test-full149.sh
#   - 或固定顶：E2E_MAX_ITERATIONS=12（与 UNLIMITED 互斥时 UNLIMITED 优先）
#   - 观测：mode_debug.general.lead_workers / loop_rounds.action_types / budget_used
#     写入 qNNN.json + rag_eval_v2 工件；每题日志行 observe: lead_workers=…
#
# 能力开关（黄金集每题显式 capabilities[]）:
#   - rag-only / search(web)-only / rag+search(dual) / []=纯 chat — 启动时打印分布
#
# 用法:
#   bash scripts/test-full149.sh                            # 全量（默认无限预算基线）
#   E2E_QUESTIONS="58,88" bash scripts/test-full149.sh      # 定向复跑（先定向，别拿全量调试）
#   E2E_FORCE_INGEST=1 bash scripts/test-full149.sh         # 重灌语料后全量
#   E2E_UNLIMITED_BUDGET=0 bash scripts/test-full149.sh     # 产品 YAML 预算回归
#
# 退出码: 0=跑完无失败；1/101=测试失败（含熔断提前中止）；124=看门狗沉默或总时长帽。
set -uo pipefail
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT/avrag-rs"

LOG_DIR="$ROOT/output/runtime-logs"
mkdir -p "$LOG_DIR"
LOG="$LOG_DIR/full149_$(date -u +%Y%m%d-%H%M%S).log"

export E2E_MODE=nightly
export E2E_CONCURRENCY="${E2E_CONCURRENCY:-8}"
export E2E_ABORT_AFTER_CONSECUTIVE_FAILS="${E2E_ABORT_AFTER_CONSECUTIVE_FAILS:-8}"
export CARGO_BUILD_JOBS="${CARGO_BUILD_JOBS:-2}"
# Budget baseline default: lift product round ceiling (see header).
# Explicit 0/false keeps product YAML (rag=5 / search=2).
if [[ -z "${E2E_UNLIMITED_BUDGET+x}" ]]; then
  export E2E_UNLIMITED_BUDGET=1
fi

SILENT_CAP="${E2E_FULL149_SILENT_CAP_SECS:-900}"
TOTAL_CAP="${E2E_FULL149_TOTAL_CAP_SECS:-14400}"

echo "[full149] log: $LOG"
echo "[full149] concurrency=$E2E_CONCURRENCY breaker=$E2E_ABORT_AFTER_CONSECUTIVE_FAILS silent_cap=${SILENT_CAP}s total_cap=${TOTAL_CAP}s"
echo "[full149] budget: E2E_UNLIMITED_BUDGET=${E2E_UNLIMITED_BUDGET:-unset} E2E_MAX_ITERATIONS=${E2E_MAX_ITERATIONS:-unset}"

rc=0
timeout "$TOTAL_CAP" "$ROOT/scripts/with-watchdog.sh" "$LOG" "$SILENT_CAP" -- \
    cargo test -p app --test product_e2e realistic_corpus_full_eval \
    --features product-e2e -- --ignored --test-threads=1 --nocapture || rc=$?
echo "[full149] exit=$rc log=$LOG"
exit "$rc"
