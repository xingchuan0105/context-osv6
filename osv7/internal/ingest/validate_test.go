package ingest

import "testing"

func TestValidateEmptyBlocks(t *testing.T) {
	ir := &DocumentIr{Title: "t", Blocks: nil}
	iss := ValidateDocumentIr(ir)
	if len(iss) == 0 {
		t.Fatal("expected issues")
	}
}

func TestValidateOK(t *testing.T) {
	ir := &DocumentIr{
		Title:  "demo",
		Blocks: []BlockIr{{Text: "hello world paragraph for ingest"}},
	}
	Normalize(ir)
	if iss := ValidateDocumentIr(ir); len(iss) != 0 {
		t.Fatalf("%+v", iss)
	}
}

func TestParseTextBytes(t *testing.T) {
	ir := ParseTextBytes("a.md", []byte("第一段\n\n第二段内容比较长一些。"))
	if len(ir.Blocks) < 2 {
		t.Fatalf("blocks=%d", len(ir.Blocks))
	}
}
