package ingest

import (
	"context"
	"encoding/json"
	"fmt"
	"os"
	"path/filepath"
	"strings"
	"sync"
	"time"
	"unicode/utf8"

	"github.com/context-os/osv7/internal/billing"
	"github.com/context-os/osv7/internal/index"
	"github.com/context-os/osv7/internal/store"
	"github.com/google/uuid"
	"github.com/pgvector/pgvector-go"
)

// Service is the ingest harness (MCP/CLI agnostic).
type Service struct {
	store   *store.Pool
	emb     *index.Embedder
	billing billing.Surface
	wallet  *billing.WalletService

	mu       sync.Mutex
	sessions map[string]*Session // doc_id → pending
}

// Session is one in-flight ingest before commit.
type Session struct {
	DocID       string
	WorkspaceID string
	// OwnerUserID is product billing id (text). OwnerUUID is rag_text_chunks.owner_user_id.
	OwnerUserID string
	OwnerUUID   string
	Title       string
	Source      string // agent_package | server_parse
	CreatedAt   time.Time
	IR          *DocumentIr
	Status      string // open|committed|failed
	ParseRunID  string
}

// NewService constructs ingest service.
func NewService(st *store.Pool, emb *index.Embedder, bill billing.Surface) *Service {
	return &Service{
		store:    st,
		emb:      emb,
		billing:  bill,
		sessions: map[string]*Session{},
	}
}

// WithWallet enables balance floor + per-chunk debit on commit.
func (s *Service) WithWallet(w *billing.WalletService) *Service {
	s.wallet = w
	return s
}

// Preflight returns capability table + can_ingest flag.
func (s *Service) Preflight(ctx context.Context) map[string]any {
	caps := Capabilities{
		Embedding: "missing",
		Rerank:    "missing",
		OCR:       "missing",
	}
	if s.emb != nil && s.emb.Enabled() {
		caps.Embedding = "hosted"
	}
	if s.billing != nil {
		snap := s.billing.Capabilities(ctx)
		if snap.Embedding != "" {
			caps.Embedding = string(snap.Embedding)
		}
	}
	can := caps.Embedding == "hosted" || caps.Embedding == "byok"
	return map[string]any{
		"schema_version": SchemaVersion,
		"capabilities":   caps,
		"can_ingest":     can,
		"fact": func() string {
			if can {
				return "embedding 能力可用；可发起 ingest_begin。"
			}
			return "embedding 能力缺失；平台未配置或余额不足时将拒绝摄入。"
		}(),
	}
}

// BeginInput starts an ingest session.
type BeginInput struct {
	WorkspaceID string `json:"workspace_id"`
	OwnerUserID string `json:"owner_user_id,omitempty"`
	Title       string `json:"title"`
	Source      string `json:"source,omitempty"` // agent_package | server_parse
	// Sniff: optional content sample for scan detection (PDF header etc.)
	ContentSample []byte `json:"-"`
	Filename      string `json:"filename,omitempty"`
}

// Begin creates pending session; resource + preflight gates.
func (s *Service) Begin(ctx context.Context, in BeginInput) (map[string]any, error) {
	ws := strings.TrimSpace(in.WorkspaceID)
	if ws == "" {
		return nil, gate(ErrorBody{
			Error:       "resource_gate",
			Fact:        "workspace_id 为空。",
			Remediation: "提供有效的 workspace_id。",
		})
	}
	if _, err := uuid.Parse(ws); err != nil {
		return nil, gate(ErrorBody{
			Error: "resource_gate",
			Fact:  fmt.Sprintf("workspace_id 不是合法 UUID：%s", ws),
		})
	}
	owner := strings.TrimSpace(in.OwnerUserID)
	if owner == "" {
		owner = strings.TrimSpace(os.Getenv("OSV7_OWNER_USER_ID"))
	}
	if owner == "" {
		// stable synthetic owner for solo/dev when users table empty
		owner = "00000000-0000-4000-8000-000000000001"
	}
	ownerUUID := owner
	if _, err := uuid.Parse(owner); err != nil {
		// rag_text_chunks.owner_user_id is uuid; map product text ids deterministically.
		ownerUUID = uuid.NewSHA1(uuid.NameSpaceOID, []byte("osv7-owner:"+owner)).String()
	}

	pf := s.Preflight(ctx)
	if can, _ := pf["can_ingest"].(bool); !can {
		return nil, gate(ErrorBody{
			Error:       "capability_missing",
			Capability:  "embedding",
			Fact:        "本运行时未配置可用 embedding；无法 commit 索引。",
			Remediation: "配置 EMBEDDING_BASE_URL / EMBEDDING_API_KEY（平台 hosted）或 BYOK。",
		})
	}

	// Light sniff: scanned PDF heuristic
	if sniffNeedsOCR(in.Filename, in.ContentSample) {
		return nil, gate(ErrorBody{
			Error:       "capability_missing",
			Capability:  "ocr",
			Fact:        "内容嗅探显示可能为扫描件/无文本层 PDF；本 P3 运行时未配置 OCR。",
			Remediation: "转换为文本原生格式后重试，或配置 OCR（后续阶段）。",
		})
	}

	docID := uuid.NewString()
	runID := uuid.NewString()
	src := in.Source
	if src == "" {
		src = "agent_package"
	}
	sess := &Session{
		DocID:       docID,
		WorkspaceID: ws,
		OwnerUserID: owner,
		OwnerUUID:   ownerUUID,
		Title:       strings.TrimSpace(in.Title),
		Source:      src,
		CreatedAt:   time.Now().UTC(),
		Status:      "open",
		ParseRunID:  runID,
		IR: &DocumentIr{
			SchemaVersion:  SchemaVersion,
			DocumentID:     docID,
			Title:          strings.TrimSpace(in.Title),
			PrimaryBackend: src,
			Blocks:         nil,
		},
	}
	s.mu.Lock()
	s.sessions[docID] = sess
	s.mu.Unlock()

	if s.billing != nil {
		s.billing.Record(ctx, billing.UsageEvent{Tool: "ingest_begin", Kind: "gate", Units: 1})
	}
	return map[string]any{
		"doc_id":         docID,
		"parse_run_id":   runID,
		"schema_version": SchemaVersion,
		"workspace_id":   ws,
		"owner_user_id":  owner,
		"owner_uuid":     ownerUUID,
		"preflight":      pf,
		"status":         "open",
	}, nil
}

// PutBlocks replaces or appends blocks on open session.
func (s *Service) PutBlocks(ctx context.Context, docID string, blocks []BlockIr, replace bool) (map[string]any, error) {
	sess, err := s.getOpen(docID)
	if err != nil {
		return nil, err
	}
	if replace {
		sess.IR.Blocks = blocks
	} else {
		sess.IR.Blocks = append(sess.IR.Blocks, blocks...)
	}
	Normalize(sess.IR)
	return map[string]any{
		"doc_id":       docID,
		"block_count":  len(sess.IR.Blocks),
		"status":       sess.Status,
	}, nil
}

// PutSummary sets summary/kg on open session.
func (s *Service) PutSummary(ctx context.Context, docID, summary string, kg []KGTriple) (map[string]any, error) {
	sess, err := s.getOpen(docID)
	if err != nil {
		return nil, err
	}
	if summary != "" {
		sess.IR.Summary = summary
	}
	if kg != nil {
		sess.IR.KG = kg
	}
	return map[string]any{"doc_id": docID, "summary_len": utf8.RuneCountInString(sess.IR.Summary), "kg_count": len(sess.IR.KG)}, nil
}

// PutPackage installs a full DocumentIr (agent producer).
func (s *Service) PutPackage(ctx context.Context, docID string, ir DocumentIr) (map[string]any, error) {
	sess, err := s.getOpen(docID)
	if err != nil {
		return nil, err
	}
	ir.DocumentID = docID
	if ir.Title == "" {
		ir.Title = sess.Title
	}
	Normalize(&ir)
	sess.IR = &ir
	return map[string]any{"doc_id": docID, "block_count": len(ir.Blocks), "status": "open"}, nil
}

// Commit validates + embeds + writes rag_text_chunks (sync for P3).
func (s *Service) Commit(ctx context.Context, docID string) (map[string]any, error) {
	sess, err := s.getOpen(docID)
	if err != nil {
		return nil, err
	}
	Normalize(sess.IR)
	if issues := ValidateDocumentIr(sess.IR); len(issues) > 0 {
		return nil, gate(ErrorBody{
			Error:       "validation_failed",
			Fact:        "DocumentIr 硬校验未通过。",
			Remediation: "按 detail.issues 补全后重新 put 再 commit。",
			DocID:       docID,
			Detail:      map[string]any{"issues": issues},
		})
	}
	if s.emb == nil || !s.emb.Enabled() {
		return nil, gate(ErrorBody{
			Error:      "capability_missing",
			Capability: "embedding",
			Fact:       "commit 时 embedding 不可用。",
			DocID:      docID,
		})
	}
	nBlocks := len(sess.IR.Blocks)
	if s.wallet != nil {
		cost := billing.PriceEmbedChunkFen * int64(nBlocks)
		if err := s.wallet.EnsureFloor(ctx, sess.OwnerUserID, "embedding", cost); err != nil {
			if fe, ok := err.(billing.FloorError); ok {
				return nil, gate(ErrorBody{
					Error:       "balance_insufficient",
					Capability:  "embedding",
					Fact:        fe.Fact,
					Remediation: fe.Remediation,
					DocID:       docID,
					Detail:      map[string]any{"balance_fen": fe.BalanceFen, "need_fen": cost},
				})
			}
			return nil, err
		}
	}

	// Delete previous version of this doc_id for owner (idempotent re-ingest)
	_, _ = s.store.Exec(ctx, `
DELETE FROM rag_text_chunks WHERE owner_user_id = $1::uuid AND doc_id = $2::uuid`,
		sess.OwnerUUID, docID)

	var chunkIDs []string
	for i, b := range sess.IR.Blocks {
		vec, err := s.emb.Embed(ctx, b.Text)
		if err != nil {
			sess.Status = "failed"
			return nil, gate(ErrorBody{
				Error:      "embed_failed",
				Capability: "embedding",
				Fact:       fmt.Sprintf("第 %d 块 embedding 失败：%v", i, err),
				DocID:      docID,
			})
		}
		chunkID := uuid.NewString()
		if b.BlockID != "" {
			// stable-ish: hash not required; use new uuid for pk
		}
		v := pgvector.NewVector(vec)
		var page any
		if b.Page != nil {
			page = *b.Page
		}
		_, err = s.store.Exec(ctx, `
INSERT INTO rag_text_chunks (
  id, owner_user_id, workspace_id, doc_id, chunk_id, parse_run_id,
  doc_version, page, text, text_dense, chunk_type, parser_backend, source_locator
) VALUES (
  $1, $2::uuid, $3::uuid, $4::uuid, $5::uuid, $6::uuid,
  1, $7, $8, $9, $10, $11, $12::jsonb
)`,
			chunkID,
			sess.OwnerUUID,
			sess.WorkspaceID,
			docID,
			chunkID,
			sess.ParseRunID,
			page,
			b.Text,
			v,
			nullStr(b.BlockType, "text"),
			nullStr(sess.IR.PrimaryBackend, "agent_package"),
			mustJSON(map[string]any{"block_id": b.BlockID, "title": sess.IR.Title}),
		)
		if err != nil {
			sess.Status = "failed"
			return nil, fmt.Errorf("insert chunk: %w", err)
		}
		chunkIDs = append(chunkIDs, chunkID)
		if s.billing != nil {
			s.billing.Record(ctx, billing.UsageEvent{Tool: "ingest_commit", Kind: "embed", Units: 1, Detail: chunkID})
		}
	}
	sess.Status = "committed"
	// drop from memory after commit (terminal)
	s.mu.Lock()
	delete(s.sessions, docID)
	s.mu.Unlock()

	var balanceAfter int64 = -1
	if s.wallet != nil {
		cost := billing.PriceEmbedChunkFen * int64(len(chunkIDs))
		bal, err := s.wallet.DebitUsage(ctx, sess.OwnerUserID, "embedding", cost,
			fmt.Sprintf("ingest:%s", docID), fmt.Sprintf("chunks=%d", len(chunkIDs)))
		if err == nil {
			balanceAfter = bal
		}
	}

	out := map[string]any{
		"doc_id":       docID,
		"status":       "committed",
		"chunk_count":  len(chunkIDs),
		"chunk_ids":    chunkIDs,
		"workspace_id": sess.WorkspaceID,
		"parse_run_id": sess.ParseRunID,
		"title":        sess.IR.Title,
	}
	if balanceAfter >= 0 {
		out["balance_fen"] = balanceAfter
	}
	return out, nil
}

// ParseTextFile is the zero-config producer: plain text/markdown → DocumentIr blocks.
func ParseTextFile(path string) (*DocumentIr, error) {
	b, err := os.ReadFile(path)
	if err != nil {
		return nil, err
	}
	return ParseTextBytes(filepath.Base(path), b), nil
}

// ParseTextBytes splits content into paragraph blocks.
func ParseTextBytes(title string, raw []byte) *DocumentIr {
	text := string(raw)
	// normalize newlines
	text = strings.ReplaceAll(text, "\r\n", "\n")
	parts := strings.Split(text, "\n\n")
	var blocks []BlockIr
	for _, p := range parts {
		p = strings.TrimSpace(p)
		if p == "" {
			continue
		}
		// further split long paragraphs
		for _, piece := range splitLong(p, 1200) {
			blocks = append(blocks, BlockIr{BlockType: "paragraph", Text: piece})
		}
	}
	if len(blocks) == 0 && strings.TrimSpace(text) != "" {
		blocks = append(blocks, BlockIr{BlockType: "paragraph", Text: strings.TrimSpace(text)})
	}
	ir := &DocumentIr{
		SchemaVersion:  SchemaVersion,
		Title:          title,
		DocType:        "text",
		PrimaryBackend: "server_parse_text",
		Blocks:         blocks,
	}
	Normalize(ir)
	return ir
}

func splitLong(s string, maxRunes int) []string {
	r := []rune(s)
	if len(r) <= maxRunes {
		return []string{s}
	}
	var out []string
	for len(r) > 0 {
		n := maxRunes
		if n > len(r) {
			n = len(r)
		}
		// try break at newline
		chunk := r[:n]
		if n < len(r) {
			for i := len(chunk) - 1; i > len(chunk)/2; i-- {
				if chunk[i] == '\n' || chunk[i] == '。' || chunk[i] == '.' {
					n = i + 1
					chunk = r[:n]
					break
				}
			}
		}
		out = append(out, string(chunk))
		r = r[n:]
	}
	return out
}

func (s *Service) getOpen(docID string) (*Session, error) {
	s.mu.Lock()
	defer s.mu.Unlock()
	sess := s.sessions[docID]
	if sess == nil {
		return nil, gate(ErrorBody{
			Error: "session_missing",
			Fact:  fmt.Sprintf("doc_id=%s 无进行中的摄入会话。", docID),
			DocID: docID,
			Remediation: "先调用 ingest_begin。",
		})
	}
	if sess.Status != "open" {
		return nil, gate(ErrorBody{
			Error: "session_closed",
			Fact:  fmt.Sprintf("doc_id=%s 状态为 %s，不可再写。", docID, sess.Status),
			DocID: docID,
		})
	}
	return sess, nil
}

func sniffNeedsOCR(filename string, sample []byte) bool {
	name := strings.ToLower(filename)
	if !strings.HasSuffix(name, ".pdf") {
		return false
	}
	// If sample looks like PDF but has almost no extractable text markers, flag OCR.
	// P3: only flag when sample is empty or pure binary with %PDF and no 'BT' text ops (weak).
	if len(sample) == 0 {
		return false // unknown; don't block
	}
	if !strings.Contains(string(sample[:min(8, len(sample))]), "%PDF") && !strings.HasPrefix(string(sample), "%PDF") {
		// check first bytes
		if len(sample) >= 4 && string(sample[:4]) == "%PDF" {
			// ok
		} else if !strings.Contains(string(sample[:min(1024, len(sample))]), "%PDF") {
			return false
		}
	}
	// Heuristic: if no stream text operators and short, might be scan — soft: only if explicit metadata
	return false
}

func min(a, b int) int {
	if a < b {
		return a
	}
	return b
}

func nullStr(s, def string) string {
	if strings.TrimSpace(s) == "" {
		return def
	}
	return s
}

func mustJSON(v any) []byte {
	b, _ := json.Marshal(v)
	return b
}

type gateErr struct{ Body ErrorBody }

func (e gateErr) Error() string { return e.Body.Error + ": " + e.Body.Fact }

func gate(b ErrorBody) error { return gateErr{Body: b} }

// AsGate extracts structured error.
func AsGate(err error) (ErrorBody, bool) {
	if g, ok := err.(gateErr); ok {
		return g.Body, true
	}
	return ErrorBody{}, false
}

// MustJSON pretty-prints.
func MustJSON(v any) string {
	b, err := json.MarshalIndent(v, "", "  ")
	if err != nil {
		return fmt.Sprint(v)
	}
	return string(b)
}
