package retrieval

import "testing"

func TestQueryCardNormalizeDropsWebFromRequired(t *testing.T) {
	c := QueryCard{
		WorkspaceID:     "w1",
		RequiredActions: []string{"dense", "web", "lexical", "web"},
		Mode:            "open",
	}
	if err := c.ValidateShape(); err != nil {
		t.Fatal(err)
	}
	if !c.WebIntent {
		t.Fatal("expected WebIntent from web action")
	}
	if len(c.RequiredActions) != 2 {
		t.Fatalf("actions=%v", c.RequiredActions)
	}
}

func TestExplicitModeMismatch(t *testing.T) {
	c := QueryCard{
		WorkspaceID:     "w1",
		RequiredActions: []string{"dense"},
		Mode:            "explicit",
		ExplicitOptions: &ExplicitOptions{
			WorkspaceID:     "w2",
			RequiredActions: []string{"dense"},
		},
	}
	if err := c.ValidateShape(); err == nil {
		t.Fatal("expected mismatch error")
	}
}

func TestExtractAliases(t *testing.T) {
	got := extractAliases("见 #1 与 #12，重复 #1")
	if len(got) != 2 || got[0] != "#1" || got[1] != "#12" {
		t.Fatalf("got %v", got)
	}
}
