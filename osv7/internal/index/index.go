package index

import (
	"context"
	"fmt"
	"strings"
	"unicode"

	"github.com/context-os/osv7/internal/store"
	"github.com/jackc/pgx/v5"
	"github.com/pgvector/pgvector-go"
)

// Index owns vector/lexical/grep over rag_text_chunks (SQL only via store.Pool).
type Index struct {
	pool *store.Pool
	emb  *Embedder
}

func New(pool *store.Pool, emb *Embedder) *Index {
	return &Index{pool: pool, emb: emb}
}

func (ix *Index) clampLimit(n int) int {
	if n <= 0 {
		return 8
	}
	if n > 32 {
		return 32
	}
	return n
}

// Lexical uses tsvector for non-CJK and LIKE for CJK (v6 E1 simplified).
func (ix *Index) Lexical(ctx context.Context, query string, opts SearchOpts) ([]Hit, error) {
	q := strings.TrimSpace(query)
	if q == "" {
		return nil, fmt.Errorf("empty query")
	}
	limit := ix.clampLimit(opts.Limit)
	if hasCJK(q) {
		return ix.queryHits(ctx, `
SELECT chunk_id::text, doc_id::text, coalesce(workspace_id::text,''), text, page,
       1.0::float8 AS score
FROM rag_text_chunks
WHERE text LIKE '%' || $1 || '%'
  AND ($2::text = '' OR workspace_id::text = $2)
  AND ($3::text = '' OR owner_user_id::text = $3)
  AND ($4::int = 0 OR doc_id = ANY(SELECT unnest($5::uuid[])))
ORDER BY id
LIMIT $6`, "pgvector_cjk_like", q, opts.WorkspaceID, opts.OwnerUserID, len(opts.DocIDs), opts.DocIDs, limit)
	}
	return ix.queryHits(ctx, `
SELECT chunk_id::text, doc_id::text, coalesce(workspace_id::text,''), text, page,
       ts_rank(search_vector, plainto_tsquery('simple', $1))::float8 AS score
FROM rag_text_chunks
WHERE search_vector @@ plainto_tsquery('simple', $1)
  AND ($2::text = '' OR workspace_id::text = $2)
  AND ($3::text = '' OR owner_user_id::text = $3)
  AND ($4::int = 0 OR doc_id = ANY(SELECT unnest($5::uuid[])))
ORDER BY score DESC
LIMIT $6`, "pgvector_fts", q, opts.WorkspaceID, opts.OwnerUserID, len(opts.DocIDs), opts.DocIDs, limit)
}

// Grep is literal substring search.
func (ix *Index) Grep(ctx context.Context, pattern string, opts SearchOpts) ([]Hit, error) {
	pat := strings.TrimSpace(pattern)
	if pat == "" {
		return nil, fmt.Errorf("empty pattern")
	}
	limit := ix.clampLimit(opts.Limit)
	return ix.queryHits(ctx, `
SELECT chunk_id::text, doc_id::text, coalesce(workspace_id::text,''), text, page,
       1.0::float8 AS score
FROM rag_text_chunks
WHERE position($1 in text) > 0
  AND ($2::text = '' OR workspace_id::text = $2)
  AND ($3::text = '' OR owner_user_id::text = $3)
  AND ($4::int = 0 OR doc_id = ANY(SELECT unnest($5::uuid[])))
ORDER BY id
LIMIT $6`, "grep", pat, opts.WorkspaceID, opts.OwnerUserID, len(opts.DocIDs), opts.DocIDs, limit)
}

// Dense embeds query then cosine distance search on text_dense.
func (ix *Index) Dense(ctx context.Context, query string, opts SearchOpts) ([]Hit, error) {
	if ix.emb == nil || !ix.emb.Enabled() {
		return nil, fmt.Errorf("embedding not configured")
	}
	q := strings.TrimSpace(query)
	if q == "" {
		return nil, fmt.Errorf("empty query")
	}
	vec, err := ix.emb.Embed(ctx, q)
	if err != nil {
		return nil, err
	}
	limit := ix.clampLimit(opts.Limit)
	v := pgvector.NewVector(vec)
	return ix.queryHits(ctx, `
SELECT chunk_id::text, doc_id::text, coalesce(workspace_id::text,''), text, page,
       (1.0 - (text_dense <=> $1))::float8 AS score
FROM rag_text_chunks
WHERE ($2::text = '' OR workspace_id::text = $2)
  AND ($3::text = '' OR owner_user_id::text = $3)
  AND ($4::int = 0 OR doc_id = ANY(SELECT unnest($5::uuid[])))
ORDER BY text_dense <=> $1
LIMIT $6`, "pgvector_text_dense", v, opts.WorkspaceID, opts.OwnerUserID, len(opts.DocIDs), opts.DocIDs, limit)
}

func (ix *Index) queryHits(ctx context.Context, sql, backend string, args ...any) ([]Hit, error) {
	// Ensure empty doc slice binds cleanly.
	for i, a := range args {
		if s, ok := a.([]string); ok && s == nil {
			args[i] = []string{}
		}
	}
	rows, err := ix.pool.Query(ctx, sql, args...)
	if err != nil {
		return nil, err
	}
	defer rows.Close()
	return collectHits(rows, backend)
}

func collectHits(rows pgx.Rows, backend string) ([]Hit, error) {
	var out []Hit
	for rows.Next() {
		var h Hit
		var page *int64
		var text string
		if err := rows.Scan(&h.ChunkID, &h.DocID, &h.WorkspaceID, &text, &page, &h.Score); err != nil {
			return nil, err
		}
		h.Text = text
		h.Snippet = snippet(text, 240)
		h.Page = page
		h.Backend = backend
		out = append(out, h)
	}
	return out, rows.Err()
}

func snippet(s string, n int) string {
	s = strings.TrimSpace(s)
	r := []rune(s)
	if len(r) <= n {
		return s
	}
	return string(r[:n])
}

func hasCJK(s string) bool {
	for _, r := range s {
		if unicode.In(r, unicode.Han, unicode.Hangul, unicode.Hiragana, unicode.Katakana) {
			return true
		}
	}
	return false
}
