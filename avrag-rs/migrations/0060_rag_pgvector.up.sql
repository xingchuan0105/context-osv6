-- pgvector retrieval data plane (vector graph RAG).
-- Mirrors storage-milvus collections: text / multimodal / kg entities / relations / passages.
-- Requires the pgvector extension (postgresql-xx-pgvector or pgvector/pgvector image).

CREATE EXTENSION IF NOT EXISTS vector;

-- ── text chunks (dense + FTS lexical) ───────────────────────────────────────
CREATE TABLE IF NOT EXISTS rag_text_chunks (
    id              TEXT PRIMARY KEY,
    owner_user_id   UUID NOT NULL,
    workspace_id    UUID,
    doc_id          UUID NOT NULL,
    chunk_id        UUID NOT NULL,
    parse_run_id    UUID NOT NULL,
    doc_version     INT NOT NULL DEFAULT 0,
    page            BIGINT,
    text            TEXT NOT NULL,
    text_dense      vector(1024) NOT NULL,
    chunk_type      TEXT NOT NULL DEFAULT 'text',
    parser_backend  TEXT,
    source_locator  JSONB,
    search_vector   tsvector
        GENERATED ALWAYS AS (to_tsvector('simple', coalesce(text, ''))) STORED
);

CREATE INDEX IF NOT EXISTS rag_text_chunks_owner_doc
    ON rag_text_chunks (owner_user_id, doc_id);
CREATE INDEX IF NOT EXISTS rag_text_chunks_parse_run
    ON rag_text_chunks (parse_run_id);
CREATE INDEX IF NOT EXISTS rag_text_chunks_dense_hnsw
    ON rag_text_chunks
    USING hnsw (text_dense vector_cosine_ops)
    WITH (m = 16, ef_construction = 64);
CREATE INDEX IF NOT EXISTS rag_text_chunks_fts_gin
    ON rag_text_chunks USING gin (search_vector);

-- ── multimodal chunks ──────────────────────────────────────────────────────
CREATE TABLE IF NOT EXISTS rag_multimodal_chunks (
    id                TEXT PRIMARY KEY,
    owner_user_id     UUID NOT NULL,
    workspace_id      UUID,
    doc_id            UUID NOT NULL,
    chunk_id          UUID NOT NULL,
    asset_id          UUID NOT NULL,
    parse_run_id      UUID NOT NULL,
    doc_version       INT NOT NULL DEFAULT 0,
    page              BIGINT,
    context_text      TEXT NOT NULL DEFAULT '',
    caption           TEXT,
    image_path        TEXT,
    multimodal_dense  vector(1024) NOT NULL,
    chunk_type        TEXT NOT NULL DEFAULT 'multimodal',
    parser_backend    TEXT,
    retrieval_weight  REAL,
    source_locator    JSONB
);

CREATE INDEX IF NOT EXISTS rag_multimodal_chunks_owner_doc
    ON rag_multimodal_chunks (owner_user_id, doc_id);
CREATE INDEX IF NOT EXISTS rag_multimodal_chunks_parse_run
    ON rag_multimodal_chunks (parse_run_id);
CREATE INDEX IF NOT EXISTS rag_multimodal_chunks_dense_hnsw
    ON rag_multimodal_chunks
    USING hnsw (multimodal_dense vector_cosine_ops)
    WITH (m = 16, ef_construction = 64);

-- ── KG entities ────────────────────────────────────────────────────────────
CREATE TABLE IF NOT EXISTS rag_kg_entities (
    id                    TEXT PRIMARY KEY,
    owner_user_id         UUID NOT NULL,
    workspace_id          UUID,
    doc_id                UUID NOT NULL,
    entity_id             UUID NOT NULL,
    parse_run_id          UUID NOT NULL,
    doc_version           INT NOT NULL DEFAULT 0,
    name                  TEXT NOT NULL,
    normalized_name       TEXT NOT NULL,
    entity_type           TEXT,
    entity_dense          vector(1024) NOT NULL,
    supporting_chunk_ids  UUID[] NOT NULL DEFAULT '{}',
    metadata              JSONB
);

CREATE INDEX IF NOT EXISTS rag_kg_entities_owner_doc
    ON rag_kg_entities (owner_user_id, doc_id);
CREATE INDEX IF NOT EXISTS rag_kg_entities_parse_run
    ON rag_kg_entities (parse_run_id);
CREATE INDEX IF NOT EXISTS rag_kg_entities_norm_name
    ON rag_kg_entities (owner_user_id, lower(normalized_name));
CREATE INDEX IF NOT EXISTS rag_kg_entities_dense_hnsw
    ON rag_kg_entities
    USING hnsw (entity_dense vector_cosine_ops)
    WITH (m = 16, ef_construction = 64);

-- ── KG relations (edges + relation vectors) ────────────────────────────────
CREATE TABLE IF NOT EXISTS rag_kg_relations (
    id                    TEXT PRIMARY KEY,
    owner_user_id         UUID NOT NULL,
    workspace_id          UUID,
    doc_id                UUID NOT NULL,
    relation_id           UUID NOT NULL,
    parse_run_id          UUID NOT NULL,
    doc_version           INT NOT NULL DEFAULT 0,
    subject               TEXT NOT NULL,
    predicate             TEXT NOT NULL,
    object                TEXT NOT NULL,
    relation_text         TEXT NOT NULL DEFAULT '',
    relation_dense        vector(1024) NOT NULL,
    supporting_chunk_ids  UUID[] NOT NULL DEFAULT '{}',
    metadata              JSONB
);

CREATE INDEX IF NOT EXISTS rag_kg_relations_owner_doc
    ON rag_kg_relations (owner_user_id, doc_id);
CREATE INDEX IF NOT EXISTS rag_kg_relations_parse_run
    ON rag_kg_relations (parse_run_id);
CREATE INDEX IF NOT EXISTS rag_kg_relations_subject
    ON rag_kg_relations (owner_user_id, subject);
CREATE INDEX IF NOT EXISTS rag_kg_relations_object
    ON rag_kg_relations (owner_user_id, object);
CREATE INDEX IF NOT EXISTS rag_kg_relations_dense_hnsw
    ON rag_kg_relations
    USING hnsw (relation_dense vector_cosine_ops)
    WITH (m = 16, ef_construction = 64);

-- ── graph passages (evidence; reserved for passage ANN) ────────────────────
CREATE TABLE IF NOT EXISTS rag_graph_passages (
    id             TEXT PRIMARY KEY,
    owner_user_id  UUID NOT NULL,
    workspace_id   UUID,
    doc_id         UUID NOT NULL,
    chunk_id       UUID,
    passage_id     UUID NOT NULL,
    parse_run_id   UUID NOT NULL,
    doc_version    INT NOT NULL DEFAULT 0,
    text           TEXT NOT NULL DEFAULT '',
    passage_dense  vector(1024) NOT NULL,
    relation_ids   UUID[] NOT NULL DEFAULT '{}',
    metadata       JSONB
);

CREATE INDEX IF NOT EXISTS rag_graph_passages_owner_doc
    ON rag_graph_passages (owner_user_id, doc_id);
CREATE INDEX IF NOT EXISTS rag_graph_passages_parse_run
    ON rag_graph_passages (parse_run_id);
CREATE INDEX IF NOT EXISTS rag_graph_passages_dense_hnsw
    ON rag_graph_passages
    USING hnsw (passage_dense vector_cosine_ops)
    WITH (m = 16, ef_construction = 64);
