package agentd

import (
	"bufio"
	"context"
	"encoding/json"
	"fmt"
	"io"
	"os"
	"os/exec"
	"strings"
	"sync"
	"time"
)

// Config for a pi RPC subprocess.
type Config struct {
	PiBin      string   // default "pi"
	Provider   string   // e.g. deepseek
	Model      string   // e.g. deepseek-v4-flash
	SessionDir string   // optional
	SessionFile string  // resume existing transcript (--session path)
	NoSession  bool     // ephemeral
	Cwd        string   // working directory for pi
	ExtraArgs  []string // e.g. -e path, --no-extensions
	Env        []string // extra env KEY=VAL; process env is always inherited
	// AppendSystemPrompt is passed as --append-system-prompt (file path or text).
	AppendSystemPrompt string
	// Extension paths: -e for each
	Extensions []string
}

// Event is a relayed pi event for SSE / activity panels.
type Event struct {
	Type    string         `json:"type"`
	Payload map[string]any `json:"payload,omitempty"`
}

// EventHandler receives stream events (tool start, text delta, etc.).
type EventHandler func(Event)

// Session is one pi --mode rpc process (one logical chat attachment).
type Session struct {
	cfg Config
	cmd *exec.Cmd
	stdin io.WriteCloser
	stdout *bufio.Reader
	stderr io.ReadCloser

	mu     sync.Mutex
	nextID int
}

// Start launches pi RPC.
func Start(ctx context.Context, cfg Config) (*Session, error) {
	if cfg.PiBin == "" {
		cfg.PiBin = "pi"
	}
	args := []string{"--mode", "rpc"}
	if cfg.Provider != "" {
		args = append(args, "--provider", cfg.Provider)
	}
	if cfg.Model != "" {
		args = append(args, "--model", cfg.Model)
	}
	if cfg.NoSession {
		args = append(args, "--no-session")
	}
	if cfg.SessionDir != "" {
		args = append(args, "--session-dir", cfg.SessionDir)
	}
	if cfg.SessionFile != "" {
		args = append(args, "--session", cfg.SessionFile)
	}
	if cfg.AppendSystemPrompt != "" {
		args = append(args, "--append-system-prompt", cfg.AppendSystemPrompt)
	}
	for _, e := range cfg.Extensions {
		if e != "" {
			args = append(args, "-e", e)
		}
	}
	args = append(args, cfg.ExtraArgs...)

	cmd := exec.CommandContext(ctx, cfg.PiBin, args...)
	if cfg.Cwd != "" {
		cmd.Dir = cfg.Cwd
	}
	cmd.Env = append(os.Environ(), cfg.Env...)

	stdin, err := cmd.StdinPipe()
	if err != nil {
		return nil, err
	}
	stdout, err := cmd.StdoutPipe()
	if err != nil {
		return nil, err
	}
	stderr, err := cmd.StderrPipe()
	if err != nil {
		return nil, err
	}
	if err := cmd.Start(); err != nil {
		return nil, fmt.Errorf("start pi: %w", err)
	}

	// Drain stderr to log-like sink (must not block)
	go func() {
		sc := bufio.NewScanner(stderr)
		for sc.Scan() {
			// keep quiet unless debugging; user can set OSV7_AGENTD_DEBUG
			if os.Getenv("OSV7_AGENTD_DEBUG") != "" {
				fmt.Fprintln(os.Stderr, "pi-stderr:", sc.Text())
			}
		}
	}()

	return &Session{
		cfg:    cfg,
		cmd:    cmd,
		stdin:  stdin,
		stdout: bufio.NewReader(stdout),
		stderr: stderr,
		nextID: 1,
	}, nil
}

// Close terminates the pi process.
func (s *Session) Close() error {
	s.mu.Lock()
	defer s.mu.Unlock()
	_ = s.stdin.Close()
	done := make(chan error, 1)
	go func() { done <- s.cmd.Wait() }()
	select {
	case err := <-done:
		return err
	case <-time.After(3 * time.Second):
		_ = s.cmd.Process.Kill()
		return <-done
	}
}

// Prompt sends a user prompt and waits until agent_settled (or agent_end if settled missing).
// onEvent is optional (SSE activity / deltas). Final user text is still gated.
func (s *Session) Prompt(ctx context.Context, message string, onEvent EventHandler) (*TurnResult, error) {
	id := s.allocID()
	if err := s.send(map[string]any{
		"id":      id,
		"type":    "prompt",
		"message": message,
	}); err != nil {
		return nil, err
	}

	res := &TurnResult{}
	var textBuilder strings.Builder
	settled := false
	emit := func(ev Event) {
		if onEvent != nil {
			onEvent(ev)
		}
	}

	for {
		select {
		case <-ctx.Done():
			_ = s.send(map[string]any{"type": "abort"})
			return res, ctx.Err()
		default:
		}

		line, err := s.readLine(ctx)
		if err != nil {
			return res, err
		}
		if line == "" {
			continue
		}
		var ev map[string]any
		if err := json.Unmarshal([]byte(line), &ev); err != nil {
			continue
		}
		res.RawEventCount++
		etype := fmt.Sprint(ev["type"])

		// command response for prompt
		if etype == "response" && fmt.Sprint(ev["id"]) == id {
			if ok, _ := ev["success"].(bool); !ok {
				return res, fmt.Errorf("prompt rejected: %v", ev["error"])
			}
			continue
		}

		switch etype {
		case "message_update":
			ame, _ := ev["assistantMessageEvent"].(map[string]any)
			if ame == nil {
				continue
			}
			if ame["type"] == "text_delta" {
				if d, ok := ame["delta"].(string); ok {
					textBuilder.WriteString(d)
					res.StreamText += d
					emit(Event{Type: "delta", Payload: map[string]any{"text": d}})
				}
			}
		case "message_end":
			msg, _ := ev["message"].(map[string]any)
			if msg == nil {
				continue
			}
			if role, _ := msg["role"].(string); role == "assistant" {
				if t := extractAssistantText(msg); t != "" {
					res.AssistantRaw = t
				}
				if u, ok := msg["usage"].(map[string]any); ok {
					res.Usage = mergeUsage(res.Usage, u)
				}
			}
		case "tool_execution_start":
			tc := ToolCall{
				Name: fmt.Sprint(ev["toolName"]),
				ID:   fmt.Sprint(ev["toolCallId"]),
			}
			res.ToolCalls = append(res.ToolCalls, tc)
			emit(Event{Type: "tool_start", Payload: map[string]any{
				"name": tc.Name, "id": tc.ID, "args": ev["args"],
			}})
		case "tool_execution_end":
			res.ToolResults++
			emit(Event{Type: "tool_end", Payload: map[string]any{
				"name": fmt.Sprint(ev["toolName"]),
				"id":   fmt.Sprint(ev["toolCallId"]),
				"error": ev["isError"],
			}})
		case "agent_settled":
			settled = true
		case "agent_end":
			if !settled {
				if wr, _ := ev["willRetry"].(bool); !wr {
					settled = true
				}
			}
		case "extension_error":
			res.ExtensionErrors = append(res.ExtensionErrors, fmt.Sprint(ev["error"]))
			emit(Event{Type: "extension_error", Payload: map[string]any{"error": ev["error"]}})
		}

		if settled {
			break
		}
	}

	if res.AssistantRaw == "" {
		res.AssistantRaw = textBuilder.String()
	}
	res.Gated = FilterOutbound(res.AssistantRaw)
	res.UserText = res.Gated.Text
	emit(Event{Type: "done", Payload: map[string]any{
		"answer":  res.UserText,
		"blocked": res.Gated.Blocked,
		"reason":  res.Gated.Reason,
	}})
	return res, nil
}

// GetState issues get_state.
func (s *Session) GetState(ctx context.Context) (map[string]any, error) {
	id := s.allocID()
	if err := s.send(map[string]any{"id": id, "type": "get_state"}); err != nil {
		return nil, err
	}
	for {
		line, err := s.readLine(ctx)
		if err != nil {
			return nil, err
		}
		var ev map[string]any
		if err := json.Unmarshal([]byte(line), &ev); err != nil {
			continue
		}
		if ev["type"] == "response" && fmt.Sprint(ev["id"]) == id {
			if ok, _ := ev["success"].(bool); !ok {
				return nil, fmt.Errorf("get_state failed: %v", ev["error"])
			}
			data, _ := ev["data"].(map[string]any)
			return data, nil
		}
	}
}

func (s *Session) allocID() string {
	s.mu.Lock()
	defer s.mu.Unlock()
	id := fmt.Sprintf("r%d", s.nextID)
	s.nextID++
	return id
}

func (s *Session) send(obj map[string]any) error {
	b, err := json.Marshal(obj)
	if err != nil {
		return err
	}
	s.mu.Lock()
	defer s.mu.Unlock()
	_, err = s.stdin.Write(append(b, '\n'))
	return err
}

func (s *Session) readLine(ctx context.Context) (string, error) {
	type result struct {
		line string
		err  error
	}
	ch := make(chan result, 1)
	go func() {
		line, err := s.stdout.ReadString('\n')
		ch <- result{line: strings.TrimRight(line, "\r\n"), err: err}
	}()
	select {
	case <-ctx.Done():
		return "", ctx.Err()
	case r := <-ch:
		if r.err == io.EOF {
			return r.line, io.EOF
		}
		return r.line, r.err
	}
}

// TurnResult is one agent turn after gate.
type TurnResult struct {
	UserText        string         `json:"user_text"` // gated
	AssistantRaw    string         `json:"assistant_raw,omitempty"`
	StreamText      string         `json:"-"`
	Gated           GateResult     `json:"gate"`
	Usage           map[string]any `json:"usage,omitempty"`
	ToolCalls       []ToolCall     `json:"tool_calls,omitempty"`
	ToolResults     int            `json:"tool_results"`
	RawEventCount   int            `json:"raw_event_count"`
	ExtensionErrors []string       `json:"extension_errors,omitempty"`
}

type ToolCall struct {
	Name string `json:"name"`
	ID   string `json:"id,omitempty"`
}

func extractAssistantText(msg map[string]any) string {
	// content may be string or array of blocks
	switch c := msg["content"].(type) {
	case string:
		return c
	case []any:
		var b strings.Builder
		for _, part := range c {
			m, ok := part.(map[string]any)
			if !ok {
				continue
			}
			if m["type"] == "text" {
				if t, ok := m["text"].(string); ok {
					b.WriteString(t)
				}
			}
		}
		return b.String()
	}
	return ""
}

func mergeUsage(dst, src map[string]any) map[string]any {
	if dst == nil {
		dst = map[string]any{}
	}
	for k, v := range src {
		dst[k] = v
	}
	return dst
}
