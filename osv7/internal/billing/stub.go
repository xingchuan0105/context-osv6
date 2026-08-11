package billing

import (
	"context"
	"log"
	"sync"
	"time"
)

// Capability is hosted | byok | missing (L0 preflight shape).
type Capability string

const (
	Hosted  Capability = "hosted"
	BYOK    Capability = "byok"
	Missing Capability = "missing"
)

// Snapshot is the account capability table.
type Snapshot struct {
	Embedding Capability `json:"embedding"`
	Rerank    Capability `json:"rerank"`
	OCR       Capability `json:"ocr"`
}

// UsageEvent is a metering stub for P1 (no wallet deduct yet).
type UsageEvent struct {
	At       time.Time `json:"at"`
	Tool     string    `json:"tool"`
	Kind     string    `json:"kind"` // embed | search | gate
	Units    int       `json:"units"`
	UserID   string    `json:"user_id,omitempty"`
	Detail   string    `json:"detail,omitempty"`
}

// Surface is the narrow metering face used by retrieval/ingest/agentd.
type Surface interface {
	Capabilities(ctx context.Context) Snapshot
	Record(ctx context.Context, ev UsageEvent)
}

// Service is an in-memory metering stub (no wallet).
type Service struct {
	mu     sync.Mutex
	events []UsageEvent
	snap   Snapshot
}

func NewStub(embedHosted bool) *Service {
	emb := Missing
	if embedHosted {
		emb = Hosted
	}
	return &Service{
		snap: Snapshot{
			Embedding: emb,
			Rerank:    Missing,
			OCR:       Missing,
		},
	}
}

func (s *Service) Capabilities(_ context.Context) Snapshot {
	return s.snap
}

func (s *Service) Record(_ context.Context, ev UsageEvent) {
	if ev.At.IsZero() {
		ev.At = time.Now().UTC()
	}
	s.mu.Lock()
	s.events = append(s.events, ev)
	n := len(s.events)
	s.mu.Unlock()
	log.Printf("billing usage #%d tool=%s kind=%s units=%d detail=%s", n, ev.Tool, ev.Kind, ev.Units, ev.Detail)
}

func (s *Service) Events() []UsageEvent {
	s.mu.Lock()
	defer s.mu.Unlock()
	out := make([]UsageEvent, len(s.events))
	copy(out, s.events)
	return out
}
