#!/usr/bin/env bash
# Probe local product stack readiness for L3 UI (Playwright REUSE).
#
# Exit codes:
#   0  — login path OK (or account missing but API healthy — register can proceed)
#   1  — hard fail (API down / login 5xx schema skew)
#   2  — usage error
#
# Usage:
#   bash scripts/dev-stack-check.sh
#   API_BASE=http://127.0.0.1:8080 bash scripts/dev-stack-check.sh
#
# Env (optional, defaults match e2e fixture):
#   API_BASE / PLAYWRIGHT_API_BASE  default http://127.0.0.1:8080
#   E2E_TEST_USER_EMAIL / E2E_TEST_USER_PASSWORD
set -euo pipefail
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
# shellcheck source=pyramid-lib.sh
source "${ROOT}/scripts/pyramid-lib.sh"

API_BASE="${API_BASE:-${PLAYWRIGHT_API_BASE:-http://127.0.0.1:8080}}"
EMAIL="${E2E_TEST_USER_EMAIL:-}"
PASSWORD="${E2E_TEST_USER_PASSWORD:-}"

# Peek fixture defaults without sourcing full .env when unset
if [[ -z "$EMAIL" ]]; then
  EMAIL="$(_pyramid_env_get E2E_TEST_USER_EMAIL 2>/dev/null || true)"
fi
if [[ -z "$PASSWORD" ]]; then
  PASSWORD="$(_pyramid_env_get E2E_TEST_USER_PASSWORD 2>/dev/null || true)"
fi
EMAIL="${EMAIL:-e2e-test@example.com}"
PASSWORD="${PASSWORD:-E2eTest123!}"

echo "[PYRAMID] layer=ops cap=CAP-AUTH signal=S2"
echo "[dev-stack-check] api_base=${API_BASE}"
echo "[dev-stack-check] email=${EMAIL}"

# Health (best-effort)
health_code="000"
for path in /health /api/health /api/v1/health; do
  code="$(curl -sS -m 3 -o /dev/null -w '%{http_code}' "${API_BASE}${path}" 2>/dev/null || echo 000)"
  if [[ "$code" =~ ^2 ]]; then
    health_code="$code"
    echo "[dev-stack-check] health ${path} -> ${code}"
    break
  fi
done
if [[ "$health_code" == "000" ]]; then
  # API may still answer /api/auth/login without a dedicated health route
  echo "[dev-stack-check] WARN no /health hit; will probe login"
fi

# Login probe — body not printed (may contain tokens); status only
tmp="$(mktemp)"
trap 'rm -f "$tmp"' EXIT
http_code="$(
  curl -sS -m 15 -o "$tmp" -w '%{http_code}' \
    -X POST "${API_BASE}/api/auth/login" \
    -H 'Content-Type: application/json' \
    -d "{\"email\":\"${EMAIL}\",\"password\":\"${PASSWORD}\"}" \
    2>/dev/null || echo 000
)"

err_snippet="$(
  python3 -c '
import json,sys
p=sys.argv[1]
try:
  d=json.load(open(p))
  print(((d.get("error") or "") + " " + (d.get("message") or "")).strip())
except Exception:
  print(open(p).read()[:200].replace("\n"," "))
' "$tmp" 2>/dev/null || true
)"

echo "[dev-stack-check] login HTTP ${http_code} ${err_snippet}"

case "$http_code" in
  200|201)
    echo "[dev-stack-check] OK login succeeded"
    exit 0
    ;;
  401|403|404)
    # Missing/wrong password — setup-auth can register or fix password; not schema skew.
    echo "[dev-stack-check] OK API auth reachable (login ${http_code}); ensureTestUser may register"
    exit 0
    ;;
  000)
    echo "[dev-stack-check] FAIL cannot reach ${API_BASE}"
    echo "[PYRAMID] next= start avrag-api (product-dev-up) or set API_BASE"
    exit 1
    ;;
  5*)
    if echo "$err_snippet" | grep -qi 'org_id'; then
      echo "[dev-stack-check] FAIL login 5xx mentions org_id — API binary older than DB migrations"
    else
      echo "[dev-stack-check] FAIL login ${http_code} (schema/binary skew or DB error)"
    fi
    echo "[PYRAMID] next= cd avrag-rs && cargo build -p avrag-api -p avrag-worker && restart processes"
    echo "[PYRAMID] detail= do not run Playwright REUSE against stale target/debug/avrag-api"
    echo "[PYRAMID] detail= if API fails to start: Milvus collection missing owner_user_id → drop avrag_* collections"
    exit 1
    ;;
  *)
    echo "[dev-stack-check] FAIL unexpected login status ${http_code}"
    echo "[PYRAMID] next= inspect ${API_BASE}/api/auth/login and avrag-rs/.dev-logs/api.log"
    exit 1
    ;;
esac
