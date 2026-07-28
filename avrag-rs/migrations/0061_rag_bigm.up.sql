-- E1: Chinese lexical retrieval via pg_bigm.
-- The 'simple' tsvector config does not tokenize CJK (whole-segment tokens),
-- so every Chinese bm25 query returned 0. pg_bigm's GIN index accelerates
-- LIKE/ILIKE '%term%' on the raw text column (incl. 1-2 char terms); the
-- search path for CJK queries switches to ILIKE + similarity() (search.rs).
-- pg_trgm provides the similarity() scoring function (pg_bigm has none);
-- it is used in-memory for ranking only, no pg_trgm index is created.
-- Idempotent: extensions may already exist from manual setup.

CREATE EXTENSION IF NOT EXISTS pg_bigm;
CREATE EXTENSION IF NOT EXISTS pg_trgm;

CREATE INDEX IF NOT EXISTS idx_rag_text_chunks_text_bigm
    ON rag_text_chunks USING gin (text gin_bigm_ops);
