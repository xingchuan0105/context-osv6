package agentd

import (
	"strings"
	"testing"
)

func TestFilterOutboundClean(t *testing.T) {
	g := FilterOutbound("根据材料，结论是 2019 年建厂。")
	if g.Blocked || g.Text == "" {
		t.Fatalf("%+v", g)
	}
}

func TestFilterOutboundProtocol(t *testing.T) {
	g := FilterOutbound(`<dsml:function_calls><invoke name="search">`)
	if !g.Blocked || g.Reason != "protocol_residue" {
		t.Fatalf("%+v", g)
	}
}

func TestFilterOutboundToolJSON(t *testing.T) {
	raw := "```json\n{\"tool_calls\":[{\"name\":\"dense\"}]}\n```\n然后正常回答在这里。"
	g := FilterOutbound(raw)
	if g.Blocked {
		t.Fatalf("expected salvage, got %+v", g)
	}
	if g.Text == "" || !strings.Contains(g.Text, "正常回答") {
		t.Fatalf("text=%q", g.Text)
	}
}
