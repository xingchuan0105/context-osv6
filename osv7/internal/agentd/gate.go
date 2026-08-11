package agentd

import (
	"regexp"
	"strings"
)

// Outbound gate: user bubble only sees natural language final text.
// Blocks protocol residue / tool transcripts (L0 "唯一必须带进新架构的护栏").

var (
	reDSML       = regexp.MustCompile(`(?is)</?dsml[^>]*>|invoke\s+name=|function_calls?`)
	reToolFence  = regexp.MustCompile("(?is)```(?:json|tool|xml)?\\s*\\{[^`]{0,200}\"tool_calls\"")
	reToolCall   = regexp.MustCompile(`(?is)\btool_call(s)?\b\s*[:=\[{]`)
	reMCPNoise   = regexp.MustCompile(`(?is)\[tool_execution_|mcp__|/tool_use|</?tool_`)
	reSelected   = regexp.MustCompile(`(?m)^SELECTED:\s*#\d+`)
	reKeepLine   = regexp.MustCompile(`(?m)^KEEP\b`)
)

// GateResult is the filtered user-visible payload.
type GateResult struct {
	// Text is what may enter the user bubble (may be empty if fully blocked).
	Text string `json:"text"`
	// Blocked is true when original content was rejected or heavily stripped.
	Blocked bool `json:"blocked"`
	// Reason is machine-stable when Blocked.
	Reason string `json:"reason,omitempty"`
	// Raw kept for telemetry only (never UI).
	Raw string `json:"-"`
}

// FilterOutbound applies the thin outbound gate to assistant final text.
func FilterOutbound(raw string) GateResult {
	raw = strings.TrimSpace(raw)
	if raw == "" {
		return GateResult{Text: "", Blocked: false, Raw: raw}
	}
	if looksLikeProtocol(raw) {
		// Try salvage: strip obvious protocol blocks, keep remaining prose.
		salvaged := stripProtocol(raw)
		// DSML / tool call shells rarely salvage to a good user bubble; require
		// substantial remaining prose (CJK or long natural language).
		if salvaged == "" || looksLikeProtocol(salvaged) || !salvageOK(raw, salvaged) {
			return GateResult{
				Text:    "",
				Blocked: true,
				Reason:  "protocol_residue",
				Raw:     raw,
			}
		}
		return GateResult{Text: salvaged, Blocked: false, Reason: "stripped", Raw: raw}
	}
	return GateResult{Text: raw, Blocked: false, Raw: raw}
}

func salvageOK(raw, salvaged string) bool {
	runes := []rune(strings.TrimSpace(salvaged))
	if len(runes) < 4 {
		return false
	}
	// Prefer CJK prose or long ASCII sentence after stripping.
	cjk := 0
	letters := 0
	for _, r := range runes {
		if r >= 0x4e00 && r <= 0x9fff {
			cjk++
		}
		if (r >= 'a' && r <= 'z') || (r >= 'A' && r <= 'Z') {
			letters++
		}
	}
	if cjk >= 4 {
		return true
	}
	if letters >= 24 && len(runes) >= 40 {
		return true
	}
	return false
}

func looksLikeProtocol(s string) bool {
	if reDSML.MatchString(s) {
		return true
	}
	if reToolFence.MatchString(s) {
		return true
	}
	if reToolCall.MatchString(s) {
		return true
	}
	if reMCPNoise.MatchString(s) {
		return true
	}
	// Entire message is only SELECTED/KEEP lines
	lines := strings.Split(s, "\n")
	onlyMeta := true
	for _, ln := range lines {
		ln = strings.TrimSpace(ln)
		if ln == "" {
			continue
		}
		if reSelected.MatchString(ln) || reKeepLine.MatchString(ln) {
			continue
		}
		onlyMeta = false
		break
	}
	if onlyMeta && (reSelected.MatchString(s) || reKeepLine.MatchString(s)) {
		return true
	}
	return false
}

func stripProtocol(s string) string {
	s = reDSML.ReplaceAllString(s, "")
	s = reMCPNoise.ReplaceAllString(s, "")
	// Drop fenced tool JSON blocks (rough)
	s = regexp.MustCompile("(?s)```[^`]*```").ReplaceAllStringFunc(s, func(block string) string {
		if strings.Contains(strings.ToLower(block), "tool_call") || strings.Contains(block, "function_call") {
			return ""
		}
		return block
	})
	return strings.TrimSpace(s)
}
