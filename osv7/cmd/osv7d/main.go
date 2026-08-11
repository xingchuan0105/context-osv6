// osv7d: modular monolith entry — agentd + share + billing (+ projection).
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
	"github.com/context-os/osv7/internal/share"
	"github.com/context-os/osv7/internal/store"
)

type chatReq struct {
	Message     string `json:"message"`
	WorkspaceID string `json:"workspace_id,omitempty"`
	UserID      string `json:"user_id,omitempty"`
	SessionID   string `json:"session_id,omitempty"`
	Harness     *bool  `json:"harness,omitempty"`
	Timeout     int    `json:"timeout_sec,omitempty"`
	Persist     *bool  `json:"persist,omitempty"`
}

func main() {
	log.SetFlags(log.LstdFlags | log.Lmsgprefix)
	log.SetPrefix("osv7d: ")
	tryLoadEnv()

	addr := envOr("OSV7_ADDR", envOr("OSV7_AGENTD_ADDR", ":8090"))
	root := envOr("OSV7_ROOT", findRoot())

	ctx := context.Background()
	var pool *store.Pool
	if dsn := os.Getenv("DATABASE_URL"); dsn != "" {
		p, err := store.Connect(ctx, dsn)
		if err != nil {
			log.Fatalf("pg: %v", err)
		}
		pool = p
		defer pool.Close()
	} else {
		log.Fatal("DATABASE_URL required")
	}
	if err := pool.EnsureSessionSchema(ctx); err != nil {
		log.Fatal(err)
	}

	wallet := billing.NewWalletService(pool)
	if err := wallet.EnsureSchema(ctx); err != nil {
		log.Fatal(err)
	}
	shares := share.New(pool)
	if err := shares.EnsureSchema(ctx); err != nil {
		log.Fatal(err)
	}

	host := agentd.NewHost(wallet, agentd.Config{}).WithStore(pool).WithWallet(wallet)

	mux := http.NewServeMux()
	mux.HandleFunc("/healthz", func(w http.ResponseWriter, _ *http.Request) {
		_, _ = w.Write([]byte("ok"))
	})

	// --- chat ---
	mux.HandleFunc("/v1/chat", func(w http.ResponseWriter, r *http.Request) {
		if r.Method != http.MethodPost {
			http.Error(w, "POST only", 405)
			return
		}
		var req chatReq
		if err := json.NewDecoder(r.Body).Decode(&req); err != nil {
			http.Error(w, err.Error(), 400)
			return
		}
		res, err := runChat(r.Context(), host, root, req)
		if err != nil {
			writeErr(w, err)
			return
		}
		writeJSON(w, res)
	})
	mux.HandleFunc("/v1/chat/stream", func(w http.ResponseWriter, r *http.Request) {
		if r.Method != http.MethodPost {
			http.Error(w, "POST only", 405)
			return
		}
		var req chatReq
		if err := json.NewDecoder(r.Body).Decode(&req); err != nil {
			http.Error(w, err.Error(), 400)
			return
		}
		fl, ok := w.(http.Flusher)
		if !ok {
			http.Error(w, "no flush", 500)
			return
		}
		w.Header().Set("Content-Type", "text/event-stream")
		w.Header().Set("Cache-Control", "no-cache")
		writeSSE := func(ev string, v any) {
			b, _ := json.Marshal(v)
			fmt.Fprintf(w, "event: %s\ndata: %s\n\n", ev, b)
			fl.Flush()
		}
		res, err := runChat(r.Context(), host, root, req)
		// stream simplified: only final (full stream still via agentd OnEvent if wired)
		_ = req
		if err != nil {
			writeSSE("error", map[string]string{"error": err.Error()})
			return
		}
		writeSSE("result", res)
	})

	// --- sessions ---
	mux.HandleFunc("/v1/sessions", func(w http.ResponseWriter, r *http.Request) {
		if r.Method != http.MethodGet {
			http.Error(w, "GET only", 405)
			return
		}
		list, err := host.ListSessions(r.Context(), r.URL.Query().Get("user_id"), 50)
		if err != nil {
			http.Error(w, err.Error(), 500)
			return
		}
		if list == nil {
			list = []store.SessionRow{}
		}
		writeJSON(w, map[string]any{"sessions": list})
	})
	mux.HandleFunc("/v1/sessions/", func(w http.ResponseWriter, r *http.Request) {
		path := strings.TrimPrefix(r.URL.Path, "/v1/sessions/")
		parts := strings.Split(strings.Trim(path, "/"), "/")
		if len(parts) == 0 || parts[0] == "" {
			http.Error(w, "id required", 400)
			return
		}
		id := parts[0]
		if len(parts) >= 2 && parts[1] == "messages" {
			msgs, err := host.ListMessages(r.Context(), id, 500)
			if err != nil {
				http.Error(w, err.Error(), 500)
				return
			}
			if msgs == nil {
				msgs = []store.MessageRow{}
			}
			writeJSON(w, map[string]any{"messages": msgs})
			return
		}
		s, err := host.GetSession(r.Context(), id)
		if err != nil {
			http.Error(w, err.Error(), 500)
			return
		}
		if s == nil {
			http.Error(w, "not found", 404)
			return
		}
		writeJSON(w, s)
	})

	// --- billing ---
	mux.HandleFunc("/v1/billing/wallet", func(w http.ResponseWriter, r *http.Request) {
		uid := r.URL.Query().Get("user_id")
		if uid == "" {
			http.Error(w, "user_id required", 400)
			return
		}
		if r.Method == http.MethodGet {
			bal, err := wallet.Balance(r.Context(), uid)
			if err != nil {
				http.Error(w, err.Error(), 500)
				return
			}
			writeJSON(w, map[string]any{
				"user_id":     uid,
				"balance_fen": bal,
				"byok": map[string]bool{
					"embedding": wallet.HasBYOK(r.Context(), uid, "embedding"),
					"chat":      wallet.HasBYOK(r.Context(), uid, "chat"),
				},
			})
			return
		}
		http.Error(w, "GET only", 405)
	})
	mux.HandleFunc("/v1/billing/topup", func(w http.ResponseWriter, r *http.Request) {
		if r.Method != http.MethodPost {
			http.Error(w, "POST only", 405)
			return
		}
		var body struct {
			UserID    string `json:"user_id"`
			AmountFen int64  `json:"amount_fen"`
			Idem      string `json:"idempotency_key"`
		}
		if err := json.NewDecoder(r.Body).Decode(&body); err != nil {
			http.Error(w, err.Error(), 400)
			return
		}
		bal, err := wallet.TopUp(r.Context(), body.UserID, body.AmountFen, body.Idem)
		if err != nil {
			writeErr(w, err)
			return
		}
		writeJSON(w, map[string]any{"user_id": body.UserID, "balance_fen": bal})
	})
	mux.HandleFunc("/v1/billing/byok", func(w http.ResponseWriter, r *http.Request) {
		if r.Method != http.MethodPost {
			http.Error(w, "POST only", 405)
			return
		}
		var body struct {
			UserID     string `json:"user_id"`
			Capability string `json:"capability"`
			Enabled    bool   `json:"enabled"`
		}
		if err := json.NewDecoder(r.Body).Decode(&body); err != nil {
			http.Error(w, err.Error(), 400)
			return
		}
		if body.Capability == "" {
			body.Capability = "embedding"
		}
		if err := wallet.SetBYOK(r.Context(), body.UserID, body.Capability, body.Enabled); err != nil {
			http.Error(w, err.Error(), 500)
			return
		}
		writeJSON(w, map[string]any{"ok": true})
	})

	// --- share (authz stub: caller supplies owner_user_id) ---
	mux.HandleFunc("/v1/share", func(w http.ResponseWriter, r *http.Request) {
		if r.Method != http.MethodPost {
			http.Error(w, "POST only", 405)
			return
		}
		var body struct {
			WorkspaceID string `json:"workspace_id"`
			OwnerUserID string `json:"owner_user_id"`
			Title       string `json:"title"`
			TTLHours    int    `json:"ttl_hours"`
		}
		if err := json.NewDecoder(r.Body).Decode(&body); err != nil {
			http.Error(w, err.Error(), 400)
			return
		}
		ttl := time.Duration(body.TTLHours) * time.Hour
		if body.TTLHours == 0 {
			ttl = 72 * time.Hour
		}
		link, err := shares.Create(r.Context(), body.WorkspaceID, body.OwnerUserID, body.Title, ttl)
		if err != nil {
			http.Error(w, err.Error(), 500)
			return
		}
		writeJSON(w, map[string]any{
			"token":      link.Token,
			"public_url": "/public/s/" + link.Token,
			"link":       link,
		})
	})
	mux.HandleFunc("/public/s/", func(w http.ResponseWriter, r *http.Request) {
		if r.Method != http.MethodGet {
			http.Error(w, "GET only", 405)
			return
		}
		tok := strings.TrimPrefix(r.URL.Path, "/public/s/")
		tok = strings.Trim(tok, "/")
		view, err := shares.ResolvePublic(r.Context(), tok)
		if err != nil {
			http.Error(w, err.Error(), 403)
			return
		}
		if view == nil {
			http.Error(w, "not found", 404)
			return
		}
		// ETag / 304
		if inm := r.Header.Get("If-None-Match"); inm != "" && inm == view.ETag {
			w.WriteHeader(http.StatusNotModified)
			return
		}
		w.Header().Set("ETag", view.ETag)
		w.Header().Set("Cache-Control", "public, max-age=30")
		writeJSON(w, view)
	})

	log.Printf("listening on %s root=%s", addr, root)
	log.Fatal(http.ListenAndServe(addr, mux))
}

func runChat(ctx context.Context, host *agentd.Host, root string, req chatReq) (*agentd.ChatResult, error) {
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
	persist := true
	if req.Persist != nil {
		persist = *req.Persist
	}
	return host.ChatOneShot(ctx, agentd.ChatOptions{
		Message:           req.Message,
		Timeout:           timeout,
		Cwd:               root,
		WorkspaceID:       req.WorkspaceID,
		UserID:            req.UserID,
		ProductSessionID:  req.SessionID,
		EnableHarness:     harness,
		RetrievalCLI:      filepath.Join(root, "bin", "retrieval-cli"),
		PersistProjection: persist,
	})
}

func writeJSON(w http.ResponseWriter, v any) {
	w.Header().Set("Content-Type", "application/json")
	enc := json.NewEncoder(w)
	enc.SetIndent("", "  ")
	_ = enc.Encode(v)
}

func writeErr(w http.ResponseWriter, err error) {
	if fe, ok := err.(billing.FloorError); ok {
		w.Header().Set("Content-Type", "application/json")
		w.WriteHeader(http.StatusPaymentRequired)
		_ = json.NewEncoder(w).Encode(map[string]any{
			"error":       "balance_insufficient",
			"fact":        fe.Fact,
			"remediation": fe.Remediation,
			"balance_fen": fe.BalanceFen,
		})
		return
	}
	http.Error(w, err.Error(), http.StatusBadGateway)
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
		for _, line := range strings.Split(string(b), "\n") {
			line = strings.TrimSpace(line)
			if line == "" || strings.HasPrefix(line, "#") {
				continue
			}
			i := strings.IndexByte(line, '=')
			if i <= 0 {
				continue
			}
			k, v := line[:i], strings.Trim(line[i+1:], `"'`)
			if os.Getenv(k) == "" {
				_ = os.Setenv(k, v)
			}
		}
		return
	}
}
