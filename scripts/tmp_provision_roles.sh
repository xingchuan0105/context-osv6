#!/usr/bin/env bash
# T2+T3 for the 2026-08-30 window. Runs ON the VPS as root. Generates fresh
# credentials, applies db/roles/002_runtime_grants.sql, writes migrate.env
# (0600), rewrites avrag.env: runtime DSN -> avrag_runtime, levers pinned,
# E2E_RESET_SECRET removed, JWT_SECRET rotated. Passwords are echoed once to
# stdout base64 so the caller can persist them locally; not printed as plain.
set -euo pipefail

APPG=/opt/avrag-rs
SQLDIR=/tmp/roles-002
gen() { openssl rand -base64 24 | tr -d '\n/+=' | cut -c1-32; }

PW_CLUSTER=$(gen); PW_RUNTIME=$(gen); PW_MIGRATE=$(gen)
JWT=$(gen)

# Fetch the provisioning SQL (uploaded to /tmp by the caller).
SQL=/tmp/002_runtime_grants.sql
[[ -f "$SQL" ]] || { echo "provisioning sql missing"; exit 1; }

# Apply as the container's existing superuser role (avrag is rolsuper=t here),
# passing the runtime password into the SQL's :runtime_password variable.
OUT=$(docker exec -i avrag-postgres psql -U avrag -d avrag_rs -v ON_ERROR_STOP=1 \
  < <(sed "s/:runtime_password/'$PW_RUNTIME'/" "$SQL") 2>&1) || { echo "PROVISION_FAILED"; echo "$OUT" | tail -5; exit 1; }
echo "$OUT" | tail -2

# Set/rotate passwords: avrag (migrator/owner) + avrag_runtime + cluster admin.
docker exec avrag-postgres psql -U avrag -d avrag_rs -tAc \
  "ALTER ROLE avrag_cluster_admin WITH PASSWORD '$PW_CLUSTER';
   ALTER ROLE avrag WITH PASSWORD '$PW_MIGRATE';
   ALTER ROLE avrag_runtime WITH PASSWORD '$PW_RUNTIME';" >/dev/null
echo "passwords rotated"

# migrate.env: migration DSN only (avrag = migrator). Runtime env must not hold it.
MIGURL="postgresql://avrag:${PW_MIGRATE}@127.0.0.1:5432/avrag_rs"
umask 077
cat > /etc/avrag-rs/migrate.env <<EOF
MIGRATION_DATABASE_URL=$MIGURL
AVRAG_MIGRATION_ROLE_ONLY=true
AVRAG_MIGRATIONS_DIR=$APPG/migrations
EOF
chmod 600 /etc/avrag-rs/migrate.env

# avrag.env hardening: swap DSN role, kill levers/secrets, pin prod semantics.
python3 - "$PW_RUNTIME" "$JWT" <<'PY'
import os, re, sys
from pathlib import Path
pw_rt, jwt = sys.argv[1], sys.argv[2]
p = Path("/etc/avrag-rs/avrag.env")
lines = p.read_text().splitlines()
kvs = dict(l.split("=",1) for l in lines if "=" in l and not l.startswith("#"))
# runtime DSN -> avrag_runtime
new_dsn = f"postgres://avrag_runtime:{pw_rt}@127.0.0.1:5432/avrag_rs"
# rotate JWT
out, seen = [], set()
for l in lines:
    k = l.split("=",1)[0] if "=" in l else ""
    if k == "DATABASE_URL": l = f"DATABASE_URL={new_dsn}"
    if k == "JWT_SECRET": l = f"JWT_SECRET={jwt}"
    out.append(l)
    if k: seen.add(k)
def add(k, v):
    if k not in seen: out.append(f"{k}={v}"); seen.add(k)
add("JWT_SECRET", f"JWT_SECRET={jwt}")
res = [l for l in out if not l.startswith("E2E_RESET_SECRET=")]
p.write_text("\n".join(res) + "\n")
print("avrag.env: DATABASE_URL->avrag_runtime, JWT rotated, E2E_RESET_SECRET removed")
PY
grep -q '^E2E_ENABLED=' /etc/avrag-rs/avrag.env && sed -i 's/^E2E_ENABLED=.*/E2E_ENABLED=false/' /etc/avrag-rs/avrag.env
grep -q '^NODE_ENV=' /etc/avrag-rs/avrag.env && sed -i 's/^NODE_ENV=.*/NODE_ENV=production/' /etc/avrag-rs/avrag.env || true
grep -q '^TRUST_PROXY_AUTH=' /etc/avrag-rs/avrag.env && sed -i 's/^TRUST_PROXY_AUTH=.*/TRUST_PROXY_AUTH=false/' /etc/avrag-rs/avrag.env
chmod 600 /etc/avrag-rs/avrag.env

# One-time credential handoff (base64 single lines).
printf 'CLUSTER=%s\nMIGRATOR=%s\nRUNTIME=%s\nJWT=%s\n' \
  "$(printf %s "$PW_CLUSTER" | base64 -w0)" \
  "$(printf %s "$PW_MIGRATE" | base64 -w0)" \
  "$(printf %s "$PW_RUNTIME" | base64 -w0)" \
  "$(printf %s "$JWT" | base64 -w0)"