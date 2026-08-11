package store

import (
	"context"
	"fmt"

	"github.com/jackc/pgx/v5/pgxpool"
)

// Pool is the only package allowed to hold a pgx pool (SQL boundary).
type Pool struct {
	*pgxpool.Pool
}

func Connect(ctx context.Context, databaseURL string) (*Pool, error) {
	cfg, err := pgxpool.ParseConfig(databaseURL)
	if err != nil {
		return nil, fmt.Errorf("parse DATABASE_URL: %w", err)
	}
	p, err := pgxpool.NewWithConfig(ctx, cfg)
	if err != nil {
		return nil, fmt.Errorf("pg connect: %w", err)
	}
	if err := p.Ping(ctx); err != nil {
		p.Close()
		return nil, fmt.Errorf("pg ping: %w", err)
	}
	return &Pool{Pool: p}, nil
}

// WorkspaceChunkCount returns how many rag_text_chunks exist for a workspace.
func (p *Pool) WorkspaceChunkCount(ctx context.Context, workspaceID string) (int64, error) {
	var n int64
	err := p.QueryRow(ctx,
		`SELECT count(*) FROM rag_text_chunks WHERE workspace_id::text = $1`,
		workspaceID,
	).Scan(&n)
	return n, err
}

// WorkspaceExists is true when workspaces row exists OR chunks exist (v6 orphan chunks).
func (p *Pool) WorkspaceExists(ctx context.Context, workspaceID string) (bool, error) {
	var ok bool
	err := p.QueryRow(ctx, `
SELECT EXISTS(SELECT 1 FROM workspaces WHERE id::text = $1)
    OR EXISTS(SELECT 1 FROM rag_text_chunks WHERE workspace_id::text = $1)
`, workspaceID).Scan(&ok)
	return ok, err
}
