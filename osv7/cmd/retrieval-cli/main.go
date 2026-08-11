// retrieval-cli: same harness Service as retrieval-mcp, for pi extension / scripts.
// Session state is file-backed (full snapshot including aliases).
package main

import (
	"context"
	"encoding/json"
	"flag"
	"fmt"
	"os"
	"path/filepath"
	"strings"

	"github.com/context-os/osv7/internal/billing"
	"github.com/context-os/osv7/internal/config"
	"github.com/context-os/osv7/internal/index"
	"github.com/context-os/osv7/internal/retrieval"
	"github.com/context-os/osv7/internal/store"
)

func main() {
	if len(os.Args) < 2 {
		fmt.Fprintln(os.Stderr, "usage: retrieval-cli <set-card|lexical|dense|grep|select|keep|verify|status> [flags]")
		os.Exit(2)
	}
	cmd := os.Args[1]
	args := os.Args[2:]

	cfg, err := config.Load()
	if err != nil {
		fatal(err)
	}
	ctx := context.Background()
	pool, err := store.Connect(ctx, cfg.DatabaseURL)
	if err != nil {
		fatal(err)
	}
	defer pool.Close()
	emb := index.NewEmbedder(cfg.EmbedBaseURL, cfg.EmbedAPIKey, cfg.EmbedModel, cfg.EmbedDim, cfg.EmbedTimeout)
	bill := billing.NewStub(emb.Enabled())
	ix := index.New(pool, emb)
	svc := retrieval.NewService(pool, ix, bill, cfg.DefaultUserID)
	loadSnapshot(svc)

	var out any
	switch cmd {
	case "set-card":
		fs := flag.NewFlagSet("set-card", flag.ExitOnError)
		ws := fs.String("workspace", "", "workspace_id")
		actions := fs.String("actions", "lexical,dense", "comma required_actions")
		qtype := fs.String("type", "rag_fact", "question_type")
		_ = fs.Parse(args)
		card := retrieval.QueryCard{
			QuestionType:    *qtype,
			WorkspaceID:     *ws,
			RequiredActions: splitCSV(*actions),
			Mode:            "open",
		}
		out, err = svc.SetQueryCard(ctx, card)
	case "lexical":
		fs := flag.NewFlagSet("lexical", flag.ExitOnError)
		q := fs.String("query", "", "query")
		limit := fs.Int("limit", 8, "limit")
		_ = fs.Parse(args)
		out, err = svc.Lexical(ctx, *q, *limit)
	case "dense":
		fs := flag.NewFlagSet("dense", flag.ExitOnError)
		q := fs.String("query", "", "query")
		limit := fs.Int("limit", 8, "limit")
		_ = fs.Parse(args)
		out, err = svc.Dense(ctx, *q, *limit)
	case "grep":
		fs := flag.NewFlagSet("grep", flag.ExitOnError)
		p := fs.String("pattern", "", "pattern")
		limit := fs.Int("limit", 8, "limit")
		_ = fs.Parse(args)
		out, err = svc.Grep(ctx, *p, *limit)
	case "select":
		fs := flag.NewFlagSet("select", flag.ExitOnError)
		a := fs.String("aliases", "", "#1,#2")
		_ = fs.Parse(args)
		out, err = svc.Select(splitCSV(*a))
	case "keep":
		fs := flag.NewFlagSet("keep", flag.ExitOnError)
		a := fs.String("aliases", "", "optional")
		_ = fs.Parse(args)
		out, err = svc.Keep(splitCSV(*a))
	case "verify":
		fs := flag.NewFlagSet("verify", flag.ExitOnError)
		d := fs.String("draft", "", "draft text")
		req := fs.Bool("require-selected", false, "require selected")
		_ = fs.Parse(args)
		out, err = svc.VerifyDraft(*d, *req)
	case "status":
		out = svc.Status()
	default:
		fatal(fmt.Errorf("unknown command %s", cmd))
	}

	if err != nil {
		if body, ok := retrieval.AsGate(err); ok {
			fmt.Fprintln(os.Stderr, body.JSON())
			os.Exit(3)
		}
		fatal(err)
	}
	saveSnapshot(svc)
	fmt.Println(retrieval.MustJSON(out))
}

func splitCSV(s string) []string {
	var out []string
	for _, p := range strings.Split(s, ",") {
		p = strings.TrimSpace(p)
		if p != "" {
			out = append(out, p)
		}
	}
	return out
}

func fatal(err error) {
	fmt.Fprintln(os.Stderr, err.Error())
	os.Exit(1)
}

func statePath() string {
	if p := os.Getenv("OSV7_RETRIEVAL_STATE"); p != "" {
		return p
	}
	return filepath.Join(os.TempDir(), "osv7-retrieval-state.json")
}

func loadSnapshot(svc *retrieval.Service) {
	b, err := os.ReadFile(statePath())
	if err != nil {
		return
	}
	var snap retrieval.Snapshot
	if json.Unmarshal(b, &snap) != nil {
		return
	}
	svc.Session().ImportSnapshot(snap)
}

func saveSnapshot(svc *retrieval.Service) {
	snap := svc.Session().ExportSnapshot()
	raw, _ := json.MarshalIndent(snap, "", "  ")
	_ = os.WriteFile(statePath(), raw, 0o600)
}
