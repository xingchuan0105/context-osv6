#!/usr/bin/env bash
# Collect VPS system/service metrics during a load run, appended as CSV rows.
# Runs against the VPS over ssh (env from avrag-rs/.env).
#
# Usage: bash scripts/loadtest/collect_vps.sh <out.csv> <interval_secs> <count>
set -euo pipefail
OUT="${1:?out csv}"; INTERVAL="${2:-5}"; COUNT="${3:-120}"

ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
set -a; source "$ROOT/avrag-rs/.env"; set +a
SSH=(sshpass -p "$VPS_MAIN_PASSWORD" ssh -o StrictHostKeyChecking=no "$VPS_MAIN_USER@$VPS_MAIN_HOST")

echo "ts,cpu_pct,mem_used_mb,load1,tcp_estab,tcp_timewait,pg_conns,redis_conns,api_inflight,sse_active" >> "$OUT"

for _ in $(seq 1 "$COUNT"); do
  row=$("${SSH[@]}" '
    ts=$(date -u +%FT%TZ)
    cpu=$(top -bn1 | awk "/Cpu\\(s\\)/{print 100-\$8}" | cut -d. -f1)
    mem=$(free -m | awk "/Mem:/{print \$3}")
    load=$(cut -d" " -f1 /proc/loadavg)
    estab=$(ss -s | awk "/estab/{print \$2}" | tr -d ",")
    tw=$(ss -s | awk "/timewait/{print \$2}" | tr -d ",")
    pg=$(docker exec avrag-postgres psql -U avrag -d postgres -t -c "select count(*) from pg_stat_activity" 2>/dev/null | tr -d " ")
    redis=$(docker exec avrag-redis redis-cli client list 2>/dev/null | wc -l | tr -d " ")
    metrics=$(curl -s -m 3 http://127.0.0.1:8081/metrics 2>/dev/null)
    inflight=$(echo "$metrics" | awk "/^avrag_http_inflight /{print \$2}")
    sse=$(echo "$metrics" | awk "/^avrag_sse_active /{print \$2}")
    echo "$ts,${cpu:-0},${mem:-0},${load:-0},${estab:-0},${tw:-0},${pg:-0},${redis:-0},${inflight:-0},${sse:-0}"
  ' 2>/dev/null || echo "$(date -u +%FT%TZ),collect_error,,,,,,,,")
  echo "$row" >> "$OUT"
  sleep "$INTERVAL"
done
echo "collect done → $OUT"
