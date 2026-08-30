#!/usr/bin/env bash
# Set a strong password on avrag_cluster_admin via single-user one-shot, then
# restart the app container and verify TCP login. Order fixed: run the one-shot
# AFTER stopping the service container, ALWAYS restart it at the end.
set -euo pipefail
PWCA=$(openssl rand -hex 12)

sudo docker stop avrag-postgres >/dev/null || true
printf "ALTER ROLE avrag_cluster_admin LOGIN SUPERUSER PASSWORD '%s';\n" "$PWCA" \
  | sudo docker run --rm -i -v /data/avrag/pg:/var/lib/postgresql/data:rw \
      --user postgres postgres:16-alpine \
      postgres --single -D /var/lib/postgresql/data avrag_rs 2>&1 | grep -E 'ALTER|ERROR' | tail -1 || true

sudo docker start avrag-postgres >/dev/null || sudo docker restart avrag-postgres >/dev/null
sleep 3
sudo docker exec avrag-postgres psql "postgresql://avrag_cluster_admin:${PWCA}@127.0.0.1:5432/avrag_rs" -tAc 'select current_user' | tail -1
umask 077; printf '%s\n' "$PWCA" | sudo tee /root/.ca_pw >/dev/null
sudo chmod 600 /root/.ca_pw
echo "PWCA_SET=ok"