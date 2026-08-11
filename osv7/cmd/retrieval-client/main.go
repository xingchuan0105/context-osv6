// retrieval-client: exercise P1 MCP tools against stdio server (no pi).
package main

import (
	"context"
	"fmt"
	"log"
	"os"
	"os/exec"

	"github.com/modelcontextprotocol/go-sdk/mcp"
)

func main() {
	bin := env("RETRIEVAL_MCP_BIN", "./bin/retrieval-mcp")
	// Default: a local workspace known to contain sample CJK corpus (滴灌通).
	ws := env("OSV7_WORKSPACE_ID", "0c8391f1-8bfb-415f-9a7f-10624b7cfb4d")
	query := "滴灌通"
	if len(os.Args) > 1 {
		query = os.Args[1]
	}

	ctx := context.Background()
	client := mcp.NewClient(&mcp.Implementation{Name: "retrieval-client", Version: "p1"}, nil)
	cmd := exec.Command(bin)
	cmd.Env = os.Environ()
	cmd.Stderr = os.Stderr
	session, err := client.Connect(ctx, &mcp.CommandTransport{Command: cmd}, nil)
	if err != nil {
		log.Fatal(err)
	}
	defer session.Close()

	call := func(name string, args map[string]any) {
		res, err := session.CallTool(ctx, &mcp.CallToolParams{Name: name, Arguments: args})
		if err != nil {
			log.Fatalf("%s: %v", name, err)
		}
		fmt.Printf("\n=== %s IsError=%v ===\n", name, res.IsError)
		for _, c := range res.Content {
			if t, ok := c.(*mcp.TextContent); ok {
				fmt.Println(t.Text)
			}
		}
		if res.IsError {
			log.Fatalf("tool %s failed", name)
		}
	}

	// Gate: no card → error
	res, err := session.CallTool(ctx, &mcp.CallToolParams{
		Name:      "lexical",
		Arguments: map[string]any{"query": query, "limit": 2},
	})
	if err != nil {
		log.Fatal(err)
	}
	fmt.Printf("\n=== lexical without card IsError=%v (expect true) ===\n", res.IsError)
	for _, c := range res.Content {
		if t, ok := c.(*mcp.TextContent); ok {
			fmt.Println(t.Text)
		}
	}
	if !res.IsError {
		log.Fatal("expected resource/card gate without set_query_card")
	}

	call("set_query_card", map[string]any{
		"question_type":    "rag_fact",
		"required_actions": []string{"lexical", "dense"},
		"workspace_id":     ws,
		"mode":             "open",
	})

	call("lexical", map[string]any{"query": query, "limit": 3})
	call("dense", map[string]any{"query": query, "limit": 3})
	call("select_evidence", map[string]any{"aliases": []string{"#1"}})
	call("keep_evidence", map[string]any{"aliases": []string{}})
	call("verify_draft", map[string]any{
		"draft":             "根据 #1 的材料，结论如下。",
		"require_selected":  true,
	})
	call("retrieval_status", map[string]any{})
	fmt.Println("\n==> P1 client OK")
}

func env(k, def string) string {
	if v := os.Getenv(k); v != "" {
		return v
	}
	return def
}
