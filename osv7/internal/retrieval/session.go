package retrieval

import (
	"fmt"
	"strings"
	"sync"

	"github.com/context-os/osv7/internal/index"
)

// Handle is one evidence unit with stable alias (#1, #2, …).
type Handle struct {
	Alias   string    `json:"alias"`
	ChunkID string    `json:"chunk_id"`
	DocID   string    `json:"doc_id"`
	Snippet string    `json:"snippet"`
	Score   float64   `json:"score,omitempty"`
	Backend string    `json:"backend,omitempty"`
	Reseen  string    `json:"reseen,omitempty"` // prior alias if same chunk
	Hit     index.Hit `json:"-"`
}

// Session is task-scoped state: card + alias space + Ok actions + selection.
type Session struct {
	mu sync.Mutex

	Card *QueryCard

	// OkActions: harness action → at least one non-error tool return.
	OkActions map[string]bool

	// chunkID → first alias
	seen map[string]string
	// alias → handle
	byAlias map[string]*Handle
	// order of first-seen aliases
	order []string

	// SELECTED set (alias → true)
	selected map[string]bool
	// KEEP freeze
	kept map[string]bool

	counter uint64
}

func NewSession() *Session {
	return &Session{
		OkActions: map[string]bool{},
		seen:      map[string]string{},
		byAlias:   map[string]*Handle{},
		selected:  map[string]bool{},
		kept:      map[string]bool{},
	}
}

func (s *Session) SetCard(c QueryCard) {
	s.mu.Lock()
	defer s.mu.Unlock()
	cp := c
	s.Card = &cp
	// New card = new task: reset observation, keep nothing across tasks.
	s.OkActions = map[string]bool{}
	s.seen = map[string]string{}
	s.byAlias = map[string]*Handle{}
	s.order = nil
	s.selected = map[string]bool{}
	s.kept = map[string]bool{}
	s.counter = 0
}

func (s *Session) GetCard() *QueryCard {
	s.mu.Lock()
	defer s.mu.Unlock()
	return s.Card
}

func (s *Session) MarkOk(action string) {
	s.mu.Lock()
	defer s.mu.Unlock()
	s.OkActions[action] = true
}

func (s *Session) MissingRequired() []string {
	s.mu.Lock()
	defer s.mu.Unlock()
	if s.Card == nil {
		return nil
	}
	var miss []string
	for _, a := range s.Card.RequiredActions {
		if !s.OkActions[a] {
			miss = append(miss, a)
		}
	}
	return miss
}

// IngestHits assigns aliases (reseen shares first alias, body may omit full text).
func (s *Session) IngestHits(hits []index.Hit) []Handle {
	s.mu.Lock()
	defer s.mu.Unlock()
	out := make([]Handle, 0, len(hits))
	for _, h := range hits {
		key := h.ChunkID
		if key == "" {
			key = h.DocID + ":" + h.Snippet
		}
		if prev, ok := s.seen[key]; ok {
			out = append(out, Handle{
				Alias:   prev,
				ChunkID: h.ChunkID,
				DocID:   h.DocID,
				Snippet: "", // reseen: body omitted
				Score:   h.Score,
				Backend: h.Backend,
				Reseen:  prev,
				Hit:     h,
			})
			continue
		}
		s.counter++
		alias := fmt.Sprintf("#%d", s.counter)
		s.seen[key] = alias
		hh := &Handle{
			Alias:   alias,
			ChunkID: h.ChunkID,
			DocID:   h.DocID,
			Snippet: h.Snippet,
			Score:   h.Score,
			Backend: h.Backend,
			Hit:     h,
		}
		s.byAlias[alias] = hh
		s.order = append(s.order, alias)
		out = append(out, *hh)
	}
	return out
}

// Select marks SELECTED aliases (must already exist).
func (s *Session) Select(aliases []string) (ok []string, unknown []string) {
	s.mu.Lock()
	defer s.mu.Unlock()
	for _, a := range aliases {
		a = normalizeAlias(a)
		if _, exists := s.byAlias[a]; !exists {
			unknown = append(unknown, a)
			continue
		}
		s.selected[a] = true
		ok = append(ok, a)
	}
	return ok, unknown
}

// Keep freezes currently selected set (or explicit list).
func (s *Session) Keep(aliases []string) (kept []string, unknown []string) {
	s.mu.Lock()
	defer s.mu.Unlock()
	if len(aliases) == 0 {
		for a := range s.selected {
			s.kept[a] = true
			kept = append(kept, a)
		}
		return kept, nil
	}
	for _, a := range aliases {
		a = normalizeAlias(a)
		if _, exists := s.byAlias[a]; !exists {
			unknown = append(unknown, a)
			continue
		}
		s.kept[a] = true
		s.selected[a] = true
		kept = append(kept, a)
	}
	return kept, unknown
}

// VerifyDraft checks that every #n citation in draft exists (and optionally is selected/kept).
func (s *Session) VerifyDraft(draft string, requireSelected bool) (valid []string, invalid []string) {
	s.mu.Lock()
	defer s.mu.Unlock()
	found := extractAliases(draft)
	for _, a := range found {
		h, ok := s.byAlias[a]
		if !ok || h == nil {
			invalid = append(invalid, a)
			continue
		}
		if requireSelected && !s.selected[a] && !s.kept[a] {
			invalid = append(invalid, a+" (not selected)")
			continue
		}
		valid = append(valid, a)
	}
	return valid, invalid
}

func (s *Session) Status() map[string]any {
	s.mu.Lock()
	defer s.mu.Unlock()
	var card any
	if s.Card != nil {
		card = *s.Card
	}
	sel := keys(s.selected)
	kept := keys(s.kept)
	return map[string]any{
		"card":             card,
		"ok_actions":       copyBoolMap(s.OkActions),
		"missing_required": s.missingLocked(),
		"alias_count":      len(s.order),
		"selected":         sel,
		"kept":             kept,
	}
}

// Snapshot is a JSON-serializable session dump for CLI multi-process continuity.
type Snapshot struct {
	Card      *QueryCard        `json:"card,omitempty"`
	OkActions map[string]bool   `json:"ok_actions,omitempty"`
	Counter   uint64            `json:"counter"`
	Order     []string          `json:"order,omitempty"`
	Handles   map[string]Handle `json:"handles,omitempty"` // alias → handle
	Seen      map[string]string `json:"seen,omitempty"`    // chunk key → alias
	Selected  map[string]bool   `json:"selected,omitempty"`
	Kept      map[string]bool   `json:"kept,omitempty"`
}

// ExportSnapshot copies session state for persistence.
func (s *Session) ExportSnapshot() Snapshot {
	s.mu.Lock()
	defer s.mu.Unlock()
	snap := Snapshot{
		OkActions: copyBoolMap(s.OkActions),
		Counter:   s.counter,
		Order:     append([]string{}, s.order...),
		Handles:   map[string]Handle{},
		Seen:      map[string]string{},
		Selected:  copyBoolMap(s.selected),
		Kept:      copyBoolMap(s.kept),
	}
	if s.Card != nil {
		cp := *s.Card
		snap.Card = &cp
	}
	for k, v := range s.seen {
		snap.Seen[k] = v
	}
	for a, h := range s.byAlias {
		if h != nil {
			hh := *h
			hh.Hit = index.Hit{} // drop heavy hit blob
			snap.Handles[a] = hh
		}
	}
	return snap
}

// ImportSnapshot restores session state (replaces in-memory maps).
func (s *Session) ImportSnapshot(snap Snapshot) {
	s.mu.Lock()
	defer s.mu.Unlock()
	if snap.Card != nil {
		cp := *snap.Card
		s.Card = &cp
	} else {
		s.Card = nil
	}
	s.OkActions = copyBoolMap(snap.OkActions)
	if s.OkActions == nil {
		s.OkActions = map[string]bool{}
	}
	s.counter = snap.Counter
	s.order = append([]string{}, snap.Order...)
	s.seen = map[string]string{}
	for k, v := range snap.Seen {
		s.seen[k] = v
	}
	s.byAlias = map[string]*Handle{}
	for a, h := range snap.Handles {
		hh := h
		s.byAlias[a] = &hh
	}
	s.selected = copyBoolMap(snap.Selected)
	if s.selected == nil {
		s.selected = map[string]bool{}
	}
	s.kept = copyBoolMap(snap.Kept)
	if s.kept == nil {
		s.kept = map[string]bool{}
	}
}

func (s *Session) missingLocked() []string {
	if s.Card == nil {
		return nil
	}
	var miss []string
	for _, a := range s.Card.RequiredActions {
		if !s.OkActions[a] {
			miss = append(miss, a)
		}
	}
	return miss
}

func normalizeAlias(a string) string {
	a = strings.TrimSpace(a)
	if a == "" {
		return a
	}
	if !strings.HasPrefix(a, "#") {
		a = "#" + a
	}
	return a
}

func extractAliases(draft string) []string {
	// simple scan for #digits
	var out []string
	seen := map[string]struct{}{}
	for i := 0; i < len(draft); i++ {
		if draft[i] != '#' {
			continue
		}
		j := i + 1
		for j < len(draft) && draft[j] >= '0' && draft[j] <= '9' {
			j++
		}
		if j == i+1 {
			continue
		}
		a := draft[i:j]
		if _, ok := seen[a]; ok {
			continue
		}
		seen[a] = struct{}{}
		out = append(out, a)
	}
	return out
}

func keys(m map[string]bool) []string {
	out := make([]string, 0, len(m))
	for k, v := range m {
		if v {
			out = append(out, k)
		}
	}
	return out
}

func copyBoolMap(m map[string]bool) map[string]bool {
	out := make(map[string]bool, len(m))
	for k, v := range m {
		out[k] = v
	}
	return out
}
