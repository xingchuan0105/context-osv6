// ingest-cli: agent package + server_parse text producers.
//
//	ingest-cli preflight
//	ingest-cli agent-package --workspace UUID --title T --file package.json
//	ingest-cli server-parse --workspace UUID --file doc.md
package main

import (
	"context"
	"encoding/json"
	"flag"
	"fmt"
	"os"
	"path/filepath"

	"github.com/context-os/osv7/internal/billing"
	"github.com/context-os/osv7/internal/config"
	"github.com/context-os/osv7/internal/index"
	"github.com/context-os/osv7/internal/ingest"
	"github.com/context-os/osv7/internal/store"
)

func main() {
	if len(os.Args) < 2 {
		fmt.Fprintln(os.Stderr, "usage: ingest-cli <preflight|agent-package|server-parse>")
		os.Exit(2)
	}
	cmd := os.Args[1]
	args := os.Args[2:]

	cfg, err := config.Load()
	must(err)
	ctx := context.Background()
	pool, err := store.Connect(ctx, cfg.DatabaseURL)
	must(err)
	defer pool.Close()
	emb := index.NewEmbedder(cfg.EmbedBaseURL, cfg.EmbedAPIKey, cfg.EmbedModel, cfg.EmbedDim, cfg.EmbedTimeout)
	wallet := billing.NewWalletService(pool)
	_ = wallet.EnsureSchema(ctx)
	svc := ingest.NewService(pool, emb, wallet).WithWallet(wallet)

	switch cmd {
	case "preflight":
		fmt.Println(ingest.MustJSON(svc.Preflight(ctx)))
	case "agent-package":
		fs := flag.NewFlagSet("agent-package", flag.ExitOnError)
		ws := fs.String("workspace", "", "workspace uuid")
		title := fs.String("title", "", "title override")
		file := fs.String("file", "", "DocumentIr JSON file")
		_ = fs.Parse(args)
		raw, err := os.ReadFile(*file)
		must(err)
		var ir ingest.DocumentIr
		must(json.Unmarshal(raw, &ir))
		if *title != "" {
			ir.Title = *title
		}
		begin, err := svc.Begin(ctx, ingest.BeginInput{
			WorkspaceID: *ws,
			Title:       ir.Title,
			Source:      "agent_package",
			Filename:    filepath.Base(*file),
		})
		mustGate(err)
		docID, _ := begin["doc_id"].(string)
		_, err = svc.PutPackage(ctx, docID, ir)
		mustGate(err)
		out, err := svc.Commit(ctx, docID)
		mustGate(err)
		fmt.Println(ingest.MustJSON(out))
	case "server-parse":
		fs := flag.NewFlagSet("server-parse", flag.ExitOnError)
		ws := fs.String("workspace", "", "workspace uuid")
		file := fs.String("file", "", "text/markdown path")
		_ = fs.Parse(args)
		ir, err := ingest.ParseTextFile(*file)
		must(err)
		begin, err := svc.Begin(ctx, ingest.BeginInput{
			WorkspaceID: *ws,
			Title:       ir.Title,
			Source:      "server_parse",
			Filename:    filepath.Base(*file),
		})
		mustGate(err)
		docID, _ := begin["doc_id"].(string)
		_, err = svc.PutPackage(ctx, docID, *ir)
		mustGate(err)
		out, err := svc.Commit(ctx, docID)
		mustGate(err)
		fmt.Println(ingest.MustJSON(out))
	default:
		fmt.Fprintln(os.Stderr, "unknown command")
		os.Exit(2)
	}
}

func must(err error) {
	if err != nil {
		fmt.Fprintln(os.Stderr, err)
		os.Exit(1)
	}
}

func mustGate(err error) {
	if err == nil {
		return
	}
	if body, ok := ingest.AsGate(err); ok {
		fmt.Fprintln(os.Stderr, ingest.MustJSON(body))
		os.Exit(3)
	}
	must(err)
}

