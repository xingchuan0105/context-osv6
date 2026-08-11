// retrieval-mcp: P1 harness MCP (题卡 + 资源/契约闸 + dense/lexical/grep + 证据句柄).
//
// Transport: stdio by default; set OSV7_MCP_HTTP_ADDR=:8081 for Streamable HTTP.
package main

import (
	"context"
	"log"
	"net/http"
	"os"
	"os/signal"
	"syscall"

	"github.com/context-os/osv7/internal/billing"
	"github.com/context-os/osv7/internal/config"
	"github.com/context-os/osv7/internal/index"
	"github.com/context-os/osv7/internal/retrieval"
	"github.com/context-os/osv7/internal/store"
	"github.com/modelcontextprotocol/go-sdk/mcp"
)

func main() {
	log.SetOutput(os.Stderr)
	log.SetFlags(log.LstdFlags | log.Lmsgprefix)
	log.SetPrefix("retrieval-mcp: ")

	cfg, err := config.Load()
	if err != nil {
		log.Fatal(err)
	}
	ctx, cancel := signal.NotifyContext(context.Background(), syscall.SIGINT, syscall.SIGTERM)
	defer cancel()

	pool, err := store.Connect(ctx, cfg.DatabaseURL)
	if err != nil {
		log.Fatal(err)
	}
	defer pool.Close()

	emb := index.NewEmbedder(cfg.EmbedBaseURL, cfg.EmbedAPIKey, cfg.EmbedModel, cfg.EmbedDim, cfg.EmbedTimeout)
	bill := billing.NewStub(emb.Enabled())
	ix := index.New(pool, emb)
	svc := retrieval.NewService(pool, ix, bill, cfg.DefaultUserID)

	server := mcp.NewServer(&mcp.Implementation{
		Name:    "osv7-retrieval",
		Version: "p1.0.0",
	}, nil)
	registerTools(server, svc)

	if cfg.HTTPAddr != "" {
		handler := mcp.NewStreamableHTTPHandler(func(_ *http.Request) *mcp.Server {
			// P1: one process-wide session (single-tenant spike). Multi-session map in P2.
			return server
		}, nil)
		log.Printf("Streamable HTTP on %s (also run with empty OSV7_MCP_HTTP_ADDR for stdio)", cfg.HTTPAddr)
		go func() {
			if err := http.ListenAndServe(cfg.HTTPAddr, handler); err != nil {
				log.Printf("http: %v", err)
			}
		}()
	}

	log.Printf("stdio MCP ready (embed=%v user=%q)", emb.Enabled(), cfg.DefaultUserID)
	if err := server.Run(ctx, &mcp.StdioTransport{}); err != nil {
		log.Fatal(err)
	}
}

func registerTools(server *mcp.Server, svc *retrieval.Service) {
	type cardIn struct {
		QuestionType     string   `json:"question_type" jsonschema:"rag_fact|table_count|calculation|chitchat|other"`
		RequiredActions  []string `json:"required_actions" jsonschema:"harness actions: dense,lexical,grep,..."`
		WorkspaceID      string   `json:"workspace_id" jsonschema:"required workspace UUID"`
		DocIDs           []string `json:"doc_ids,omitempty"`
		WebIntent        bool     `json:"web_intent,omitempty"`
		EvidenceRequired bool     `json:"evidence_required,omitempty"`
		Mode             string   `json:"mode,omitempty" jsonschema:"open or explicit"`
		// Flattened explicit options for MCP schema simplicity
		ExplicitWorkspaceID string   `json:"explicit_workspace_id,omitempty"`
		ExplicitActions     []string `json:"explicit_required_actions,omitempty"`
	}

	mcp.AddTool(server, &mcp.Tool{
		Name:        "set_query_card",
		Description: "Install task-level query card. Required before any retrieval primitive. Resource + dual-mode shape gates apply.",
	}, func(ctx context.Context, _ *mcp.CallToolRequest, in cardIn) (*mcp.CallToolResult, any, error) {
		card := retrieval.QueryCard{
			QuestionType:     in.QuestionType,
			RequiredActions:  in.RequiredActions,
			WorkspaceID:      in.WorkspaceID,
			DocIDs:           in.DocIDs,
			WebIntent:        in.WebIntent,
			EvidenceRequired: in.EvidenceRequired,
			Mode:             in.Mode,
		}
		if in.Mode == "explicit" || in.ExplicitWorkspaceID != "" {
			card.Mode = "explicit"
			card.ExplicitOptions = &retrieval.ExplicitOptions{
				WorkspaceID:     firstNonEmpty(in.ExplicitWorkspaceID, in.WorkspaceID),
				RequiredActions: in.ExplicitActions,
			}
		}
		out, err := svc.SetQueryCard(ctx, card)
		return toolResult(out, err)
	})

	type qIn struct {
		Query string `json:"query" jsonschema:"search query"`
		Limit int    `json:"limit,omitempty"`
	}
	mcp.AddTool(server, &mcp.Tool{
		Name:        "lexical",
		Description: "Lexical / FTS (or CJK LIKE) over rag_text_chunks. Requires active query card.",
	}, func(ctx context.Context, _ *mcp.CallToolRequest, in qIn) (*mcp.CallToolResult, any, error) {
		out, err := svc.Lexical(ctx, in.Query, in.Limit)
		return toolResult(out, err)
	})

	mcp.AddTool(server, &mcp.Tool{
		Name:        "dense",
		Description: "Dense vector search (embed query + pgvector). Requires card + embedding capability.",
	}, func(ctx context.Context, _ *mcp.CallToolRequest, in qIn) (*mcp.CallToolResult, any, error) {
		out, err := svc.Dense(ctx, in.Query, in.Limit)
		return toolResult(out, err)
	})

	type grepIn struct {
		Pattern string `json:"pattern" jsonschema:"literal substring"`
		Limit   int    `json:"limit,omitempty"`
	}
	mcp.AddTool(server, &mcp.Tool{
		Name:        "grep",
		Description: "Literal substring grep over chunk text. Requires active query card.",
	}, func(ctx context.Context, _ *mcp.CallToolRequest, in grepIn) (*mcp.CallToolResult, any, error) {
		out, err := svc.Grep(ctx, in.Pattern, in.Limit)
		return toolResult(out, err)
	})

	type selIn struct {
		Aliases []string `json:"aliases" jsonschema:"e.g. #1 #2"`
	}
	mcp.AddTool(server, &mcp.Tool{
		Name:        "select_evidence",
		Description: "SELECTED: mark evidence handles for later KEEP / verify_draft.",
	}, func(_ context.Context, _ *mcp.CallToolRequest, in selIn) (*mcp.CallToolResult, any, error) {
		out, err := svc.Select(in.Aliases)
		return toolResult(out, err)
	})

	mcp.AddTool(server, &mcp.Tool{
		Name:        "keep_evidence",
		Description: "KEEP: freeze selected (or listed) handles.",
	}, func(_ context.Context, _ *mcp.CallToolRequest, in selIn) (*mcp.CallToolResult, any, error) {
		out, err := svc.Keep(in.Aliases)
		return toolResult(out, err)
	})

	type verIn struct {
		Draft            string `json:"draft" jsonschema:"answer draft text containing #n citations"`
		RequireSelected  bool   `json:"require_selected,omitempty"`
	}
	mcp.AddTool(server, &mcp.Tool{
		Name:        "verify_draft",
		Description: "Handle-level citation check only (no LLM). Also enforces contract gate on required_actions.",
	}, func(_ context.Context, _ *mcp.CallToolRequest, in verIn) (*mcp.CallToolResult, any, error) {
		out, err := svc.VerifyDraft(in.Draft, in.RequireSelected)
		return toolResult(out, err)
	})

	mcp.AddTool(server, &mcp.Tool{
		Name:        "retrieval_status",
		Description: "Current card, Ok actions, aliases, selected/kept, capabilities.",
	}, func(_ context.Context, _ *mcp.CallToolRequest, _ struct{}) (*mcp.CallToolResult, any, error) {
		return toolResult(svc.Status(), nil)
	})
}

func toolResult(out any, err error) (*mcp.CallToolResult, any, error) {
	if err != nil {
		if body, ok := retrieval.AsGate(err); ok {
			return &mcp.CallToolResult{
				IsError: true,
				Content: []mcp.Content{&mcp.TextContent{Text: body.JSON()}},
			}, nil, nil
		}
		return &mcp.CallToolResult{
			IsError: true,
			Content: []mcp.Content{&mcp.TextContent{Text: err.Error()}},
		}, nil, nil
	}
	return &mcp.CallToolResult{
		Content: []mcp.Content{&mcp.TextContent{Text: retrieval.MustJSON(out)}},
	}, nil, nil
}

func firstNonEmpty(a, b string) string {
	if a != "" {
		return a
	}
	return b
}
