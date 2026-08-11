// retrieval-eval scores osv7 index against golden_set_realistic (full-149).
//
// Modes:
//
//	available — only cases whose source_chunks needles exist in local rag_text_chunks
//	all       — every retrieval-eligible case (most miss if corpus not ingested)
//
// Layer A only: no product LLM agent. Tools: lexical / dense / grep (merged stream).
package main

import (
	"context"
	"encoding/json"
	"flag"
	"fmt"
	"log"
	"os"
	"path/filepath"
	"strings"
	"time"
	"unicode/utf8"

	"github.com/context-os/osv7/internal/config"
	"github.com/context-os/osv7/internal/index"
	"github.com/context-os/osv7/internal/store"
	"github.com/jackc/pgx/v5/pgxpool"
)

type goldenFile struct {
	Version string         `json:"version"`
	Subsets []goldenSubset `json:"subsets"`
}

type goldenSubset struct {
	Name     string          `json:"name"`
	Examples []goldenExample `json:"examples"`
}

type goldenExample struct {
	Query           string        `json:"query"`
	ExpectedAnswer  string        `json:"expected_answer"`
	SourceChunks    []sourceChunk `json:"source_chunks"`
	Mode            string        `json:"mode"`
	Capabilities    []string      `json:"capabilities"`
	RequiresNetwork bool          `json:"requires_network"`
	Description     string        `json:"description"`
}

type sourceChunk struct {
	Type string `json:"type"`
	Text string `json:"text"`
}

type caseResult struct {
	Subset         string   `json:"subset"`
	Query          string   `json:"query"`
	WorkspaceID    string   `json:"workspace_id,omitempty"`
	GoldenCount    int      `json:"golden_count"`
	Matched        int      `json:"matched"`
	Recall         float64  `json:"recall"`
	Hit            bool     `json:"hit"`
	RetrievedCount int      `json:"retrieved_count"`
	Tools          []string `json:"tools"`
	MatchedNeedles []string `json:"matched_needles,omitempty"`
	MissingNeedles []string `json:"missing_needles,omitempty"`
	Error          string   `json:"error,omitempty"`
	Skipped        bool     `json:"skipped,omitempty"`
	SkipReason     string   `json:"skip_reason,omitempty"`
	DurationMs     int64    `json:"duration_ms"`
}

type report struct {
	GeneratedAt string       `json:"generated_at"`
	GoldenPath  string       `json:"golden_path"`
	Mode        string       `json:"mode"`
	Tools       []string     `json:"tools"`
	K           int          `json:"k"`
	Eligible    int          `json:"eligible"`
	Ran         int          `json:"ran"`
	Skipped     int          `json:"skipped"`
	Hits        int          `json:"hits"`
	HitRate     float64      `json:"hit_rate"`
	MeanRecall  float64      `json:"mean_recall"`
	CorpusNote  string       `json:"corpus_note"`
	Cases       []caseResult `json:"cases"`
}

func main() {
	log.SetFlags(log.LstdFlags | log.Lmsgprefix)
	log.SetPrefix("retrieval-eval: ")

	goldenPath := flag.String("golden", defaultGolden(), "path to golden_set_realistic.json")
	mode := flag.String("mode", "available", "available | all")
	toolsFlag := flag.String("tools", "lexical,dense", "comma list: lexical,dense,grep")
	k := flag.Int("k", 15, "top-k per tool before merge")
	outPath := flag.String("out", "", "write JSON report path")
	limit := flag.Int("limit", 0, "max cases to run (0=all matching mode)")
	failBelow := flag.Float64("fail-below", 0.5, "exit 2 if available-mode hit_rate below this (0=disable)")
	flag.Parse()

	tools := splitCSV(*toolsFlag)
	cfg, err := config.Load()
	if err != nil {
		log.Fatal(err)
	}
	ctx := context.Background()
	pool, err := store.Connect(ctx, cfg.DatabaseURL)
	if err != nil {
		log.Fatal(err)
	}
	defer pool.Close()

	emb := index.NewEmbedder(cfg.EmbedBaseURL, cfg.EmbedAPIKey, cfg.EmbedModel, cfg.EmbedDim, cfg.EmbedTimeout)
	if contains(tools, "dense") && !emb.Enabled() {
		log.Fatal("dense requested but embedding not configured")
	}
	ix := index.New(pool, emb)

	raw, err := os.ReadFile(*goldenPath)
	if err != nil {
		log.Fatal(err)
	}
	var gf goldenFile
	if err := json.Unmarshal(raw, &gf); err != nil {
		log.Fatal(err)
	}

	type item struct {
		subset  string
		ex      goldenExample
		needles []string
	}
	var eligible []item
	for _, s := range gf.Subsets {
		for _, e := range s.Examples {
			if !isRetrievalEligible(e) {
				continue
			}
			needles := substringNeedles(e)
			if len(needles) == 0 {
				continue
			}
			eligible = append(eligible, item{subset: s.Name, ex: e, needles: needles})
		}
	}

	rep := report{
		GeneratedAt: time.Now().UTC().Format(time.RFC3339),
		GoldenPath:  *goldenPath,
		Mode:        *mode,
		Tools:       tools,
		K:           *k,
		Eligible:    len(eligible),
		CorpusNote:  "Local rag_text_chunks may not hold full-149 corpus. mode=available keeps only cases with ≥1 needle present.",
	}

	ran := 0
	for _, it := range eligible {
		if *limit > 0 && ran >= *limit {
			break
		}
		ws, _, err := bestWorkspace(ctx, pool.Pool, it.needles)
		if err != nil {
			log.Fatal(err)
		}
		if *mode == "available" && ws == "" {
			rep.Skipped++
			rep.Cases = append(rep.Cases, caseResult{
				Subset:      it.subset,
				Query:       it.ex.Query,
				GoldenCount: len(it.needles),
				Skipped:     true,
				SkipReason:  "no source_chunk needle present in local rag_text_chunks",
			})
			continue
		}

		start := time.Now()
		cr := caseResult{
			Subset:      it.subset,
			Query:       it.ex.Query,
			WorkspaceID: ws,
			GoldenCount: len(it.needles),
			Tools:       tools,
		}
		if ws == "" {
			ws2, err := largestWorkspace(ctx, pool.Pool)
			if err != nil {
				cr.Error = err.Error()
				cr.DurationMs = time.Since(start).Milliseconds()
				rep.Cases = append(rep.Cases, cr)
				ran++
				continue
			}
			ws = ws2
			cr.WorkspaceID = ws
			cr.SkipReason = "needle absent; searched largest workspace (expect miss)"
		}

		opts := index.SearchOpts{WorkspaceID: ws, Limit: *k}
		var hits []index.Hit
		for _, tool := range tools {
			var part []index.Hit
			var err error
			switch tool {
			case "lexical":
				part, err = ix.Lexical(ctx, it.ex.Query, opts)
			case "dense":
				part, err = ix.Dense(ctx, it.ex.Query, opts)
			case "grep":
				part, err = ix.Grep(ctx, runePrefix(it.ex.Query, 24), opts)
			default:
				err = fmt.Errorf("unknown tool %s", tool)
			}
			if err != nil {
				cr.Error = err.Error()
				break
			}
			hits = append(hits, part...)
		}
		if cr.Error != "" {
			cr.DurationMs = time.Since(start).Milliseconds()
			rep.Cases = append(rep.Cases, cr)
			ran++
			continue
		}

		texts := hitTexts(hits)
		cr.RetrievedCount = len(texts)
		matched, missing := matchNeedles(it.needles, texts)
		cr.Matched = len(matched)
		cr.MatchedNeedles = matched
		cr.MissingNeedles = missing
		if cr.GoldenCount > 0 {
			cr.Recall = float64(cr.Matched) / float64(cr.GoldenCount)
		}
		cr.Hit = cr.Matched > 0
		cr.DurationMs = time.Since(start).Milliseconds()
		if cr.Hit {
			rep.Hits++
		}
		rep.MeanRecall += cr.Recall
		rep.Cases = append(rep.Cases, cr)
		ran++
		log.Printf("[%d] hit=%v recall=%.2f n=%d subset=%s q=%q",
			ran, cr.Hit, cr.Recall, cr.RetrievedCount, it.subset, trunc(it.ex.Query, 40))
	}
	rep.Ran = ran
	if ran > 0 {
		rep.HitRate = float64(rep.Hits) / float64(ran)
		rep.MeanRecall /= float64(ran)
	}

	b, _ := json.MarshalIndent(rep, "", "  ")
	fmt.Println(string(b))
	if *outPath != "" {
		if err := os.MkdirAll(filepath.Dir(*outPath), 0o755); err != nil {
			log.Fatal(err)
		}
		if err := os.WriteFile(*outPath, b, 0o644); err != nil {
			log.Fatal(err)
		}
		log.Printf("wrote %s", *outPath)
	}

	// Human summary line
	log.Printf("SUMMARY mode=%s eligible=%d ran=%d skipped=%d hits=%d hit_rate=%.3f mean_recall=%.3f",
		rep.Mode, rep.Eligible, rep.Ran, rep.Skipped, rep.Hits, rep.HitRate, rep.MeanRecall)

	if *mode == "available" && *failBelow > 0 && ran > 0 && rep.HitRate+1e-9 < *failBelow {
		log.Printf("FAIL hit_rate=%.3f < fail-below=%.3f", rep.HitRate, *failBelow)
		os.Exit(2)
	}
}

func defaultGolden() string {
	candidates := []string{
		filepath.Join("..", "avrag-rs", "tests", "rag_quality", "golden_set_realistic.json"),
		filepath.Join("avrag-rs", "tests", "rag_quality", "golden_set_realistic.json"),
	}
	for _, c := range candidates {
		if _, err := os.Stat(c); err == nil {
			return c
		}
	}
	return candidates[0]
}

func isRetrievalEligible(e goldenExample) bool {
	if len(e.SourceChunks) == 0 {
		return false
	}
	mode := strings.ToLower(e.Mode)
	if mode == "" {
		mode = "rag"
	}
	if mode == "chat" {
		return false
	}
	if len(e.Capabilities) == 1 && e.Capabilities[0] == "chat" {
		return false
	}
	if e.RequiresNetwork && len(e.Capabilities) > 0 && !hasCap(e.Capabilities, "rag") && mode != "rag" {
		return false
	}
	return true
}

func hasCap(caps []string, want string) bool {
	for _, c := range caps {
		if c == want {
			return true
		}
	}
	return false
}

func substringNeedles(e goldenExample) []string {
	var out []string
	for _, ch := range e.SourceChunks {
		if ch.Type == "substring" && strings.TrimSpace(ch.Text) != "" {
			out = append(out, ch.Text)
		}
	}
	return out
}

func bestWorkspace(ctx context.Context, pool *pgxpool.Pool, needles []string) (string, int, error) {
	counts := map[string]int{}
	present := 0
	for _, text := range needles {
		rows, err := pool.Query(ctx,
			`SELECT DISTINCT workspace_id::text FROM rag_text_chunks
			 WHERE position($1 in text) > 0 AND workspace_id IS NOT NULL`,
			text,
		)
		if err != nil {
			return "", 0, err
		}
		found := false
		for rows.Next() {
			var ws string
			if err := rows.Scan(&ws); err != nil {
				rows.Close()
				return "", 0, err
			}
			counts[ws]++
			found = true
		}
		rows.Close()
		if found {
			present++
		}
	}
	if len(counts) == 0 {
		return "", present, nil
	}
	best, bestN := "", -1
	for ws, n := range counts {
		if n > bestN || (n == bestN && ws < best) {
			best, bestN = ws, n
		}
	}
	return best, present, nil
}

func largestWorkspace(ctx context.Context, pool *pgxpool.Pool) (string, error) {
	var ws string
	err := pool.QueryRow(ctx, `
SELECT workspace_id::text FROM rag_text_chunks
WHERE workspace_id IS NOT NULL
GROUP BY workspace_id ORDER BY count(*) DESC LIMIT 1`).Scan(&ws)
	return ws, err
}

func hitTexts(hits []index.Hit) []string {
	seen := map[string]struct{}{}
	var out []string
	for _, h := range hits {
		t := h.Text
		if t == "" {
			t = h.Snippet
		}
		if t == "" {
			continue
		}
		if _, ok := seen[t]; ok {
			continue
		}
		seen[t] = struct{}{}
		out = append(out, t)
	}
	return out
}

func matchNeedles(needles, texts []string) (matched, missing []string) {
	for _, n := range needles {
		ok := false
		for _, t := range texts {
			if chunkMatches(n, t) {
				ok = true
				break
			}
		}
		if ok {
			matched = append(matched, n)
		} else {
			missing = append(missing, n)
		}
	}
	return matched, missing
}

func chunkMatches(golden, content string) bool {
	if golden == "" || content == "" {
		return false
	}
	if strings.Contains(content, golden) || strings.Contains(golden, content) {
		return true
	}
	prefix := runePrefix(golden, 80)
	if utf8.RuneCountInString(prefix) >= 8 && strings.Contains(content, prefix) {
		return true
	}
	return false
}

func splitCSV(s string) []string {
	var out []string
	for _, p := range strings.Split(s, ",") {
		p = strings.TrimSpace(p)
		if p != "" {
			out = append(out, p)
		}
	}
	return out
}

func contains(ss []string, x string) bool {
	for _, s := range ss {
		if s == x {
			return true
		}
	}
	return false
}

func runePrefix(s string, n int) string {
	r := []rune(s)
	if len(r) <= n {
		return s
	}
	return string(r[:n])
}

func trunc(s string, n int) string {
	r := []rune(s)
	if len(r) <= n {
		return s
	}
	return string(r[:n]) + "…"
}
