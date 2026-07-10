#!/usr/bin/env bash
# Dump document + ingestion task state for triage (CAP-INGEST).
#
# Usage:
#   bash scripts/ingest-doc-dump.sh <document_uuid>
#   DOCUMENT_ID=... bash scripts/ingest-doc-dump.sh
#
# Reads DATABASE_URL from avrag-rs/.env when unset.
# Uses app.current_role=super_admin for RLS bypass (local ops only).
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
# shellcheck source=pyramid-lib.sh
source "${ROOT}/scripts/pyramid-lib.sh"
# Only DATABASE_URL — do not source entire .env
pyramid_load_database_url

DOC_ID="${1:-${DOCUMENT_ID:-}}"
if [[ -z "$DOC_ID" ]]; then
  echo "usage: bash scripts/ingest-doc-dump.sh <document_uuid>" >&2
  exit 2
fi

if [[ -z "${DATABASE_URL:-}" ]]; then
  echo "[PYRAMID] FAIL signal=S0 next= export DATABASE_URL or set avrag-rs/.env" >&2
  exit 1
fi

if ! command -v psql >/dev/null 2>&1; then
  echo "[PYRAMID] FAIL signal=S0 next= install psql client" >&2
  exit 1
fi

# Validate UUID shape (avoid SQL injection via shell)
if ! [[ "$DOC_ID" =~ ^[0-9a-fA-F-]{36}$ ]]; then
  echo "[PYRAMID] FAIL signal=S0 next= pass a valid UUID document id" >&2
  exit 2
fi

echo "======== ingest dump document_id=${DOC_ID} ========"
echo "[PYRAMID] layer=ops cap=CAP-INGEST signal=S4"

# RLS: must be session-level (is_local=false). Under psql autocommit,
# is_local=true only applies to the set_config statement → later SELECTs return 0 rows.
# Single psql session: one connection, GUC sticky for the whole dump.
psql "$DATABASE_URL" -v ON_ERROR_STOP=1 \
  -v doc_id="$DOC_ID" <<'SQL'
SELECT set_config('app.current_role', 'super_admin', false);
-- Fail fast if GUC did not stick (should never happen with is_local=false).
SELECT CASE
  WHEN current_setting('app.current_role', true) = 'super_admin' THEN 'rls_bypass=ok'
  ELSE current_setting('app.current_role', true)
END AS rls_guc_check;

\echo --- documents ---
SELECT id, status, chunk_count, file_name, mime_type,
       object_path, updated_at, created_at
FROM documents
WHERE id = :'doc_id'::uuid;

\echo --- ingestion_tasks (latest 5) ---
SELECT task_id, status, attempt_count, locked_by, lock_token IS NOT NULL AS has_lock_token,
       left(coalesce(last_error, ''), 240) AS last_error,
       kind, queue_group, updated_at, enqueued_at
FROM ingestion_tasks
WHERE document_id = :'doc_id'::uuid
ORDER BY enqueued_at DESC
LIMIT 5;

\echo --- document_parse_runs (latest 3) ---
SELECT run_id, status, duration_ms, created_at, updated_at,
       left(coalesce(error_json::text, ''), 200) AS error_json
FROM document_parse_runs
WHERE document_id = :'doc_id'::uuid
ORDER BY created_at DESC
LIMIT 3;

\echo --- chunk / multimodal / block counts ---
SELECT
  (SELECT count(*) FROM chunks c WHERE c.document_id = :'doc_id'::uuid) AS body_chunks,
  (SELECT count(*) FROM document_multimodal_chunks m WHERE m.document_id = :'doc_id'::uuid) AS multimodal_chunks,
  (SELECT count(*) FROM document_blocks b WHERE b.document_id = :'doc_id'::uuid) AS blocks;

\echo --- terminal integrity ---
SELECT
  d.status,
  d.chunk_count,
  (
    EXISTS (
      SELECT 1 FROM chunks c
      WHERE c.document_id = d.id AND c.chunk_type = 'body'
    )
    OR EXISTS (
      SELECT 1 FROM document_multimodal_chunks m WHERE m.document_id = d.id
    )
  ) AS has_ingest_content,
  CASE
    WHEN d.status = 'completed'
         AND NOT (
           EXISTS (SELECT 1 FROM chunks c WHERE c.document_id = d.id AND c.chunk_type = 'body')
           OR EXISTS (SELECT 1 FROM document_multimodal_chunks m WHERE m.document_id = d.id)
         )
    THEN 'FALSE_COMPLETED'
    WHEN d.status = 'processing' THEN 'STILL_PROCESSING'
    WHEN d.status = 'failed' THEN 'FAILED'
    WHEN d.status = 'completed' THEN 'OK_COMPLETED'
    ELSE d.status
  END AS terminal_flag
FROM documents d
WHERE d.id = :'doc_id'::uuid;
SQL

echo ""
echo "[PYRAMID] next= rg 'stage=' /path/to/worker.log | rg ${DOC_ID}"
echo "[PYRAMID] next= bash scripts/test-l2-patho.sh"
echo "[PYRAMID] next= bash scripts/pyramid-triage.sh \"\$(tail -50 /path/to/worker.log)\""
echo "======== dump done ========"
