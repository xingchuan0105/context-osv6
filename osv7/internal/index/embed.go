package index

import (
	"bytes"
	"context"
	"encoding/json"
	"fmt"
	"io"
	"net/http"
	"time"
)

// Embedder calls OpenAI-compatible /embeddings (SiliconFlow etc.).
type Embedder struct {
	baseURL string
	apiKey  string
	model   string
	dim     int
	client  *http.Client
}

func NewEmbedder(baseURL, apiKey, model string, dim, timeoutMs int) *Embedder {
	if timeoutMs <= 0 {
		timeoutMs = 15000
	}
	return &Embedder{
		baseURL: baseURL,
		apiKey:  apiKey,
		model:   model,
		dim:     dim,
		client:  &http.Client{Timeout: time.Duration(timeoutMs) * time.Millisecond},
	}
}

func (e *Embedder) Enabled() bool {
	return e != nil && e.baseURL != "" && e.apiKey != "" && e.model != ""
}

func (e *Embedder) Embed(ctx context.Context, text string) ([]float32, error) {
	if !e.Enabled() {
		return nil, fmt.Errorf("embedder disabled")
	}
	body := map[string]any{
		"model": e.model,
		"input": text,
	}
	// Some providers accept dimensions; leave unset when 0.
	raw, _ := json.Marshal(body)
	url := e.baseURL + "/embeddings"
	req, err := http.NewRequestWithContext(ctx, http.MethodPost, url, bytes.NewReader(raw))
	if err != nil {
		return nil, err
	}
	req.Header.Set("Content-Type", "application/json")
	req.Header.Set("Authorization", "Bearer "+e.apiKey)

	res, err := e.client.Do(req)
	if err != nil {
		return nil, err
	}
	defer res.Body.Close()
	b, _ := io.ReadAll(res.Body)
	if res.StatusCode < 200 || res.StatusCode >= 300 {
		return nil, fmt.Errorf("embed HTTP %d: %s", res.StatusCode, truncate(string(b), 300))
	}
	var parsed struct {
		Data []struct {
			Embedding []float32 `json:"embedding"`
		} `json:"data"`
	}
	if err := json.Unmarshal(b, &parsed); err != nil {
		return nil, err
	}
	if len(parsed.Data) == 0 || len(parsed.Data[0].Embedding) == 0 {
		return nil, fmt.Errorf("embed: empty vector")
	}
	return parsed.Data[0].Embedding, nil
}

func truncate(s string, n int) string {
	if len(s) <= n {
		return s
	}
	return s[:n]
}
