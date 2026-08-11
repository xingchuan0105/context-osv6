package share

import (
	"context"
	"crypto/rand"
	"crypto/sha256"
	"encoding/hex"
	"fmt"
	"strings"
	"time"

	"github.com/context-os/osv7/internal/store"
	"github.com/jackc/pgx/v5"
)

// Service owns public read-only share links (no LLM, no writes to corpus).
type Service struct {
	pool *store.Pool
}

func New(pool *store.Pool) *Service {
	return &Service{pool: pool}
}

func (s *Service) EnsureSchema(ctx context.Context) error {
	_, err := s.pool.Exec(ctx, `
CREATE TABLE IF NOT EXISTS osv7_share_links (
  token         text PRIMARY KEY,
  workspace_id  text NOT NULL,
  owner_user_id text NOT NULL DEFAULT '',
  title         text NOT NULL DEFAULT '',
  access_level  text NOT NULL DEFAULT 'read',
  created_at    timestamptz NOT NULL DEFAULT now(),
  expires_at    timestamptz,
  revoked_at    timestamptz,
  access_count  bigint NOT NULL DEFAULT 0
);
CREATE INDEX IF NOT EXISTS osv7_share_links_ws ON osv7_share_links (workspace_id);
`)
	return err
}

type Link struct {
	Token       string     `json:"token"`
	WorkspaceID string     `json:"workspace_id"`
	OwnerUserID string     `json:"owner_user_id,omitempty"`
	Title       string     `json:"title"`
	AccessLevel string     `json:"access_level"`
	CreatedAt   time.Time  `json:"created_at"`
	ExpiresAt   *time.Time `json:"expires_at,omitempty"`
	RevokedAt   *time.Time `json:"revoked_at,omitempty"`
	AccessCount int64      `json:"access_count"`
}

// Create issues a new public token for a workspace (read-only).
func (s *Service) Create(ctx context.Context, workspaceID, ownerUserID, title string, ttl time.Duration) (*Link, error) {
	ws := strings.TrimSpace(workspaceID)
	if ws == "" {
		return nil, fmt.Errorf("workspace_id required")
	}
	tok, err := randomToken(24)
	if err != nil {
		return nil, err
	}
	var exp *time.Time
	if ttl > 0 {
		t := time.Now().UTC().Add(ttl)
		exp = &t
	}
	if title == "" {
		title = "shared workspace"
	}
	_, err = s.pool.Exec(ctx, `
INSERT INTO osv7_share_links (token, workspace_id, owner_user_id, title, access_level, expires_at)
VALUES ($1,$2,$3,$4,'read',$5)`, tok, ws, ownerUserID, title, exp)
	if err != nil {
		return nil, err
	}
	return s.Get(ctx, tok)
}

func (s *Service) Get(ctx context.Context, token string) (*Link, error) {
	var l Link
	var exp, rev *time.Time
	err := s.pool.QueryRow(ctx, `
SELECT token, workspace_id, owner_user_id, title, access_level, created_at, expires_at, revoked_at, access_count
FROM osv7_share_links WHERE token=$1`, token).Scan(
		&l.Token, &l.WorkspaceID, &l.OwnerUserID, &l.Title, &l.AccessLevel,
		&l.CreatedAt, &exp, &rev, &l.AccessCount,
	)
	if err == pgx.ErrNoRows {
		return nil, nil
	}
	if err != nil {
		return nil, err
	}
	l.ExpiresAt, l.RevokedAt = exp, rev
	return &l, nil
}

func (s *Service) Revoke(ctx context.Context, token string) error {
	_, err := s.pool.Exec(ctx, `UPDATE osv7_share_links SET revoked_at=now() WHERE token=$1 AND revoked_at IS NULL`, token)
	return err
}

// PublicView is the payload for anonymous GET (no private keys, no write).
type PublicView struct {
	Token         string         `json:"token"`
	Title         string         `json:"title"`
	WorkspaceID   string         `json:"workspace_id"`
	AccessLevel   string         `json:"access_level"`
	ChunkCount    int64          `json:"chunk_count"`
	DocCount      int64          `json:"doc_count"`
	SampleSnippets []string      `json:"sample_snippets,omitempty"`
	ETag          string         `json:"etag"`
	// Recent product chat bubbles if any (read-only projection)
	RecentMessages []store.MessageRow `json:"recent_messages,omitempty"`
}

// ResolvePublic validates token and builds cacheable public view.
func (s *Service) ResolvePublic(ctx context.Context, token string) (*PublicView, error) {
	l, err := s.Get(ctx, token)
	if err != nil {
		return nil, err
	}
	if l == nil {
		return nil, nil
	}
	if l.RevokedAt != nil {
		return nil, fmt.Errorf("share revoked")
	}
	if l.ExpiresAt != nil && time.Now().After(*l.ExpiresAt) {
		return nil, fmt.Errorf("share expired")
	}
	var chunkCount, docCount int64
	_ = s.pool.QueryRow(ctx, `
SELECT count(*), count(DISTINCT doc_id)
FROM rag_text_chunks WHERE workspace_id::text = $1`, l.WorkspaceID).Scan(&chunkCount, &docCount)

	rows, err := s.pool.Query(ctx, `
SELECT left(text, 160) FROM rag_text_chunks
WHERE workspace_id::text = $1
ORDER BY id LIMIT 3`, l.WorkspaceID)
	var snips []string
	if err == nil {
		defer rows.Close()
		for rows.Next() {
			var t string
			if rows.Scan(&t) == nil {
				snips = append(snips, t)
			}
		}
	}

	// optional: recent messages from any session tagged with this workspace
	var msgs []store.MessageRow
	mrows, err := s.pool.Query(ctx, `
SELECT m.id::text, m.session_id::text, m.role, m.content, m.created_at
FROM osv7_messages m
JOIN osv7_sessions s ON s.id = m.session_id
WHERE s.workspace_id = $1
ORDER BY m.created_at DESC
LIMIT 10`, l.WorkspaceID)
	if err == nil {
		defer mrows.Close()
		for mrows.Next() {
			var m store.MessageRow
			if mrows.Scan(&m.ID, &m.SessionID, &m.Role, &m.Content, &m.CreatedAt) == nil {
				msgs = append(msgs, m)
			}
		}
		// reverse to chronological
		for i, j := 0, len(msgs)-1; i < j; i, j = i+1, j-1 {
			msgs[i], msgs[j] = msgs[j], msgs[i]
		}
	}

	// ETag from content shape only (not access_count — counter must not bust cache).
	etag := etagOf(l.Token, l.Title, chunkCount, docCount, len(snips))
	// bump access after etag (best-effort)
	_, _ = s.pool.Exec(ctx, `UPDATE osv7_share_links SET access_count = access_count + 1 WHERE token=$1`, token)

	return &PublicView{
		Token:          l.Token,
		Title:          l.Title,
		WorkspaceID:    l.WorkspaceID,
		AccessLevel:    l.AccessLevel,
		ChunkCount:     chunkCount,
		DocCount:       docCount,
		SampleSnippets: snips,
		ETag:           etag,
		RecentMessages: msgs,
	}, nil
}

func randomToken(n int) (string, error) {
	b := make([]byte, n)
	if _, err := rand.Read(b); err != nil {
		return "", err
	}
	return hex.EncodeToString(b), nil
}

func etagOf(parts ...any) string {
	h := sha256.New()
	for _, p := range parts {
		fmt.Fprintf(h, "%v|", p)
	}
	return `"` + hex.EncodeToString(h.Sum(nil))[:16] + `"`
}
