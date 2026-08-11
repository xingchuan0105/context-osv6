// hello-retrieval-mcp is a P0 spike MCP server: lexical search over v6 pgvector data.
// Transport: stdio (P0). Prod path will be Streamable HTTP (see design §9.1).
package main

import (
	"context"
	"encoding/json"
	"fmt"
	"log"
	"os"
	"strings"

	"github.com/jackc/pgx/v5/pgxpool"
	"github.com/modelcontextprotocol/go-sdk/mcp"
)

type lexicalInput struct {
	Query       string `json:"query" jsonschema:"search query text"`
	WorkspaceID string `json:"workspace_id,omitempty" jsonschema:"optional workspace UUID filter"`
	Limit       int    `json:"limit,omitempty" jsonschema:"max hits, default 5, max 20"`
}

type hit struct {
	ID          string  `json:"id"`
	WorkspaceID *string `json:"workspace_id,omitempty"`
	DocID       string  `json:"doc_id"`
	ChunkID     string  `json:"chunk_id"`
	Snippet     string  `json:"snippet"`
	Rank        float64 `json:"rank"`
}

func main() {
	// Logs MUST go to stderr in stdio MCP mode (stdout is the JSON-RPC bus).
	log.SetOutput(os.Stderr)
	log.SetFlags(log.LstdFlags | log.Lmsgprefix)
	log.SetPrefix("hello-retrieval-mcp: ")

	dsn := os.Getenv("DATABASE_URL")
	if dsn == "" {
		log.Fatal("DATABASE_URL is required")
	}

	ctx := context.Background()
	pool, err := pgxpool.New(ctx, dsn)
	if err != nil {
		log.Fatalf("pg connect: %v", err)
	}
	defer pool.Close()
	if err := pool.Ping(ctx); err != nil {
		log.Fatalf("pg ping: %v", err)
	}

	server := mcp.NewServer(&mcp.Implementation{
		Name:    "hello-retrieval",
		Version: "p0.0.1",
	}, nil)

	mcp.AddTool(server, &mcp.Tool{
		Name:        "lexical",
		Description: "Lexical (tsvector) search over v6 rag_text_chunks. P0 hello path only.",
	}, func(ctx context.Context, _ *mcp.CallToolRequest, in lexicalInput) (*mcp.CallToolResult, any, error) {
		q := strings.TrimSpace(in.Query)
		if q == "" {
			return &mcp.CallToolResult{
				IsError: true,
				Content: []mcp.Content{&mcp.TextContent{Text: "query is required"}},
			}, nil, nil
		}
		limit := in.Limit
		if limit <= 0 {
			limit = 5
		}
		if limit > 20 {
			limit = 20
		}

		// plainto_tsquery is robust for free text; ILIKE fallback when tsquery is empty/noisy.
		const sql = `
SELECT id, workspace_id::text, doc_id::text, chunk_id::text,
       left(text, 240) AS snippet,
       ts_rank_cd(search_vector, plainto_tsquery('simple', $1)) AS rank
FROM rag_text_chunks
WHERE (
        search_vector @@ plainto_tsquery('simple', $1)
        OR text ILIKE '%' || $1 || '%'
      )
  AND ($2::text = '' OR workspace_id::text = $2)
ORDER BY rank DESC NULLS LAST, id
LIMIT $3`

		ws := strings.TrimSpace(in.WorkspaceID)
		rows, err := pool.Query(ctx, sql, q, ws, limit)
		if err != nil {
			return &mcp.CallToolResult{
				IsError: true,
				Content: []mcp.Content{&mcp.TextContent{Text: fmt.Sprintf("query failed: %v", err)}},
			}, nil, nil
		}
		defer rows.Close()

		hits := make([]hit, 0, limit)
		for rows.Next() {
			var h hit
			var wsPtr *string
			if err := rows.Scan(&h.ID, &wsPtr, &h.DocID, &h.ChunkID, &h.Snippet, &h.Rank); err != nil {
				return &mcp.CallToolResult{
					IsError: true,
					Content: []mcp.Content{&mcp.TextContent{Text: fmt.Sprintf("scan: %v", err)}},
				}, nil, nil
			}
			h.WorkspaceID = wsPtr
			hits = append(hits, h)
		}
		if err := rows.Err(); err != nil {
			return &mcp.CallToolResult{
				IsError: true,
				Content: []mcp.Content{&mcp.TextContent{Text: fmt.Sprintf("rows: %v", err)}},
			}, nil, nil
		}

		payload, _ := json.MarshalIndent(map[string]any{
			"tool":        "lexical",
			"query":       q,
			"workspace_id": nullIfEmpty(ws),
			"total_hits":  len(hits),
			"hits":        hits,
		}, "", "  ")
		return &mcp.CallToolResult{
			Content: []mcp.Content{&mcp.TextContent{Text: string(payload)}},
		}, nil, nil
	})

	log.Printf("stdio MCP ready (lexical → rag_text_chunks)")
	if err := server.Run(ctx, &mcp.StdioTransport{}); err != nil {
		log.Fatal(err)
	}
}

func nullIfEmpty(s string) any {
	if s == "" {
		return nil
	}
	return s
}
