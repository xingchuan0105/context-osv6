#!/usr/bin/env bash
# Run the runtime-grants provisioning as avrag_cluster_admin over TCP, then
# rotate all passwords + JWT and write migrate.env / avrag.env.
set -euo pipefail
gen() { openssl rand -base64 24 | tr -d '\n/+=' | cut -c1-32; }
PW_RUNTIME=$(gen); PW_MIGRATE=$(gen); JWT=$(gen)
PW_CLUSTER=$(sudo cat /root/.ca_pw)

CA="sudo docker exec avrag-postgres psql postgresql://avrag_cluster_admin:${PW_CLUSTER}@127.0.0.1:5432/avrag_rs -v ON_ERROR_STOP=1"

# 1) runtime role + grants + assertions (psql resolves :runtime_password).
$CA -v runtime_password="$PW_RUNTIME" -f - < /tmp/002_vps.sql > /tmp/prov3.out 2>&1 || { echo PROVISION_FAILED; tail -4 /tmp/prov3.out; exit 1; }
tail -1 /tmp/prov3.out

# 2) rotate migrator password (owner role 'avrag') and hand cluster pw is fresh.
$CA -c "ALTER ROLE avrag WITH PASSWORD '$PW_MIGRATE';" >/dev/null

# 3) migrate.env (0600).
MIGURL="postgresql://avrag:${PW_MIGRATE}@127.0.0.1:5432/avrag_rs"
umask 077
cat <<EOF | sudo tee /etc/avrag-rs/migrate.env >/dev/null
MIGRATION_DATABASE_URL=$MIGURL
AVRAG_MIGRATION_ROLE_ONLY=true
EOF
sudo chmod 600 /etc/avrag-rs/migrate.env
echo "migrate.env written"

# 4) avrag.env: runtime DSN, JWT rotated, levers pinned, E2E_RESET_SECRET out.
NEW_DSN="postgres://avrag_runtime:${PW_RUNTIME}@127.0.0.1:5432/avrag_rs"
sudo python3 - "$NEW_DSN" "$JWT" <<'PY'
import sys
from pathlib import Path
new_dsn, jwt = sys.argv[1], sys.argv[2]
p = Path("/etc/avrag-rs/avrag.env")
pins = {
    "DATABASE_URL": new_dsn,
    "JWT_SECRET": jwt,
    "NODE_ENV": "production",
    "E2E_ENABLED": "false",
    "TRUST_PROXY_AUTH": "false",
}
out, seen = [], set()
for l in p.read_text().splitlines():
    key = l.split("=", 1)[0] if "=" in l and not l.startswith("#") else None
    if key == "E2E_RESET_SECRET":
        continue
    if key in pins:
        l = f"{key}={pins[key]}"
        seen.add(key)
    out.append(l)
    if key:
        seen.add(key)
for k, v in pins.items():
    if k not in seen:
        out.append(f"{k}={v}")
p.write_text("\n".join(out) + "\n")
PY
sudo chmod 600 /etc/avrag-rs/avrag.env
echo "avrag.env rewritten"

# 5) one-time credential handoff (base64; caller persists locally).
printf 'CLUSTER=%s\nMIGRATOR=%s\nRUNTIME=%s\nJWT=%s\n' \
  "$(printf %s "$PW_CLUSTER" | base64 -w0)" \
  "$(printf %s "$PW_MIGRATE" | base64 -w0)" \
  "$(printf %s "$PW_RUNTIME" | base64 -w0)" \
  "$(printf %s "$JWT" | base64 -w0)"