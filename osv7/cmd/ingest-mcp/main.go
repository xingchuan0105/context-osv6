// ingest-mcp: P3 DocumentIr intake (stdio MCP).
package main

import (
	"context"
	"log"
	"os"
	"os/signal"
	"syscall"

	"github.com/context-os/osv7/internal/billing"
	"github.com/context-os/osv7/internal/config"
	"github.com/context-os/osv7/internal/index"
	"github.com/context-os/osv7/internal/ingest"
	"github.com/context-os/osv7/internal/store"
	"github.com/modelcontextprotocol/go-sdk/mcp"
)

func main() {
	log.SetOutput(os.Stderr)
	log.SetFlags(log.LstdFlags | log.Lmsgprefix)
	log.SetPrefix("ingest-mcp: ")

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
	svc := ingest.NewService(pool, emb, bill)

	server := mcp.NewServer(&mcp.Implementation{Name: "osv7-ingest", Version: "p3.0.0"}, nil)
	register(server, svc)
	log.Printf("stdio ready embed=%v", emb.Enabled())
	if err := server.Run(ctx, &mcp.StdioTransport{}); err != nil {
		log.Fatal(err)
	}
}

func register(server *mcp.Server, svc *ingest.Service) {
	mcp.AddTool(server, &mcp.Tool{
		Name:        "ingest_preflight",
		Description: "Capability table + can_ingest before starting intake.",
	}, func(ctx context.Context, _ *mcp.CallToolRequest, _ struct{}) (*mcp.CallToolResult, any, error) {
		return ok(svc.Preflight(ctx))
	})

	type beginIn struct {
		WorkspaceID string `json:"workspace_id"`
		OwnerUserID string `json:"owner_user_id,omitempty"`
		Title       string `json:"title"`
		Source      string `json:"source,omitempty"`
		Filename    string `json:"filename,omitempty"`
	}
	mcp.AddTool(server, &mcp.Tool{
		Name:        "ingest_begin",
		Description: "Start ingest session; returns doc_id + schema_version + preflight.",
	}, func(ctx context.Context, _ *mcp.CallToolRequest, in beginIn) (*mcp.CallToolResult, any, error) {
		out, err := svc.Begin(ctx, ingest.BeginInput{
			WorkspaceID: in.WorkspaceID,
			OwnerUserID: in.OwnerUserID,
			Title:       in.Title,
			Source:      in.Source,
			Filename:    in.Filename,
		})
		return res(out, err)
	})

	type blocksIn struct {
		DocID   string            `json:"doc_id"`
		Blocks  []ingest.BlockIr  `json:"blocks"`
		Replace bool              `json:"replace,omitempty"`
	}
	mcp.AddTool(server, &mcp.Tool{
		Name:        "ingest_blocks",
		Description: "Upload/replace DocumentIr blocks for open session.",
	}, func(ctx context.Context, _ *mcp.CallToolRequest, in blocksIn) (*mcp.CallToolResult, any, error) {
		out, err := svc.PutBlocks(ctx, in.DocID, in.Blocks, in.Replace)
		return res(out, err)
	})

	type sumIn struct {
		DocID   string            `json:"doc_id"`
		Summary string            `json:"summary,omitempty"`
		KG      []ingest.KGTriple `json:"kg,omitempty"`
	}
	mcp.AddTool(server, &mcp.Tool{
		Name:        "ingest_summary",
		Description: "Set summary and optional KG triples.",
	}, func(ctx context.Context, _ *mcp.CallToolRequest, in sumIn) (*mcp.CallToolResult, any, error) {
		out, err := svc.PutSummary(ctx, in.DocID, in.Summary, in.KG)
		return res(out, err)
	})

	type pkgIn struct {
		DocID string           `json:"doc_id"`
		IR    ingest.DocumentIr `json:"ir"`
	}
	mcp.AddTool(server, &mcp.Tool{
		Name:        "ingest_package",
		Description: "Agent producer: submit full DocumentIr package for open doc_id.",
	}, func(ctx context.Context, _ *mcp.CallToolRequest, in pkgIn) (*mcp.CallToolResult, any, error) {
		out, err := svc.PutPackage(ctx, in.DocID, in.IR)
		return res(out, err)
	})

	type commitIn struct {
		DocID string `json:"doc_id"`
	}
	mcp.AddTool(server, &mcp.Tool{
		Name:        "ingest_commit",
		Description: "Hard-validate, embed, index into rag_text_chunks.",
	}, func(ctx context.Context, _ *mcp.CallToolRequest, in commitIn) (*mcp.CallToolResult, any, error) {
		out, err := svc.Commit(ctx, in.DocID)
		return res(out, err)
	})
}

func res(out any, err error) (*mcp.CallToolResult, any, error) {
	if err != nil {
		if body, ok := ingest.AsGate(err); ok {
			return &mcp.CallToolResult{
				IsError: true,
				Content: []mcp.Content{&mcp.TextContent{Text: ingest.MustJSON(body)}},
			}, nil, nil
		}
		return &mcp.CallToolResult{
			IsError: true,
			Content: []mcp.Content{&mcp.TextContent{Text: err.Error()}},
		}, nil, nil
	}
	return ok(out)
}

func ok(out any) (*mcp.CallToolResult, any, error) {
	return &mcp.CallToolResult{
		Content: []mcp.Content{&mcp.TextContent{Text: ingest.MustJSON(out)}},
	}, nil, nil
}
