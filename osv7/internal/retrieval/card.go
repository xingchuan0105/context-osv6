package retrieval

import (
	"fmt"
	"strings"
)

// Valid harness actions (web is agent-side only — not validated as Ok here).
var harnessActions = map[string]struct{}{
	"dense": {}, "lexical": {}, "grep": {},
	"struct_catalog": {}, "struct_query": {}, "doc_summary": {},
}

// QueryCard is the task-level card (L0: no card → no retrieval).
type QueryCard struct {
	QuestionType    string   `json:"question_type"`
	RequiredActions []string `json:"required_actions"`
	WorkspaceID     string   `json:"workspace_id"`
	DocIDs          []string `json:"doc_ids,omitempty"`
	// WebIntent is declarative metadata only for harness (not verified here).
	WebIntent        bool `json:"web_intent,omitempty"`
	EvidenceRequired bool `json:"evidence_required,omitempty"`

	// Mode: "open" (agent self-declare) | "explicit" (UI options alignment).
	Mode string `json:"mode,omitempty"`
	// ExplicitOptions: when mode=explicit, card must match these options.
	ExplicitOptions *ExplicitOptions `json:"explicit_options,omitempty"`
}

// ExplicitOptions is the product UI selection for dual-mode hard alignment.
type ExplicitOptions struct {
	WorkspaceID     string   `json:"workspace_id"`
	RequiredActions []string `json:"required_actions,omitempty"`
	WebIntent       *bool    `json:"web_intent,omitempty"`
}

// Normalize trims and lowercases mode/actions.
func (c *QueryCard) Normalize() {
	c.QuestionType = strings.TrimSpace(c.QuestionType)
	if c.QuestionType == "" {
		c.QuestionType = "other"
	}
	c.WorkspaceID = strings.TrimSpace(c.WorkspaceID)
	c.Mode = strings.ToLower(strings.TrimSpace(c.Mode))
	if c.Mode == "" {
		c.Mode = "open"
	}
	out := make([]string, 0, len(c.RequiredActions))
	seen := map[string]struct{}{}
	for _, a := range c.RequiredActions {
		a = strings.TrimSpace(strings.ToLower(a))
		if a == "" {
			continue
		}
		// web is not a harness action — drop from required for contract gate
		if a == "web" || a == "fetch" {
			c.WebIntent = true
			continue
		}
		if _, ok := harnessActions[a]; !ok {
			continue
		}
		if _, dup := seen[a]; dup {
			continue
		}
		seen[a] = struct{}{}
		out = append(out, a)
	}
	c.RequiredActions = out
	docs := make([]string, 0, len(c.DocIDs))
	for _, d := range c.DocIDs {
		d = strings.TrimSpace(d)
		if d != "" {
			docs = append(docs, d)
		}
	}
	c.DocIDs = docs
}

// ValidateShape checks card-only rules (resource existence is ResourceGate).
func (c *QueryCard) ValidateShape() error {
	c.Normalize()
	if c.WorkspaceID == "" {
		return fmt.Errorf("workspace_id is required")
	}
	if c.Mode != "open" && c.Mode != "explicit" {
		return fmt.Errorf("mode must be open or explicit")
	}
	if c.Mode == "explicit" {
		if c.ExplicitOptions == nil {
			return fmt.Errorf("explicit mode requires explicit_options")
		}
		eo := c.ExplicitOptions
		if strings.TrimSpace(eo.WorkspaceID) == "" {
			return fmt.Errorf("explicit_options.workspace_id is required")
		}
		if eo.WorkspaceID != c.WorkspaceID {
			return fmt.Errorf("workspace_id must match explicit_options.workspace_id")
		}
		if len(eo.RequiredActions) > 0 {
			want := normalizeActionSet(eo.RequiredActions)
			got := normalizeActionSet(c.RequiredActions)
			if !stringSetEqual(want, got) {
				return fmt.Errorf("required_actions must match explicit_options")
			}
		}
		if eo.WebIntent != nil && *eo.WebIntent != c.WebIntent {
			return fmt.Errorf("web_intent must match explicit_options")
		}
	}
	return nil
}

func normalizeActionSet(in []string) map[string]struct{} {
	out := map[string]struct{}{}
	for _, a := range in {
		a = strings.TrimSpace(strings.ToLower(a))
		if a == "" || a == "web" || a == "fetch" {
			continue
		}
		if _, ok := harnessActions[a]; ok {
			out[a] = struct{}{}
		}
	}
	return out
}

func stringSetEqual(a, b map[string]struct{}) bool {
	if len(a) != len(b) {
		return false
	}
	for k := range a {
		if _, ok := b[k]; !ok {
			return false
		}
	}
	return true
}
