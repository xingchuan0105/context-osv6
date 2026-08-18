#!/usr/bin/env bash
# 面向已部署环境（默认 VPS https://app.contextlm.top）的 Playwright E2E 标准跑法。
#
# 前提（目标机侧，一次性解锁）:
#   /etc/avrag-rs/avrag.env 追加两行:
#     E2E_ENABLED=true
#     E2E_RESET_SECRET=<与本地 avrag-rs/.env 相同的值>
#   然后 docker restart avrag-api
#   （reset 端点自带 secret 校验 + 邮箱白名单 e2e-*/@test.local/@example.com 双门）
#
# 默认行为:
#   - SKIP_BACKEND=1 SKIP_FRONTEND=1：本地不起 Rust/Next，全部流量打目标域名
#   - PLAYWRIGHT_BASE_URL / E2E_API_HEALTH_URL 指向目标
#   - E2E_RESET_SECRET 等由 playwright.config.ts 自动从 avrag-rs/.env 加载
#   - 默认 project=functional（smoke 级，~分钟）；journey/skills/billing 用 E2E_PROJECTS 传
#   - 日志 output/runtime-logs/vps_e2e_<UTC>.log
#
# 用法:
#   bash scripts/test-vps-e2e.sh                            # functional smoke
#   E2E_PROJECTS="journey" bash scripts/test-vps-e2e.sh     # 上传→RAG→引用全链路（真 LLM）
#   E2E_PROJECTS="functional journey skills" bash scripts/test-vps-e2e.sh
#   E2E_TARGET_URL="https://<其他环境>" bash scripts/test-vps-e2e.sh
#
# 注意:
#   - journey/skills 走目标机真实 embedding/LLM key，有 token 成本
#   - billing 需目标后端 PRICING_REVAMP_ROLLOUT=100，且会改测试账号计费状态
#   - 测试数据落在目标库，隔离在 e2e 测试账号内
#
# 退出码: 0=跑完无失败；1=测试失败或 preflight 失败。
set -uo pipefail
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
TARGET="${E2E_TARGET_URL:-https://app.contextlm.top}"
PROJECTS="${E2E_PROJECTS:-functional}"

LOG_DIR="$ROOT/output/runtime-logs"
mkdir -p "$LOG_DIR"
LOG="$LOG_DIR/vps_e2e_$(date -u +%Y%m%d-%H%M%S).log"

echo "[vps-e2e] target:   $TARGET"
echo "[vps-e2e] projects: $PROJECTS"
echo "[vps-e2e] log:      $LOG"

# --- preflight（只读探活；e2e gate 探测等价于 globalSetup 首步，无额外副作用）---
code=$(curl -s -o /dev/null -w "%{http_code}" -m 10 "$TARGET/health")
if [[ "$code" != "200" ]]; then
  echo "[vps-e2e] FAIL: $TARGET/health -> $code（部署不健康，先 bash scripts/deploy-status.sh）" | tee -a "$LOG"
  exit 1
fi
code=$(curl -s -o /dev/null -w "%{http_code}" -m 10 "$TARGET/api/v1/workspaces")
if [[ "$code" != "401" ]]; then
  echo "[vps-e2e] FAIL: $TARGET/api/v1/workspaces -> $code（预期未登录 401；/api 代理链路异常）" | tee -a "$LOG"
  exit 1
fi

ENV_FILE="$ROOT/avrag-rs/.env"
if [[ -f "$ENV_FILE" ]]; then
  set -a
  # shellcheck disable=SC1090
  source "$ENV_FILE"
  set +a
fi
if [[ -z "${E2E_RESET_SECRET:-}" ]]; then
  echo "[vps-e2e] FAIL: 本地 avrag-rs/.env 缺 E2E_RESET_SECRET" | tee -a "$LOG"
  exit 1
fi
# 与 globalSetup 首步完全等价的调用（同一测试账号、同一端点）——gate 关/secret 不一致在此快速失败。
probe_code=$(curl -s -o /dev/null -w "%{http_code}" -m 15 -X POST "$TARGET/api/e2e/reset-user-data" \
  -H "Content-Type: application/json" \
  -H "X-E2E-Secret: $E2E_RESET_SECRET" \
  -d "{\"email\":\"${E2E_TEST_USER_EMAIL:-e2e-test@example.com}\"}")
if [[ "$probe_code" != "200" ]]; then
  echo "[vps-e2e] FAIL: e2e gate 探测 -> $probe_code。解锁：目标机 /etc/avrag-rs/avrag.env 加 E2E_ENABLED=true + E2E_RESET_SECRET=<同本地 .env>，docker restart avrag-api" | tee -a "$LOG"
  exit 1
fi
echo "[vps-e2e] preflight: health OK / /api 代理 OK / e2e gate 开"

# --- run ---
cd "$ROOT/frontend_next"
export SKIP_BACKEND=1
export SKIP_FRONTEND=1
export PLAYWRIGHT_BASE_URL="$TARGET"
export E2E_API_HEALTH_URL="$TARGET/health"

proj_args=()
for p in $PROJECTS; do
  proj_args+=("--project=$p")
done

pnpm exec playwright test "${proj_args[@]}" 2>&1 | tee "$LOG"
status=${PIPESTATUS[0]}
if [[ "$status" == "0" ]]; then
  echo "[vps-e2e] PASS (projects: $PROJECTS)"
else
  echo "[vps-e2e] FAIL exit=$status — 日志: $LOG"
fi
exit "$status"
