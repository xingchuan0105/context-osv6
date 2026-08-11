// agentd-chat: P2 one-shot chat via pi RPC + outbound gate.
//
//	agentd-chat "hello"
//	agentd-chat -harness -workspace UUID "检索问题"
package main

import (
	"context"
	"encoding/json"
	"flag"
	"fmt"
	"log"
	"os"
	"path/filepath"
	"time"

	"github.com/context-os/osv7/internal/agentd"
	"github.com/context-os/osv7/internal/billing"
)

func main() {
	log.SetFlags(log.LstdFlags | log.Lmsgprefix)
	log.SetPrefix("agentd-chat: ")

	timeout := flag.Duration("timeout", 3*time.Minute, "turn timeout")
	provider := flag.String("provider", "", "pi provider")
	model := flag.String("model", "", "pi model")
	noExt := flag.Bool("no-extensions", false, "disable pi extensions")
	sessionDir := flag.String("session-dir", "", "persist session under dir")
	cwd := flag.String("cwd", "", "pi working directory")
	jsonOut := flag.Bool("json", false, "print ChatResult JSON")
	workspace := flag.String("workspace", "", "workspace_id for harness retrieval")
	harness := flag.Bool("harness", false, "enable osv7 retrieval tools extension")
	flag.Parse()

	msg := flag.Arg(0)
	if msg == "" {
		fmt.Fprintln(os.Stderr, "usage: agentd-chat [flags] \"message\"")
		os.Exit(2)
	}

	tryLoadEnv()

	bill := billing.NewStub(true)
	host := agentd.NewHost(bill, agentd.Config{
		Provider: *provider,
		Model:    *model,
	})

	workCwd := *cwd
	if workCwd == "" {
		if ex, err := os.Executable(); err == nil {
			workCwd = filepath.Clean(filepath.Join(filepath.Dir(ex), ".."))
		}
	}

	useHarness := *harness || *workspace != ""
	res, err := host.ChatOneShot(context.Background(), agentd.ChatOptions{
		Message:       msg,
		Timeout:       *timeout,
		SessionDir:    *sessionDir,
		Cwd:           workCwd,
		NoExtensions:  *noExt && !useHarness,
		WorkspaceID:   *workspace,
		EnableHarness: useHarness,
		RetrievalCLI:  filepath.Join(workCwd, "bin", "retrieval-cli"),
	})
	if err != nil {
		log.Fatal(err)
	}

	if *jsonOut {
		enc := json.NewEncoder(os.Stdout)
		enc.SetIndent("", "  ")
		_ = enc.Encode(res)
		if res.Blocked || res.Answer == "" {
			os.Exit(3)
		}
		return
	}

	if res.Blocked {
		fmt.Fprintf(os.Stderr, "gate blocked: %s\n", res.GateReason)
		os.Exit(3)
	}
	fmt.Println(res.Answer)
	fmt.Fprintf(os.Stderr, "<!-- events=%d tools=%d duration_ms=%d session=%s -->\n",
		res.Events, len(res.Tools), res.DurationMs, res.SessionID)
}

func tryLoadEnv() {
	candidates := []string{
		filepath.Join("..", "avrag-rs", ".env"),
		filepath.Join("avrag-rs", ".env"),
	}
	if ex, err := os.Executable(); err == nil {
		root := filepath.Clean(filepath.Join(filepath.Dir(ex), ".."))
		candidates = append(candidates, filepath.Join(root, "..", "avrag-rs", ".env"))
	}
	for _, p := range candidates {
		b, err := os.ReadFile(p)
		if err != nil {
			continue
		}
		for _, line := range splitLines(string(b)) {
			line = trimSpace(line)
			if line == "" || line[0] == '#' {
				continue
			}
			i := indexByte(line, '=')
			if i <= 0 {
				continue
			}
			k, v := line[:i], trimQuotes(line[i+1:])
			if os.Getenv(k) == "" {
				_ = os.Setenv(k, v)
			}
		}
		return
	}
}

func splitLines(s string) []string {
	var out []string
	start := 0
	for i := 0; i < len(s); i++ {
		if s[i] == '\n' {
			out = append(out, s[start:i])
			start = i + 1
		}
	}
	if start < len(s) {
		out = append(out, s[start:])
	}
	return out
}

func trimSpace(s string) string {
	for len(s) > 0 && (s[0] == ' ' || s[0] == '\t' || s[0] == '\r') {
		s = s[1:]
	}
	for len(s) > 0 && (s[len(s)-1] == ' ' || s[len(s)-1] == '\t' || s[len(s)-1] == '\r') {
		s = s[:len(s)-1]
	}
	return s
}

func trimQuotes(s string) string {
	if len(s) >= 2 {
		if (s[0] == '"' && s[len(s)-1] == '"') || (s[0] == '\'' && s[len(s)-1] == '\'') {
			return s[1 : len(s)-1]
		}
	}
	return s
}

func indexByte(s string, c byte) int {
	for i := 0; i < len(s); i++ {
		if s[i] == c {
			return i
		}
	}
	return -1
}
