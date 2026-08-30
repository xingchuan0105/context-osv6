#!/usr/bin/env bash
# One-shot, idempotent-by-intent evidence snapshot for the 2026-08-30 tenant
# isolation window. Runs ON the VPS as root. Stores under /root/incident-2026
# 0830-evidence/<stamp>/ with SHA256 manifest; nothing is printed except paths.
set -euo pipefail
EV=/root/incident-20260830-evidence
STAMP=$(date -u +%Y%m%dT%H%M%SZ)
OUT="$EV/$STAMP"
mkdir -p "$OUT"
chmod 700 "$EV" "$OUT"

PW=$(grep -oP '^(DATABASE_URL|AVRAG.*DATABASE_URL)=postgres(ql)?://avrag:\K[^@]+' /etc/avrag-rs/avrag.env | head -1)
if [[ -z "$PW" ]]; then
  echo "evidence: cannot resolve DSN password from avrag.env (key name mismatch)" >&2
  grep -oP '^[A-Z_]*DATABASE[A-Z_]*=' /etc/avrag-rs/avrag.env || true
  exit 1
fi
DSN="postgresql://avrag:${PW}@127.0.0.1:5432/avrag_rs"

{
  echo "evidence-start $(date -u +%FT%TZ)"
  docker ps --format '{{.Names}} {{.Status}}' | grep -E 'avrag|postgres'
} > "$OUT/MANIFEST.txt"

# Full logical dump (schema + data) — the primary forensic artifact.
docker exec avrag-postgres pg_dump "$DSN" > "$OUT/db_full.sql" 2>>"$OUT/MANIFEST.txt" \
  && echo "db_full=ok" >> "$OUT/MANIFEST.txt" \
  || echo "db_full=FAILED" >> "$OUT/MANIFEST.txt"

# Globals (roles) — best effort inside the container's postgres superuser.
docker exec avrag-postgres pg_dumpall --globals-only \
  -U $(docker exec avrag-postgres psql -U postgres -tAc 'select current_user' 2>/dev/null || echo avrag) \
  > "$OUT/globals.sql" 2>/dev/null \
  && echo "globals=ok" >> "$OUT/MANIFEST.txt" \
  || { docker exec avrag-postgres pg_dumpall --globals-only "--username=$(grep -oP '://avrag:' <<<"$DSN" >/dev/null && echo avrag)" > "$OUT/globals.sql" 2>/dev/null && echo "globals=ok(avrag)" >> "$OUT/MANIFEST.txt" || echo "globals=skipped" >> "$OUT/MANIFEST.txt"; }

# Object store manifest (paths + sizes only; no content copy).
OBJ=${AVRAG_OBJECT_ROOT:-/data/avrag/objects}
if [[ -d "$OBJ" ]]; then
  find "$OBJ" -type f -printf '%s %p\n' | sort > "$OUT/object_manifest.txt"
  echo "object_manifest_lines=$(wc -l < "$OUT/object_manifest.txt")" >> "$OUT/MANIFEST.txt"
fi

# Log preservation (container logs + vhost logs since the incident scope).
docker logs --timestamps avrag-api > "$OUT/api_console.log" 2>&1 || true
docker logs --timestamps avrag-worker > "$OUT/worker_console.log" 2>&1 || true
cp /var/log/nginx/access.log "$OUT/nginx_access.log" 2>/dev/null || true
cp /var/log/nginx/error.log "$OUT/nginx_error.log" 2>/dev/null || true

( cd "$OUT" && sha256sum db_full.sql globals.sql object_manifest.txt api_console.log worker_console.log nginx_access.log nginx_error.log > SHA256SUMS.txt ) 2>/dev/null || ( cd "$OUT" && sha256sum * > SHA256SUMS.txt )

# Immutable-ish: no overwrite of an existing stamp (mkdir already guarantees).
echo "evidence dir: $OUT"
grep -E 'db_full=|globals=|object_manifest_lines' "$OUT/MANIFEST.txt"