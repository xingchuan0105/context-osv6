-- E1: Chinese lexical retrieval via pg_bigm.
-- The 'simple' tsvector config does not tokenize CJK (whole-segment tokens),
-- so every Chinese bm25 query returned 0. pg_bigm's GIN index accelerates
-- LIKE/ILIKE '%term%' on the raw text column (incl. 1-2 char terms); the
-- search path for CJK queries switches to ILIKE + similarity() (search.rs).
-- pg_trgm provides the similarity() scoring function (pg_bigm has none);
-- it is used in-memory for ranking only, no pg_trgm index is created.
--
-- Environments without the pg_bigm package (e.g. stock pgvector/pgvector:pg16
-- desktop images) soft-skip the extension + gin index so later migrations still
-- apply. VGRAG dense/graph (0060) remain fully usable; CJK lexical degrades
-- until pg_bigm is installed. Production images that ship pg_bigm are unchanged.

DO $bigm$
BEGIN
  CREATE EXTENSION IF NOT EXISTS pg_bigm;
EXCEPTION
  WHEN OTHERS THEN
    RAISE NOTICE 'pg_bigm not available (%); CJK gin_bigm index will be skipped', SQLERRM;
END
$bigm$;

CREATE EXTENSION IF NOT EXISTS pg_trgm;

DO $idx$
BEGIN
  IF EXISTS (SELECT 1 FROM pg_extension WHERE extname = 'pg_bigm') THEN
    EXECUTE $sql$
      CREATE INDEX IF NOT EXISTS idx_rag_text_chunks_text_bigm
          ON rag_text_chunks USING gin (text gin_bigm_ops)
    $sql$;
  END IF;
END
$idx$;
