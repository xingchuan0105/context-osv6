// agentd-server: HTTP API for P2 chat + session projection.
//
//	POST /v1/chat
//	POST /v1/chat/stream   SSE
//	GET  /v1/sessions?user_id=
//	GET  /v1/sessions/{id}
//	GET  /v1/sessions/{id}/messages
//	GET  /healthz
package main

import (
	"context"
	"encoding/json"
	"fmt"
	"log"
	"net/http"
	"os"
	"path/filepath"
	"strings"
	"time"

	"github.com/context-os/osv7/internal/agentd"
	"github.com/context-os/osv7/internal/billing"
	"github.com/context-os/osv7/internal/store"
)

type chatReq struct {
	Message     string `json:"message"`
	WorkspaceID string `json:"workspace_id,omitempty"`
	UserID      string `json:"user_id,omitempty"`
	SessionID   string `json:"session_id,omitempty"` // product session
	Harness     *bool  `json:"harness,omitempty"`
	Timeout     int    `json:"timeout_sec,omitempty"`
	Persist     *bool  `json:"persist,omitempty"` // default true when DATABASE_URL set
}

func main() {
	log.SetFlags(log.LstdFlags | log.Lmsgprefix)
	log.SetPrefix("agentd-server: ")
	tryLoadEnv()

	addr := envOr("OSV7_AGENTD_ADDR", ":8090")
	root := envOr("OSV7_ROOT", findRoot())

	bill := billing.NewStub(true)
	host := agentd.NewHost(bill, agentd.Config{})

	var pool *store.Pool
	if dsn := os.Getenv("DATABASE_URL"); dsn != "" {
		ctx, cancel := context.WithTimeout(context.Background(), 10*time.Second)
		p, err := store.Connect(ctx, dsn)
		cancel()
		if err != nil {
			log.Printf("store connect failed (projection off): %v", err)
		} else {
			pool = p
			if err := pool.EnsureSessionSchema(context.Background()); err != nil {
				log.Printf("schema: %v", err)
			} else {
				host = host.WithStore(pool)
				log.Printf("PG projection enabled")
			}
		}
	}

	mux := http.NewServeMux()
	mux.HandleFunc("/healthz", func(w http.ResponseWriter, _ *http.Request) {
		w.WriteHeader(http.StatusOK)
		_, _ = w.Write([]byte("ok"))
	})

	mux.HandleFunc("/v1/chat", func(w http.ResponseWriter, r *http.Request) {
		if r.Method != http.MethodPost {
			http.Error(w, "POST only", http.StatusMethodNotAllowed)
			return
		}
		var req chatReq
		if err := json.NewDecoder(r.Body).Decode(&req); err != nil {
			http.Error(w, err.Error(), http.StatusBadRequest)
			return
		}
		res, err := runChat(r.Context(), host, root, req, pool != nil, nil)
		if err != nil {
			http.Error(w, err.Error(), http.StatusBadGateway)
			return
		}
		writeJSON(w, res)
	})

	mux.HandleFunc("/v1/chat/stream", func(w http.ResponseWriter, r *http.Request) {
		if r.Method != http.MethodPost {
			http.Error(w, "POST only", http.StatusMethodNotAllowed)
			return
		}
		var req chatReq
		if err := json.NewDecoder(r.Body).Decode(&req); err != nil {
			http.Error(w, err.Error(), http.StatusBadRequest)
			return
		}
		flusher, ok := w.(http.Flusher)
		if !ok {
			http.Error(w, "streaming unsupported", http.StatusInternalServerError)
			return
		}
		w.Header().Set("Content-Type", "text/event-stream")
		w.Header().Set("Cache-Control", "no-cache")
		w.Header().Set("Connection", "keep-alive")
		writeSSE := func(event string, v any) {
			b, _ := json.Marshal(v)
			fmt.Fprintf(w, "event: %s\ndata: %s\n\n", event, b)
			flusher.Flush()
		}
		res, err := runChat(r.Context(), host, root, req, pool != nil, func(ev agentd.Event) {
			writeSSE(ev.Type, ev.Payload)
		})
		if err != nil {
			writeSSE("error", map[string]string{"error": err.Error()})
			return
		}
		writeSSE("result", res)
	})

	mux.HandleFunc("/v1/sessions", func(w http.ResponseWriter, r *http.Request) {
		if r.Method != http.MethodGet {
			http.Error(w, "GET only", http.StatusMethodNotAllowed)
			return
		}
		if pool == nil {
			http.Error(w, "projection disabled", http.StatusServiceUnavailable)
			return
		}
		user := r.URL.Query().Get("user_id")
		list, err := host.ListSessions(r.Context(), user, 50)
		if err != nil {
			http.Error(w, err.Error(), http.StatusInternalServerError)
			return
		}
		if list == nil {
			list = []store.SessionRow{}
		}
		writeJSON(w, map[string]any{"sessions": list})
	})

	mux.HandleFunc("/v1/sessions/", func(w http.ResponseWriter, r *http.Request) {
		if pool == nil {
			http.Error(w, "projection disabled", http.StatusServiceUnavailable)
			return
		}
		path := strings.TrimPrefix(r.URL.Path, "/v1/sessions/")
		parts := strings.Split(strings.Trim(path, "/"), "/")
		if len(parts) == 0 || parts[0] == "" {
			http.Error(w, "session id required", http.StatusBadRequest)
			return
		}
		id := parts[0]
		if len(parts) == 1 {
			if r.Method != http.MethodGet {
				http.Error(w, "GET only", http.StatusMethodNotAllowed)
				return
			}
			s, err := host.GetSession(r.Context(), id)
			if err != nil {
				http.Error(w, err.Error(), http.StatusInternalServerError)
				return
			}
			if s == nil {
				http.Error(w, "not found", http.StatusNotFound)
				return
			}
			writeJSON(w, s)
			return
		}
		if parts[1] == "messages" {
			if r.Method != http.MethodGet {
				http.Error(w, "GET only", http.StatusMethodNotAllowed)
				return
			}
			msgs, err := host.ListMessages(r.Context(), id, 500)
			if err != nil {
				http.Error(w, err.Error(), http.StatusInternalServerError)
				return
			}
			if msgs == nil {
				msgs = []store.MessageRow{}
			}
			writeJSON(w, map[string]any{"messages": msgs})
			return
		}
		http.NotFound(w, r)
	})

	log.Printf("listening on %s (root=%s projection=%v)", addr, root, pool != nil)
	if err := http.ListenAndServe(addr, mux); err != nil {
		log.Fatal(err)
	}
}

func runChat(ctx context.Context, host *agentd.Host, root string, req chatReq, canPersist bool, on agentd.EventHandler) (*agentd.ChatResult, error) {
	if req.Message == "" {
		return nil, fmt.Errorf("message required")
	}
	timeout := 3 * time.Minute
	if req.Timeout > 0 {
		timeout = time.Duration(req.Timeout) * time.Second
	}
	harness := req.WorkspaceID != ""
	if req.Harness != nil {
		harness = *req.Harness
	}
	persist := canPersist
	if req.Persist != nil {
		persist = *req.Persist && canPersist
	}
	return host.ChatOneShot(ctx, agentd.ChatOptions{
		Message:          req.Message,
		Timeout:          timeout,
		Cwd:              root,
		WorkspaceID:      req.WorkspaceID,
		UserID:           req.UserID,
		ProductSessionID: req.SessionID,
		EnableHarness:    harness,
		RetrievalCLI:     filepath.Join(root, "bin", "retrieval-cli"),
		OnEvent:          on,
		PersistProjection: persist,
	})
}

func writeJSON(w http.ResponseWriter, v any) {
	w.Header().Set("Content-Type", "application/json")
	enc := json.NewEncoder(w)
	enc.SetIndent("", "  ")
	_ = enc.Encode(v)
}

func findRoot() string {
	if ex, err := os.Executable(); err == nil {
		return filepath.Clean(filepath.Join(filepath.Dir(ex), ".."))
	}
	wd, _ := os.Getwd()
	return wd
}

func envOr(k, d string) string {
	if v := os.Getenv(k); v != "" {
		return v
	}
	return d
}

func tryLoadEnv() {
	for _, p := range []string{
		filepath.Join("..", "avrag-rs", ".env"),
		filepath.Join("avrag-rs", ".env"),
	} {
		b, err := os.ReadFile(p)
		if err != nil {
			continue
		}
		for _, line := range splitLines(string(b)) {
			line = trim(line)
			if line == "" || line[0] == '#' {
				continue
			}
			i := indexEq(line)
			if i <= 0 {
				continue
			}
			k, v := line[:i], unquote(line[i+1:])
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

func trim(s string) string {
	for len(s) > 0 && (s[0] == ' ' || s[0] == '\t' || s[0] == '\r') {
		s = s[1:]
	}
	for len(s) > 0 && (s[len(s)-1] == ' ' || s[len(s)-1] == '\t' || s[len(s)-1] == '\r') {
		s = s[:len(s)-1]
	}
	return s
}

func unquote(s string) string {
	if len(s) >= 2 && ((s[0] == '"' && s[len(s)-1] == '"') || (s[0] == '\'' && s[len(s)-1] == '\'')) {
		return s[1 : len(s)-1]
	}
	return s
}

func indexEq(s string) int {
	for i := 0; i < len(s); i++ {
		if s[i] == '=' {
			return i
		}
	}
	return -1
}
