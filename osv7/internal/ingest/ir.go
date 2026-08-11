package ingest

// DocumentIr is the versioned intake package (field-aligned with v6 ir.rs, simplified for P3).
// SchemaVersion documents the contract; hard validation enforces required fields.

const SchemaVersion = "osv7-document-ir-p3.0"

// DocumentIr is one document's structured intake.
type DocumentIr struct {
	SchemaVersion  string            `json:"schema_version"`
	DocumentID     string            `json:"document_id,omitempty"` // optional; server may assign
	Title          string            `json:"title"`
	DocType        string            `json:"doc_type"` // text|markdown|pdf|office|other
	PrimaryBackend string            `json:"primary_backend"`
	Language       string            `json:"language,omitempty"`
	Metadata       map[string]string `json:"metadata,omitempty"`
	Blocks         []BlockIr         `json:"blocks"`
	Summary        string            `json:"summary,omitempty"`
	// KG is optional for P3; accepted but not indexed yet.
	KG []KGTriple `json:"kg,omitempty"`
}

// BlockIr is one content block (chunk unit for P3: 1 block → 1 text chunk).
type BlockIr struct {
	BlockID   string `json:"block_id,omitempty"`
	Page      *int64 `json:"page,omitempty"`
	BlockType string `json:"block_type"` // paragraph|heading|table|list|other
	Text      string `json:"text"`
}

// KGTriple is a minimal knowledge graph edge (accepted, not required for commit).
type KGTriple struct {
	Subject   string `json:"subject"`
	Predicate string `json:"predicate"`
	Object    string `json:"object"`
}

// Capabilities mirrors L0 account capability table.
type Capabilities struct {
	Embedding string `json:"embedding"` // hosted|byok|missing
	Rerank    string `json:"rerank"`
	OCR       string `json:"ocr"`
}

// ErrorBody is L0 agent-actionable error (third-person facts).
type ErrorBody struct {
	Error       string `json:"error"`
	Capability  string `json:"capability,omitempty"`
	Fact        string `json:"fact"`
	Remediation string `json:"remediation,omitempty"`
	DocID       string `json:"doc_id,omitempty"`
	Detail      any    `json:"detail,omitempty"`
}
