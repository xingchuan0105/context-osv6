package retrieval

import (
	"context"
	"encoding/json"
	"fmt"

	"github.com/context-os/osv7/internal/billing"
	"github.com/context-os/osv7/internal/index"
	"github.com/context-os/osv7/internal/store"
)

// Service is the retrieval harness (MCP-agnostic).
type Service struct {
	store   *store.Pool
	index   *index.Index
	billing billing.Surface
	sess    *Session
	userID  string // optional default owner filter
}

func NewService(st *store.Pool, ix *index.Index, bill billing.Surface, userID string) *Service {
	return &Service{
		store:   st,
		index:   ix,
		billing: bill,
		sess:    NewSession(),
		userID:  userID,
	}
}

func (s *Service) Session() *Session { return s.sess }

// SetQueryCard applies resource + shape gates then installs the card.
func (s *Service) SetQueryCard(ctx context.Context, card QueryCard) (map[string]any, error) {
	if err := card.ValidateShape(); err != nil {
		return nil, gateErr(errResource(
			fmt.Sprintf("题卡形状不合法：%v", err),
			"修正题卡字段后重新 set_query_card。",
			map[string]any{"reason": err.Error()},
		))
	}
	ok, err := s.store.WorkspaceExists(ctx, card.WorkspaceID)
	if err != nil {
		return nil, err
	}
	if !ok {
		return nil, gateErr(errResource(
			fmt.Sprintf("workspace_id=%s 在 workspaces 与 rag_text_chunks 中均未出现。", card.WorkspaceID),
			"改用存在语料的 workspace_id，或先完成摄入。",
			map[string]any{"workspace_id": card.WorkspaceID},
		))
	}
	n, err := s.store.WorkspaceChunkCount(ctx, card.WorkspaceID)
	if err != nil {
		return nil, err
	}
	s.sess.SetCard(card)
	s.billing.Record(ctx, billing.UsageEvent{Tool: "set_query_card", Kind: "gate", Units: 1, UserID: s.userID})
	return map[string]any{
		"ok":            true,
		"card":          card,
		"chunk_count":   n,
		"capabilities":  s.billing.Capabilities(ctx),
		"session_after": s.sess.Status(),
	}, nil
}

func (s *Service) requireCard() (*QueryCard, error) {
	c := s.sess.GetCard()
	if c == nil {
		return nil, gateErr(errCardMissing())
	}
	return c, nil
}

func (s *Service) searchOpts(card *QueryCard, limit int) index.SearchOpts {
	return index.SearchOpts{
		WorkspaceID: card.WorkspaceID,
		OwnerUserID: s.userID,
		DocIDs:      card.DocIDs,
		Limit:       limit,
	}
}

func (s *Service) Lexical(ctx context.Context, query string, limit int) (map[string]any, error) {
	card, err := s.requireCard()
	if err != nil {
		return nil, err
	}
	hits, err := s.index.Lexical(ctx, query, s.searchOpts(card, limit))
	if err != nil {
		return nil, err
	}
	handles := s.sess.IngestHits(hits)
	s.sess.MarkOk("lexical")
	s.billing.Record(ctx, billing.UsageEvent{Tool: "lexical", Kind: "search", Units: len(hits), UserID: s.userID})
	return s.packHits("lexical", query, handles), nil
}

func (s *Service) Grep(ctx context.Context, pattern string, limit int) (map[string]any, error) {
	card, err := s.requireCard()
	if err != nil {
		return nil, err
	}
	hits, err := s.index.Grep(ctx, pattern, s.searchOpts(card, limit))
	if err != nil {
		return nil, err
	}
	handles := s.sess.IngestHits(hits)
	s.sess.MarkOk("grep")
	s.billing.Record(ctx, billing.UsageEvent{Tool: "grep", Kind: "search", Units: len(hits), UserID: s.userID})
	return s.packHits("grep", pattern, handles), nil
}

func (s *Service) Dense(ctx context.Context, query string, limit int) (map[string]any, error) {
	card, err := s.requireCard()
	if err != nil {
		return nil, err
	}
	caps := s.billing.Capabilities(ctx)
	if caps.Embedding == billing.Missing {
		return nil, gateErr(errCapability(
			"embedding",
			"本运行时未配置 embedding（EMBEDDING_BASE_URL / EMBEDDING_API_KEY）。",
			"配置平台 embedding 或 BYOK 后重试 dense。",
		))
	}
	hits, err := s.index.Dense(ctx, query, s.searchOpts(card, limit))
	if err != nil {
		return nil, err
	}
	handles := s.sess.IngestHits(hits)
	s.sess.MarkOk("dense")
	s.billing.Record(ctx, billing.UsageEvent{Tool: "dense", Kind: "embed", Units: 1, UserID: s.userID, Detail: "query_embed"})
	s.billing.Record(ctx, billing.UsageEvent{Tool: "dense", Kind: "search", Units: len(hits), UserID: s.userID})
	return s.packHits("dense", query, handles), nil
}

func (s *Service) Select(aliases []string) (map[string]any, error) {
	if _, err := s.requireCard(); err != nil {
		return nil, err
	}
	ok, unknown := s.sess.Select(aliases)
	if len(unknown) > 0 {
		return nil, gateErr(errContract(
			"SELECTED 引用了未知句柄。",
			map[string]any{"unknown": unknown, "selected": ok},
		))
	}
	return map[string]any{"selected": ok, "status": s.sess.Status()}, nil
}

func (s *Service) Keep(aliases []string) (map[string]any, error) {
	if _, err := s.requireCard(); err != nil {
		return nil, err
	}
	kept, unknown := s.sess.Keep(aliases)
	if len(unknown) > 0 {
		return nil, gateErr(errContract(
			"KEEP 引用了未知句柄。",
			map[string]any{"unknown": unknown, "kept": kept},
		))
	}
	return map[string]any{"kept": kept, "status": s.sess.Status()}, nil
}

func (s *Service) VerifyDraft(draft string, requireSelected bool) (map[string]any, error) {
	if _, err := s.requireCard(); err != nil {
		return nil, err
	}
	// Contract gate: required actions must be Ok before verify is meaningful for handoff.
	if miss := s.sess.MissingRequired(); len(miss) > 0 {
		return nil, gateErr(errContract(
			"题卡声明的 required_actions 仍有未 Ok 回传的动作。",
			map[string]any{"missing_required": miss},
		))
	}
	valid, invalid := s.sess.VerifyDraft(draft, requireSelected)
	pass := len(invalid) == 0
	return map[string]any{
		"pass":             pass,
		"valid_aliases":    valid,
		"invalid_aliases":  invalid,
		"require_selected": requireSelected,
		"status":           s.sess.Status(),
	}, nil
}

func (s *Service) Status() map[string]any {
	st := s.sess.Status()
	st["capabilities"] = s.billing.Capabilities(context.Background())
	return st
}

func (s *Service) packHits(tool, query string, handles []Handle) map[string]any {
	// Public view without full Hit blob
	type pub struct {
		Alias   string  `json:"alias"`
		ChunkID string  `json:"chunk_id"`
		DocID   string  `json:"doc_id"`
		Snippet string  `json:"snippet,omitempty"`
		Score   float64 `json:"score"`
		Backend string  `json:"backend"`
		Reseen  string  `json:"reseen,omitempty"`
	}
	list := make([]pub, 0, len(handles))
	for _, h := range handles {
		list = append(list, pub{
			Alias: h.Alias, ChunkID: h.ChunkID, DocID: h.DocID,
			Snippet: h.Snippet, Score: h.Score, Backend: h.Backend, Reseen: h.Reseen,
		})
	}
	return map[string]any{
		"tool":             tool,
		"query":            query,
		"total_hits":       len(list),
		"handles":          list,
		"missing_required": s.sess.MissingRequired(),
		"status":           s.sess.Status(),
	}
}

// gateErr wraps ErrorBody so MCP layer can IsError=true with JSON body.
type GateError struct {
	Body ErrorBody
}

func (e GateError) Error() string { return e.Body.Error + ": " + e.Body.Fact }

func gateErr(b ErrorBody) error { return GateError{Body: b} }

func AsGate(err error) (ErrorBody, bool) {
	if g, ok := err.(GateError); ok {
		return g.Body, true
	}
	return ErrorBody{}, false
}

func MustJSON(v any) string {
	b, err := json.MarshalIndent(v, "", "  ")
	if err != nil {
		return fmt.Sprintf("%v", v)
	}
	return string(b)
}
