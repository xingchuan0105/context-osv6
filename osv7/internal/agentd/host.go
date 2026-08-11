package agentd

import (
	"context"
	"fmt"
	"os"
	"path/filepath"
	"strings"
	"time"

	"github.com/context-os/osv7/internal/billing"
	"github.com/context-os/osv7/internal/store"
)

// Host is the product-facing agentd surface: pi sessions + gate + usage + PG projection.
type Host struct {
	cfg     Config
	billing billing.Surface
	wallet  *billing.WalletService
	store   *store.Pool // optional projection
}

// NewHost builds agentd with defaults suitable for deepseek-backed pi.
func NewHost(bill billing.Surface, cfg Config) *Host {
	if cfg.PiBin == "" {
		cfg.PiBin = envOr("PI_BIN", "pi")
	}
	if cfg.Provider == "" {
		cfg.Provider = envOr("OSV7_PI_PROVIDER", "deepseek")
	}
	if cfg.Model == "" {
		cfg.Model = envOr("OSV7_PI_MODEL", "deepseek-v4-flash")
	}
	return &Host{cfg: cfg, billing: bill}
}

// WithStore enables PG message/session projection.
func (h *Host) WithStore(p *store.Pool) *Host {
	h.store = p
	return h
}

// WithWallet enables balance floor + usage debit on chat turns.
func (h *Host) WithWallet(w *billing.WalletService) *Host {
	h.wallet = w
	return h
}

// ChatOptions for a one-shot or multi-turn attachment.
type ChatOptions struct {
	Message            string
	SessionDir         string
	Timeout            time.Duration
	Cwd                string
	NoExtensions       bool
	WorkspaceID        string
	UserID             string
	ProductSessionID   string // osv7_sessions.id; empty = create new when store set
	PiSessionFile      string // resume pi transcript
	AppendSystemPrompt string
	EnableHarness      bool
	RetrievalCLI       string
	RetrievalState     string
	OnEvent            EventHandler
	// PersistProjection writes user/assistant bubbles to PG when store set.
	PersistProjection bool
}

// ChatResult is returned to API / CLI after gate.
type ChatResult struct {
	Answer           string         `json:"answer"`
	Blocked          bool           `json:"blocked"`
	GateReason       string         `json:"gate_reason,omitempty"`
	Usage            map[string]any `json:"usage,omitempty"`
	Tools            []ToolCall     `json:"tools,omitempty"`
	Events           int            `json:"events"`
	DurationMs       int64          `json:"duration_ms"`
	SessionID        string         `json:"session_id,omitempty"` // pi session id
	ProductSessionID string         `json:"product_session_id,omitempty"`
	PiSessionFile    string         `json:"pi_session_file,omitempty"`
	Workspace        string         `json:"workspace_id,omitempty"`
	// CardKeeper soft signals (always present when harness was enabled).
	HarnessEnabled   bool     `json:"harness_enabled,omitempty"`
	HarnessToolsUsed []string `json:"harness_tools_used,omitempty"`
	CardMissing      *bool    `json:"card_missing,omitempty"`      // no set_query_card when harness on
	RetrievalInvoked *bool    `json:"retrieval_invoked,omitempty"` // lexical|dense|grep ran
	CardObservation  string   `json:"card_observation,omitempty"`  // third-person fact for UI/telemetry
	Raw              string   `json:"-"`
}

// ChatOneShot starts a pi process, one prompt, closes.
func (h *Host) ChatOneShot(ctx context.Context, opt ChatOptions) (*ChatResult, error) {
	if opt.Message == "" {
		return nil, fmt.Errorf("empty message")
	}
	if opt.Timeout <= 0 {
		opt.Timeout = 3 * time.Minute
	}
	ctx, cancel := context.WithTimeout(ctx, opt.Timeout)
	defer cancel()

	// Product session projection
	var productSessionID string
	if opt.PersistProjection && h.store != nil {
		var err error
		productSessionID, err = h.ensureProductSession(ctx, opt)
		if err != nil {
			return nil, err
		}
		// load pi path if resuming
		if opt.PiSessionFile == "" && productSessionID != "" {
			if s, err := h.store.GetSession(ctx, productSessionID); err == nil && s != nil {
				opt.PiSessionFile = s.PiSessionFile
				if opt.WorkspaceID == "" {
					opt.WorkspaceID = s.WorkspaceID
				}
			}
		}
	}

	cfg := h.cfg
	if opt.PiSessionFile != "" {
		cfg.SessionFile = opt.PiSessionFile
		cfg.NoSession = false
		if opt.SessionDir == "" {
			cfg.SessionDir = filepath.Dir(opt.PiSessionFile)
		} else {
			cfg.SessionDir = opt.SessionDir
		}
	} else if opt.SessionDir != "" {
		cfg.SessionDir = opt.SessionDir
		cfg.NoSession = false
		_ = os.MkdirAll(opt.SessionDir, 0o755)
	} else if opt.PersistProjection && h.store != nil {
		// durable pi transcript under data/sessions
		dir := filepath.Join(envOr("OSV7_SESSION_DIR", filepath.Join(os.TempDir(), "osv7-pi-sessions")))
		_ = os.MkdirAll(dir, 0o755)
		cfg.SessionDir = dir
		cfg.NoSession = false
	} else {
		cfg.NoSession = true
	}
	if opt.Cwd != "" {
		cfg.Cwd = opt.Cwd
	}

	// Ensure deepseek key visible to pi
	if os.Getenv("DEEPSEEK_API_KEY") == "" {
		if k := os.Getenv("CHAT_LLM_API_KEY"); k != "" {
			cfg.Env = append(cfg.Env, "DEEPSEEK_API_KEY="+k)
		}
		if k := os.Getenv("AGENT_LLM_API_KEY"); k != "" {
			cfg.Env = append(cfg.Env, "DEEPSEEK_API_KEY="+k)
		}
	}

	if opt.EnableHarness {
		cwd := cfg.Cwd
		if cwd == "" {
			cwd, _ = os.Getwd()
		}
		cfg.ExtraArgs = append(cfg.ExtraArgs, "--no-extensions")
		ext := filepath.Join(cwd, ".pi", "extensions", "osv7-harness.ts")
		if _, err := os.Stat(ext); err == nil {
			cfg.Extensions = append(cfg.Extensions, ext)
		}
		cli := opt.RetrievalCLI
		if cli == "" {
			cli = filepath.Join(cwd, "bin", "retrieval-cli")
		}
		cfg.Env = append(cfg.Env, "OSV7_RETRIEVAL_CLI="+cli)
		if opt.WorkspaceID != "" {
			cfg.Env = append(cfg.Env, "OSV7_WORKSPACE_ID="+opt.WorkspaceID)
		}
		state := opt.RetrievalState
		if state == "" {
			state = filepath.Join(os.TempDir(), fmt.Sprintf("osv7-retrieval-%d.json", time.Now().UnixNano()))
		}
		cfg.Env = append(cfg.Env, "OSV7_RETRIEVAL_STATE="+state)
		_ = os.Remove(state)

		appendPath := opt.AppendSystemPrompt
		if appendPath == "" {
			appendPath = filepath.Join(cwd, "prompts", "agentd-harness-append.md")
		}
		if _, err := os.Stat(appendPath); err == nil {
			cfg.AppendSystemPrompt = appendPath
		}
	} else {
		if opt.NoExtensions || len(cfg.Extensions) == 0 {
			cfg.ExtraArgs = append(cfg.ExtraArgs, "--no-extensions")
		}
		if opt.AppendSystemPrompt != "" {
			cfg.AppendSystemPrompt = opt.AppendSystemPrompt
		}
	}

	if opt.PersistProjection && h.store != nil && productSessionID != "" {
		_, _ = h.store.AppendMessage(ctx, productSessionID, "user", opt.Message, nil, nil)
		_ = h.store.TouchSession(ctx, productSessionID, store.TitleFromMessage(opt.Message))
	}

	// Billing floor before LLM
	if h.wallet != nil && opt.UserID != "" {
		if err := h.wallet.EnsureFloor(ctx, opt.UserID, "chat", billing.PriceChatTurnFen); err != nil {
			return nil, err
		}
	}

	start := time.Now()
	sess, err := Start(ctx, cfg)
	if err != nil {
		return nil, err
	}
	defer sess.Close()

	msg := opt.Message
	if opt.EnableHarness && opt.WorkspaceID != "" {
		msg = fmt.Sprintf("【环境】当前检索 workspace_id=%s\n\n%s", opt.WorkspaceID, opt.Message)
	}

	turn, err := sess.Prompt(ctx, msg, opt.OnEvent)
	if err != nil {
		return nil, err
	}

	var sessionID, piFile string
	if st, err := sess.GetState(ctx); err == nil && st != nil {
		if id, ok := st["sessionId"].(string); ok {
			sessionID = id
		}
		if f, ok := st["sessionFile"].(string); ok {
			piFile = f
		}
	}

	if opt.PersistProjection && h.store != nil && productSessionID != "" {
		if piFile != "" {
			_ = h.store.SetPiSessionFile(ctx, productSessionID, piFile)
		}
		meta := map[string]any{
			"gate_blocked": turn.Gated.Blocked,
			"gate_reason":  turn.Gated.Reason,
		}
		_, _ = h.store.AppendMessage(ctx, productSessionID, "assistant", turn.UserText, turn.ToolCalls, meta)
	}

	if h.billing != nil && turn.Usage != nil {
		units := 1
		if in, ok := asInt(turn.Usage["input"]); ok {
			units = in
		}
		h.billing.Record(ctx, billing.UsageEvent{
			Tool:   "agentd.chat",
			Kind:   "llm",
			Units:  units,
			UserID: opt.UserID,
			Detail: fmt.Sprintf("provider=%s model=%s", cfg.Provider, cfg.Model),
		})
	}
	if h.wallet != nil && opt.UserID != "" {
		_, _ = h.wallet.DebitUsage(ctx, opt.UserID, "chat", billing.PriceChatTurnFen,
			fmt.Sprintf("chat:%s:%d", productSessionID, start.UnixNano()), "agentd.chat")
	}

	res := &ChatResult{
		Answer:           turn.UserText,
		Blocked:          turn.Gated.Blocked,
		GateReason:       turn.Gated.Reason,
		Usage:            turn.Usage,
		Tools:            turn.ToolCalls,
		Events:           turn.RawEventCount,
		DurationMs:       time.Since(start).Milliseconds(),
		SessionID:        sessionID,
		ProductSessionID: productSessionID,
		PiSessionFile:    piFile,
		Workspace:        opt.WorkspaceID,
		Raw:              turn.AssistantRaw,
	}
	applyCardKeeper(res, opt.EnableHarness)
	return res, nil
}

func (h *Host) ensureProductSession(ctx context.Context, opt ChatOptions) (string, error) {
	if opt.ProductSessionID != "" {
		s, err := h.store.GetSession(ctx, opt.ProductSessionID)
		if err != nil {
			return "", err
		}
		if s == nil {
			return "", fmt.Errorf("session not found: %s", opt.ProductSessionID)
		}
		return s.ID, nil
	}
	s, err := h.store.CreateSession(ctx, opt.UserID, opt.WorkspaceID, store.TitleFromMessage(opt.Message))
	if err != nil {
		return "", err
	}
	return s.ID, nil
}

// applyCardKeeper fills soft card-keeper signals (third-person facts, no hard block here).
func applyCardKeeper(res *ChatResult, harness bool) {
	if !harness {
		return
	}
	res.HarnessEnabled = true
	names := make([]string, 0, len(res.Tools))
	set := map[string]bool{}
	for _, t := range res.Tools {
		names = append(names, t.Name)
		set[t.Name] = true
	}
	res.HarnessToolsUsed = names
	cm := !set["set_query_card"]
	ri := set["lexical"] || set["dense"] || set["grep"]
	res.CardMissing = &cm
	res.RetrievalInvoked = &ri
	var facts []string
	if cm {
		facts = append(facts, "本轮未观察到 set_query_card 调用；题卡未安装时 harness 原语会拒绝检索。")
	}
	if !ri {
		facts = append(facts, "本轮未观察到 lexical/dense/grep 成功调用记录。")
	}
	if cm || !ri {
		res.CardObservation = strings.Join(facts, " ")
	}
}

// ListSessions / ListMessages proxy store.
func (h *Host) ListSessions(ctx context.Context, userID string, limit int) ([]store.SessionRow, error) {
	if h.store == nil {
		return nil, fmt.Errorf("store not configured")
	}
	return h.store.ListSessions(ctx, userID, limit)
}

func (h *Host) ListMessages(ctx context.Context, sessionID string, limit int) ([]store.MessageRow, error) {
	if h.store == nil {
		return nil, fmt.Errorf("store not configured")
	}
	return h.store.ListMessages(ctx, sessionID, limit)
}

func (h *Host) GetSession(ctx context.Context, id string) (*store.SessionRow, error) {
	if h.store == nil {
		return nil, fmt.Errorf("store not configured")
	}
	return h.store.GetSession(ctx, id)
}

// EnsureMcpConfig writes a minimal .mcp.json in dir pointing at retrieval-mcp if missing.
func EnsureMcpConfig(dir, retrievalMCPBin string) error {
	path := filepath.Join(dir, ".mcp.json")
	if _, err := os.Stat(path); err == nil {
		return nil
	}
	body := fmt.Sprintf(`{
  "mcpServers": {
    "osv7-retrieval": {
      "command": %q,
      "args": []
    }
  }
}
`, retrievalMCPBin)
	return os.WriteFile(path, []byte(body), 0o644)
}

func envOr(k, def string) string {
	if v := os.Getenv(k); v != "" {
		return v
	}
	return def
}

func asInt(v any) (int, bool) {
	switch n := v.(type) {
	case float64:
		return int(n), true
	case int:
		return n, true
	case int64:
		return int(n), true
	default:
		return 0, false
	}
}

