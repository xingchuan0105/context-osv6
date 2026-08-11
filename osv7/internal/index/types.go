package index

// Hit is one retrieval result row with evidence alias bookkeeping left to retrieval session.
type Hit struct {
	ChunkID     string         `json:"chunk_id"`
	DocID       string         `json:"doc_id"`
	WorkspaceID string         `json:"workspace_id,omitempty"`
	Text        string         `json:"text"`
	Snippet     string         `json:"snippet"`
	Page        *int64         `json:"page,omitempty"`
	Score       float64        `json:"score"`
	Backend     string         `json:"backend"`
	Source      map[string]any `json:"source_locator,omitempty"`
}

// SearchOpts scopes a corpus query.
type SearchOpts struct {
	WorkspaceID string
	OwnerUserID string // optional; empty = no owner filter
	DocIDs      []string
	Limit       int
}
