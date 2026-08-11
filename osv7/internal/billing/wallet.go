package billing

import (
	"context"
	"fmt"
	"sync"
	"time"

	"github.com/context-os/osv7/internal/store"
	"github.com/google/uuid"
	"github.com/jackc/pgx/v5"
)

// WalletService is the only place that mutates balances (package discipline).
// Uses osv7_* tables so solo/dev works without v6 users FK rows.
type WalletService struct {
	pool *store.Pool
	// in-memory fallback when pool nil (tests)
	mu      sync.Mutex
	mem     map[string]int64
	events  []UsageEvent
	byokKeys map[string]bool // userID → embedding byok
}

func NewWalletService(pool *store.Pool) *WalletService {
	return &WalletService{
		pool:     pool,
		mem:      map[string]int64{},
		byokKeys: map[string]bool{},
	}
}

// EnsureSchema creates osv7 wallet/share tables.
func (w *WalletService) EnsureSchema(ctx context.Context) error {
	if w.pool == nil {
		return nil
	}
	_, err := w.pool.Exec(ctx, `
CREATE TABLE IF NOT EXISTS osv7_wallets (
  user_id     text PRIMARY KEY,
  balance_fen bigint NOT NULL DEFAULT 0 CHECK (balance_fen >= 0),
  updated_at  timestamptz NOT NULL DEFAULT now()
);
CREATE TABLE IF NOT EXISTS osv7_wallet_ledger (
  id                uuid PRIMARY KEY,
  user_id           text NOT NULL,
  kind              text NOT NULL,
  amount_fen        bigint NOT NULL,
  balance_after_fen bigint NOT NULL,
  idempotency_key   text NOT NULL UNIQUE,
  detail            text NOT NULL DEFAULT '',
  created_at        timestamptz NOT NULL DEFAULT now()
);
CREATE INDEX IF NOT EXISTS osv7_wallet_ledger_user_created
  ON osv7_wallet_ledger (user_id, created_at DESC);

CREATE TABLE IF NOT EXISTS osv7_byok (
  user_id    text NOT NULL,
  capability text NOT NULL, -- embedding|rerank|ocr|chat
  enabled    boolean NOT NULL DEFAULT true,
  updated_at timestamptz NOT NULL DEFAULT now(),
  PRIMARY KEY (user_id, capability)
);
`)
	return err
}

// Capabilities for a user: hosted if balance>0 or always hosted with debit; byok if flagged.
func (w *WalletService) CapabilitiesFor(ctx context.Context, userID string) Snapshot {
	byokEmb := w.HasBYOK(ctx, userID, "embedding")
	emb := Hosted
	if byokEmb {
		emb = BYOK
	} else if bal, _ := w.Balance(ctx, userID); bal <= 0 {
		// still hosted shape but debit will fail — preflight uses Balance separately
		emb = Hosted
	}
	return Snapshot{Embedding: emb, Rerank: Missing, OCR: Missing}
}

// Capabilities implements stub-compatible no-user view.
func (w *WalletService) Capabilities(ctx context.Context) Snapshot {
	return Snapshot{Embedding: Hosted, Rerank: Missing, OCR: Missing}
}

func (w *WalletService) HasBYOK(ctx context.Context, userID, cap string) bool {
	if userID == "" {
		return false
	}
	if w.pool == nil {
		w.mu.Lock()
		defer w.mu.Unlock()
		return w.byokKeys[userID+":"+cap]
	}
	var en bool
	err := w.pool.QueryRow(ctx,
		`SELECT enabled FROM osv7_byok WHERE user_id=$1 AND capability=$2`, userID, cap).Scan(&en)
	if err != nil {
		return false
	}
	return en
}

func (w *WalletService) SetBYOK(ctx context.Context, userID, cap string, enabled bool) error {
	if w.pool == nil {
		w.mu.Lock()
		w.byokKeys[userID+":"+cap] = enabled
		w.mu.Unlock()
		return nil
	}
	_, err := w.pool.Exec(ctx, `
INSERT INTO osv7_byok (user_id, capability, enabled, updated_at)
VALUES ($1,$2,$3,now())
ON CONFLICT (user_id, capability) DO UPDATE SET enabled=$3, updated_at=now()`,
		userID, cap, enabled)
	return err
}

func (w *WalletService) Balance(ctx context.Context, userID string) (int64, error) {
	if userID == "" {
		return 0, fmt.Errorf("user_id required")
	}
	if w.pool == nil {
		w.mu.Lock()
		defer w.mu.Unlock()
		return w.mem[userID], nil
	}
	var bal int64
	err := w.pool.QueryRow(ctx, `SELECT balance_fen FROM osv7_wallets WHERE user_id=$1`, userID).Scan(&bal)
	if err == pgx.ErrNoRows {
		return 0, nil
	}
	return bal, err
}

// TopUp credits wallet (dev/admin path). amount_fen > 0.
func (w *WalletService) TopUp(ctx context.Context, userID string, amountFen int64, idem string) (int64, error) {
	if amountFen <= 0 {
		return 0, fmt.Errorf("amount must be positive")
	}
	if idem == "" {
		idem = uuid.NewString()
	}
	return w.apply(ctx, userID, "topup", amountFen, idem, "topup")
}

// DebitUsage charges usage_debit if not BYOK for capability.
// Returns balance_insufficient structured via error.
func (w *WalletService) DebitUsage(ctx context.Context, userID, capability string, amountFen int64, idem, detail string) (int64, error) {
	if amountFen <= 0 {
		return w.Balance(ctx, userID)
	}
	if w.HasBYOK(ctx, userID, capability) {
		// BYOK: no platform debit
		w.Record(ctx, UsageEvent{Tool: "billing", Kind: "byok_skip", Units: int(amountFen), UserID: userID, Detail: detail})
		return w.Balance(ctx, userID)
	}
	if idem == "" {
		idem = uuid.NewString()
	}
	bal, err := w.apply(ctx, userID, "usage_debit", -amountFen, idem, detail)
	if err != nil {
		return 0, err
	}
	w.Record(ctx, UsageEvent{Tool: "billing", Kind: "usage_debit", Units: int(amountFen), UserID: userID, Detail: detail})
	return bal, nil
}

// EnsureFloor rejects when balance < minFen and not BYOK.
func (w *WalletService) EnsureFloor(ctx context.Context, userID, capability string, minFen int64) error {
	if w.HasBYOK(ctx, userID, capability) {
		return nil
	}
	bal, err := w.Balance(ctx, userID)
	if err != nil {
		return err
	}
	if bal < minFen {
		return FloorError{
			Fact:        fmt.Sprintf("用户 %s 余额 %d 分低于地板 %d 分，且未启用 %s BYOK。", userID, bal, minFen, capability),
			Remediation: "充值（POST /v1/billing/topup）或配置 BYOK。",
			BalanceFen:  bal,
		}
	}
	return nil
}

type FloorError struct {
	Fact        string
	Remediation string
	BalanceFen  int64
}

func (e FloorError) Error() string { return "balance_insufficient: " + e.Fact }

func (w *WalletService) apply(ctx context.Context, userID, kind string, amountFen int64, idem, detail string) (int64, error) {
	if w.pool == nil {
		w.mu.Lock()
		defer w.mu.Unlock()
		// idempotency skip if seen — simplified mem
		bal := w.mem[userID] + amountFen
		if bal < 0 {
			return w.mem[userID], FloorError{
				Fact:        fmt.Sprintf("余额不足：当前 %d 分，需要 %d 分。", w.mem[userID], -amountFen),
				Remediation: "充值后再试。",
				BalanceFen:  w.mem[userID],
			}
		}
		w.mem[userID] = bal
		return bal, nil
	}

	tx, err := w.pool.Begin(ctx)
	if err != nil {
		return 0, err
	}
	defer tx.Rollback(ctx)

	// idempotent (key should include user scope; still enforce uniqueness)
	var existing int64
	var existingUser string
	err = tx.QueryRow(ctx, `SELECT user_id, balance_after_fen FROM osv7_wallet_ledger WHERE idempotency_key=$1`, idem).Scan(&existingUser, &existing)
	if err == nil {
		_ = tx.Commit(ctx)
		if existingUser != userID {
			return 0, fmt.Errorf("idempotency_key already used by another user")
		}
		return existing, nil
	}
	if err != pgx.ErrNoRows {
		return 0, err
	}

	// lock wallet row
	var bal int64
	err = tx.QueryRow(ctx, `SELECT balance_fen FROM osv7_wallets WHERE user_id=$1 FOR UPDATE`, userID).Scan(&bal)
	if err == pgx.ErrNoRows {
		_, err = tx.Exec(ctx, `INSERT INTO osv7_wallets (user_id, balance_fen) VALUES ($1, 0)`, userID)
		if err != nil {
			return 0, err
		}
		bal = 0
	} else if err != nil {
		return 0, err
	}

	next := bal + amountFen
	if next < 0 {
		return bal, FloorError{
			Fact:        fmt.Sprintf("余额不足：当前 %d 分，尝试变动 %d 分。", bal, amountFen),
			Remediation: "充值（/v1/billing/topup）或启用 BYOK。",
			BalanceFen:  bal,
		}
	}
	_, err = tx.Exec(ctx, `UPDATE osv7_wallets SET balance_fen=$2, updated_at=now() WHERE user_id=$1`, userID, next)
	if err != nil {
		return 0, err
	}
	_, err = tx.Exec(ctx, `
INSERT INTO osv7_wallet_ledger (id, user_id, kind, amount_fen, balance_after_fen, idempotency_key, detail)
VALUES ($1::uuid, $2, $3, $4, $5, $6, $7)`,
		uuid.NewString(), userID, kind, amountFen, next, idem, detail)
	if err != nil {
		return 0, err
	}
	if err := tx.Commit(ctx); err != nil {
		return 0, err
	}
	return next, nil
}

// Record keeps telemetry event list (compat with stub).
func (w *WalletService) Record(_ context.Context, ev UsageEvent) {
	if ev.At.IsZero() {
		ev.At = time.Now().UTC()
	}
	w.mu.Lock()
	w.events = append(w.events, ev)
	w.mu.Unlock()
}

func (w *WalletService) Events() []UsageEvent {
	w.mu.Lock()
	defer w.mu.Unlock()
	out := make([]UsageEvent, len(w.events))
	copy(out, w.events)
	return out
}

// Prices in fen (1/100 yuan) for P4 smoke — not production tariffs.
const (
	PriceEmbedChunkFen int64 = 1  // per chunk embed
	PriceChatTurnFen   int64 = 10 // per agent turn
	PriceSearchFen     int64 = 1  // per retrieval call
)
