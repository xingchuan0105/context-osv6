#!/usr/bin/env bash
# Health + revision snapshot for local vs VPS.
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
ENV_FILE="$ROOT/avrag-rs/.env"

if [[ -f "$ENV_FILE" ]]; then
  set -a
  # shellcheck disable=SC1090
  source "$ENV_FILE"
  set +a
fi

: "${VPS_MAIN_HOST:?set VPS_MAIN_HOST}"
: "${VPS_MAIN_USER:?set VPS_MAIN_USER}"
: "${VPS_MAIN_PASSWORD:?set VPS_MAIN_PASSWORD}"

SSH=(sshpass -p "$VPS_MAIN_PASSWORD" ssh -o StrictHostKeyChecking=no "${VPS_MAIN_USER}@${VPS_MAIN_HOST}")

echo "=== LOCAL ==="
cd "$ROOT"
echo "branch: $(git rev-parse --abbrev-ref HEAD 2>/dev/null || echo n/a)"
echo "rev:    $(git rev-parse --short HEAD 2>/dev/null || echo n/a)"
if git diff --quiet 2>/dev/null && git diff --cached --quiet 2>/dev/null; then
  echo "tree:   clean"
else
  echo "tree:   dirty"
  git status -sb | head -25
fi
if [[ -f dist/desktop-release/latest.json ]]; then
  echo "desktop latest (local):"
  cat dist/desktop-release/latest.json | head -c 400
  echo
fi

echo
echo "=== VPS ${VPS_MAIN_HOST} ==="
"${SSH[@]}" bash -s <<'REMOTE'
set +e
echo "--- services ---"
for s in avrag-frontend nginx why-frontend why-api; do
  printf "%-18s %s\n" "$s" "$(systemctl is-active $s 2>/dev/null || echo n/a)"
done
echo "--- docker (key) ---"
docker ps --format '{{.Names}} {{.Status}}' 2>/dev/null | grep -E 'avrag-|ghost' || true
echo "--- DEPLOYED.txt (tail) ---"
tail -n 20 /opt/avrag-rs/DEPLOYED.txt 2>/dev/null || echo "(none)"
echo "--- frontend meta ---"
cat /opt/avrag-rs/frontend/DEPLOY_META.json 2>/dev/null || echo "(no DEPLOY_META.json)"
echo "--- backend meta ---"
cat /opt/avrag-rs/DEPLOY_META.backend.json 2>/dev/null || echo "(no DEPLOY_META.backend.json)"
echo "--- api/worker containers ---"
docker ps --filter name=avrag-api --filter name=avrag-worker --format 'table {{.Names}}\t{{.Status}}\t{{.Image}}' 2>/dev/null || true
echo
echo "--- health ---"
curl -sS -m 5 -o /dev/null -w "api_health:%{http_code}\n" http://127.0.0.1:8081/health
curl -sS -m 5 -o /dev/null -w "fe_local:%{http_code}\n" http://127.0.0.1:3001/
curl -sS -m 8 -o /dev/null -w "pub_health:%{http_code}\n" https://app.contextlm.top/health
curl -sS -m 8 -o /dev/null -w "pub_desktop:%{http_code}\n" https://app.contextlm.top/desktop
curl -sS -m 8 -o /dev/null -w "pub_latest:%{http_code}\n" https://app.contextlm.top/releases/desktop/latest.json
curl -sS -m 8 -o /dev/null -w "pub_why:%{http_code}\n" https://whyimright.contextlm.top/
curl -sS -m 8 -o /dev/null -w "pub_landing:%{http_code}\n" https://contextlm.top/
curl -sS -m 8 -o /dev/null -w "pub_canju:%{http_code}\n" https://canju.contextlm.top/
echo "--- desktop latest (remote) ---"
curl -sS -m 8 https://app.contextlm.top/releases/desktop/latest.json 2>/dev/null | head -c 400
echo
echo "--- isolation invariants (booleans only, no secrets) ---"
docker exec avrag-api bash -lc 'env | grep -E "^(NODE_ENV|E2E_ENABLED|TRUST_PROXY_AUTH)=" | sort' 2>/dev/null || echo "(env probe unavailable)"
grep -qE '^E2E_RESET_SECRET=' /etc/avrag-rs/avrag.env && echo "prod_env_e2e_reset_secret=FAIL(present)" || echo "prod_env_e2e_reset_secret=OK(absent)"
grep -qE '^(E2E_ENABLED=true|TRUST_PROXY_AUTH=true)' /etc/avrag-rs/avrag.env && echo "prod_env_levers=FAIL" || echo "prod_env_levers=OK"
# The migration-role guard exemption must never leak into the runtime env file.
grep -qE '^AVRAG_MIGRATION_ROLE_ONLY=' /etc/avrag-rs/avrag.env && echo "prod_env_migration_role_only=FAIL(present)" || echo "prod_env_migration_role_only=OK(absent)"
[[ -f /etc/avrag-rs/migrate.env ]] && stat -c 'migrate_env_mode=%a' /etc/avrag-rs/migrate.env || echo "migrate_env=missing"
PGURL=$(grep -oP '^(MIGRATION_DATABASE_URL|DATABASE_URL)=\K.*' /etc/avrag-rs/migrate.env 2>/dev/null | head -1)
if [[ -n "$PGURL" ]]; then
  PSQL="docker exec avrag-postgres psql -U avrag -d avrag_rs -tA -c"
  $PGURL "select 'runtime_role_super=' || coalesce((select rolsuper::text from pg_roles where rolname='avrag_runtime'),'absent')" 2>/dev/null || echo "runtime_role=unknown"
  $PGURL "select 'runtime_role_bypassrls=' || coalesce((select rolbypassrls from pg_roles where rolname='avrag_runtime')::text,'absent')" 2>/dev/null || true
  $PGURL "select 'tenant_tables_without_forced_rls=' || coalesce((select count(*)::text from pg_class c join pg_namespace n on n.oid=c.relnamespace where n.nspname='public' and c.relkind='r' and exists (select 1 from pg_attribute a where a.attrelid=c.oid and a.attname='owner_user_id' and not a.attisdropped) and (not c.relrowsecurity or not c.relforcerowsecurity))::text,'unknown')" 2>/dev/null || true
  $PGURL "select 'db_roles=' || string_agg(rolname, ',') from pg_roles where rolname like 'avrag%'" 2>/dev/null || true
else
  echo "migrate_env_dsn=not-read (skip DB probes)"
fi
REMOTE

echo
echo "deploy-status: done"
