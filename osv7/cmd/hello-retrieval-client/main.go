// hello-retrieval-client: call the stdio MCP server without pi (proves MCP path).
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
	bin := os.Getenv("HELLO_RETRIEVAL_MCP_BIN")
	if bin == "" {
		bin = "./bin/hello-retrieval-mcp"
	}
	query := "滴灌通"
	if len(os.Args) > 1 {
		query = os.Args[1]
	}

	ctx := context.Background()
	client := mcp.NewClient(&mcp.Implementation{Name: "hello-client", Version: "p0"}, nil)

	cmd := exec.Command(bin)
	cmd.Env = os.Environ()
	cmd.Stderr = os.Stderr
	transport := &mcp.CommandTransport{Command: cmd}
	session, err := client.Connect(ctx, transport, nil)
	if err != nil {
		log.Fatalf("connect: %v", err)
	}
	defer session.Close()

	res, err := session.CallTool(ctx, &mcp.CallToolParams{
		Name: "lexical",
		Arguments: map[string]any{
			"query": query,
			"limit": 3,
		},
	})
	if err != nil {
		log.Fatalf("CallTool: %v", err)
	}
	if res.IsError {
		log.Fatal("tool returned IsError")
	}
	for _, c := range res.Content {
		if t, ok := c.(*mcp.TextContent); ok {
			fmt.Println(t.Text)
		}
	}
}
