package store

import (
	"context"
	"encoding/json"
	"fmt"
	"time"

	"github.com/google/uuid"
	"github.com/jackc/pgx/v5"
)

// Product session projection (pi transcript is source of truth; PG is UI list/bubble view).

type SessionRow struct {
	ID            string    `json:"id"`
	UserID        string    `json:"user_id,omitempty"`
	WorkspaceID   string    `json:"workspace_id,omitempty"`
	Title         string    `json:"title,omitempty"`
	PiSessionFile string    `json:"pi_session_file,omitempty"`
	CreatedAt     time.Time `json:"created_at"`
	UpdatedAt     time.Time `json:"updated_at"`
}

type MessageRow struct {
	ID        string          `json:"id"`
	SessionID string          `json:"session_id"`
	Role      string          `json:"role"` // user | assistant | system
	Content   string          `json:"content"`
	Tools     json.RawMessage `json:"tools,omitempty"`
	Meta      json.RawMessage `json:"meta,omitempty"`
	CreatedAt time.Time       `json:"created_at"`
}

// EnsureSessionSchema creates osv7 projection tables (idempotent).
func (p *Pool) EnsureSessionSchema(ctx context.Context) error {
	_, err := p.Exec(ctx, `
CREATE TABLE IF NOT EXISTS osv7_sessions (
  id              uuid PRIMARY KEY,
  user_id         text NOT NULL DEFAULT '',
  workspace_id    text NOT NULL DEFAULT '',
  title           text NOT NULL DEFAULT '',
  pi_session_file text NOT NULL DEFAULT '',
  created_at      timestamptz NOT NULL DEFAULT now(),
  updated_at      timestamptz NOT NULL DEFAULT now()
);
CREATE INDEX IF NOT EXISTS osv7_sessions_user_updated
  ON osv7_sessions (user_id, updated_at DESC);

CREATE TABLE IF NOT EXISTS osv7_messages (
  id         uuid PRIMARY KEY,
  session_id uuid NOT NULL REFERENCES osv7_sessions(id) ON DELETE CASCADE,
  role       text NOT NULL,
  content    text NOT NULL DEFAULT '',
  tools      jsonb,
  meta       jsonb,
  created_at timestamptz NOT NULL DEFAULT now()
);
CREATE INDEX IF NOT EXISTS osv7_messages_session_created
  ON osv7_messages (session_id, created_at);
`)
	return err
}

func (p *Pool) CreateSession(ctx context.Context, userID, workspaceID, title string) (*SessionRow, error) {
	id := uuid.NewString()
	now := time.Now().UTC()
	_, err := p.Exec(ctx, `
INSERT INTO osv7_sessions (id, user_id, workspace_id, title, created_at, updated_at)
VALUES ($1::uuid, $2, $3, $4, $5, $5)`,
		id, userID, workspaceID, title, now)
	if err != nil {
		return nil, err
	}
	return &SessionRow{
		ID: id, UserID: userID, WorkspaceID: workspaceID, Title: title,
		CreatedAt: now, UpdatedAt: now,
	}, nil
}

func (p *Pool) GetSession(ctx context.Context, id string) (*SessionRow, error) {
	var s SessionRow
	err := p.QueryRow(ctx, `
SELECT id::text, user_id, workspace_id, title, pi_session_file, created_at, updated_at
FROM osv7_sessions WHERE id = $1::uuid`, id).Scan(
		&s.ID, &s.UserID, &s.WorkspaceID, &s.Title, &s.PiSessionFile, &s.CreatedAt, &s.UpdatedAt,
	)
	if err == pgx.ErrNoRows {
		return nil, nil
	}
	if err != nil {
		return nil, err
	}
	return &s, nil
}

func (p *Pool) ListSessions(ctx context.Context, userID string, limit int) ([]SessionRow, error) {
	if limit <= 0 {
		limit = 50
	}
	rows, err := p.Query(ctx, `
SELECT id::text, user_id, workspace_id, title, pi_session_file, created_at, updated_at
FROM osv7_sessions
WHERE ($1 = '' OR user_id = $1)
ORDER BY updated_at DESC
LIMIT $2`, userID, limit)
	if err != nil {
		return nil, err
	}
	defer rows.Close()
	var out []SessionRow
	for rows.Next() {
		var s SessionRow
		if err := rows.Scan(&s.ID, &s.UserID, &s.WorkspaceID, &s.Title, &s.PiSessionFile, &s.CreatedAt, &s.UpdatedAt); err != nil {
			return nil, err
		}
		out = append(out, s)
	}
	return out, rows.Err()
}

func (p *Pool) SetPiSessionFile(ctx context.Context, id, path string) error {
	_, err := p.Exec(ctx, `
UPDATE osv7_sessions SET pi_session_file = $2, updated_at = now() WHERE id = $1::uuid`, id, path)
	return err
}

func (p *Pool) TouchSession(ctx context.Context, id, title string) error {
	if title != "" {
		_, err := p.Exec(ctx, `
UPDATE osv7_sessions SET updated_at = now(),
  title = CASE WHEN title = '' OR title = 'new chat' THEN $2 ELSE title END
WHERE id = $1::uuid`, id, title)
		return err
	}
	_, err := p.Exec(ctx, `UPDATE osv7_sessions SET updated_at = now() WHERE id = $1::uuid`, id)
	return err
}

func (p *Pool) AppendMessage(ctx context.Context, sessionID, role, content string, tools any, meta any) (*MessageRow, error) {
	id := uuid.NewString()
	now := time.Now().UTC()
	var toolsJSON, metaJSON []byte
	var err error
	if tools != nil {
		toolsJSON, err = json.Marshal(tools)
		if err != nil {
			return nil, err
		}
	}
	if meta != nil {
		metaJSON, err = json.Marshal(meta)
		if err != nil {
			return nil, err
		}
	}
	_, err = p.Exec(ctx, `
INSERT INTO osv7_messages (id, session_id, role, content, tools, meta, created_at)
VALUES ($1::uuid, $2::uuid, $3, $4, $5, $6, $7)`,
		id, sessionID, role, content, nullJSON(toolsJSON), nullJSON(metaJSON), now)
	if err != nil {
		return nil, err
	}
	_ = p.TouchSession(ctx, sessionID, "")
	m := &MessageRow{
		ID: id, SessionID: sessionID, Role: role, Content: content,
		Tools: toolsJSON, Meta: metaJSON, CreatedAt: now,
	}
	return m, nil
}

func (p *Pool) ListMessages(ctx context.Context, sessionID string, limit int) ([]MessageRow, error) {
	if limit <= 0 {
		limit = 500
	}
	rows, err := p.Query(ctx, `
SELECT id::text, session_id::text, role, content, coalesce(tools,'null'::jsonb), coalesce(meta,'null'::jsonb), created_at
FROM osv7_messages
WHERE session_id = $1::uuid
ORDER BY created_at ASC
LIMIT $2`, sessionID, limit)
	if err != nil {
		return nil, err
	}
	defer rows.Close()
	var out []MessageRow
	for rows.Next() {
		var m MessageRow
		if err := rows.Scan(&m.ID, &m.SessionID, &m.Role, &m.Content, &m.Tools, &m.Meta, &m.CreatedAt); err != nil {
			return nil, err
		}
		out = append(out, m)
	}
	return out, rows.Err()
}

func nullJSON(b []byte) any {
	if len(b) == 0 {
		return nil
	}
	return b
}

// TitleFromMessage truncates user text for session list.
func TitleFromMessage(s string) string {
	r := []rune(s)
	if len(r) > 40 {
		return string(r[:40]) + "…"
	}
	if s == "" {
		return "new chat"
	}
	return s
}

// EnsureUUID validates or generates.
func EnsureUUID(id string) (string, error) {
	if id == "" {
		return uuid.NewString(), nil
	}
	if _, err := uuid.Parse(id); err != nil {
		return "", fmt.Errorf("invalid session id: %w", err)
	}
	return id, nil
}
