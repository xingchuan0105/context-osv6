package ingest

import (
	"fmt"
	"strings"
	"unicode/utf8"
)

// ValidationIssue is one hard-validation gap.
type ValidationIssue struct {
	Code    string `json:"code"`
	Path    string `json:"path,omitempty"`
	Fact    string `json:"fact"`
}

// ValidateDocumentIr hard-checks IR before commit (L0: gap list is agent feedback).
func ValidateDocumentIr(ir *DocumentIr) []ValidationIssue {
	var issues []ValidationIssue
	if ir == nil {
		return []ValidationIssue{{Code: "empty_ir", Fact: "DocumentIr 为空。"}}
	}
	if strings.TrimSpace(ir.Title) == "" {
		issues = append(issues, ValidationIssue{Code: "title_missing", Path: "title", Fact: "title 为空。"})
	}
	if len(ir.Blocks) == 0 {
		issues = append(issues, ValidationIssue{Code: "blocks_empty", Path: "blocks", Fact: "blocks 为空；至少需要一个文本块。"})
	}
	totalChars := 0
	for i, b := range ir.Blocks {
		t := strings.TrimSpace(b.Text)
		if t == "" {
			issues = append(issues, ValidationIssue{
				Code: "block_empty",
				Path: fmt.Sprintf("blocks[%d].text", i),
				Fact: fmt.Sprintf("第 %d 个 block 文本为空。", i),
			})
			continue
		}
		totalChars += utf8.RuneCountInString(t)
		if b.BlockType == "" {
			// default ok
		}
	}
	// Coverage heuristic: very short docs are allowed but flagged if summary claims pages.
	if pages := ir.Metadata["declared_pages"]; pages != "" && totalChars < 80 {
		issues = append(issues, ValidationIssue{
			Code: "coverage_low",
			Path: "blocks",
			Fact: fmt.Sprintf("声明页数=%s 但文本总长仅 %d 字，覆盖度偏低。", pages, totalChars),
		})
	}
	// Summary optional; if present check length band
	if s := strings.TrimSpace(ir.Summary); s != "" {
		n := utf8.RuneCountInString(s)
		if n < 8 {
			issues = append(issues, ValidationIssue{
				Code: "summary_too_short",
				Path: "summary",
				Fact: "summary 过短（<8 字）。",
			})
		}
		if n > 4000 {
			issues = append(issues, ValidationIssue{
				Code: "summary_too_long",
				Path: "summary",
				Fact: "summary 过长（>4000 字）。",
			})
		}
	}
	for i, t := range ir.KG {
		if strings.TrimSpace(t.Subject) == "" || strings.TrimSpace(t.Predicate) == "" || strings.TrimSpace(t.Object) == "" {
			issues = append(issues, ValidationIssue{
				Code: "kg_shape",
				Path: fmt.Sprintf("kg[%d]", i),
				Fact: "KG 三元组 subject/predicate/object 均须非空。",
			})
		}
	}
	return issues
}

// Normalize fills defaults (block ids, types, schema version).
func Normalize(ir *DocumentIr) {
	if ir.SchemaVersion == "" {
		ir.SchemaVersion = SchemaVersion
	}
	if ir.DocType == "" {
		ir.DocType = "text"
	}
	if ir.PrimaryBackend == "" {
		ir.PrimaryBackend = "agent_package"
	}
	if ir.Metadata == nil {
		ir.Metadata = map[string]string{}
	}
	for i := range ir.Blocks {
		if ir.Blocks[i].BlockID == "" {
			ir.Blocks[i].BlockID = fmt.Sprintf("b%d", i+1)
		}
		if ir.Blocks[i].BlockType == "" {
			ir.Blocks[i].BlockType = "paragraph"
		}
		ir.Blocks[i].Text = strings.TrimSpace(ir.Blocks[i].Text)
	}
}
