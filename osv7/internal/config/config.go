package config

import (
	"fmt"
	"os"
	"strconv"
	"strings"
)

// Config is process-level env for osv7 retrieval-mcp (P1).
type Config struct {
	DatabaseURL string

	// Optional identity for resource scope (P1: from env / transport headers later).
	DefaultUserID string

	EmbedBaseURL string
	EmbedAPIKey  string
	EmbedModel   string
	EmbedDim     int
	EmbedTimeout int // ms

	// Listen: empty = stdio only; e.g. ":8081" enables Streamable HTTP alongside or instead.
	HTTPAddr string
}

func Load() (Config, error) {
	c := Config{
		DatabaseURL:   strings.TrimSpace(os.Getenv("DATABASE_URL")),
		DefaultUserID: strings.TrimSpace(firstEnv("OSV7_USER_ID", "AUTH_USER_ID")),
		EmbedBaseURL:  strings.TrimRight(strings.TrimSpace(firstEnv("EMBEDDING_BASE_URL", "OPENAI_BASE_URL")), "/"),
		EmbedAPIKey:   strings.TrimSpace(firstEnv("EMBEDDING_API_KEY", "OPENAI_API_KEY")),
		EmbedModel:    strings.TrimSpace(firstEnv("EMBEDDING_MODEL", "OPENAI_EMBEDDING_MODEL")),
		EmbedDim:      intEnv("AVRAG_EMBEDDING_DIM", 1024),
		EmbedTimeout:  intEnv("EMBEDDING_TIMEOUT_MS", 15000),
		HTTPAddr:      strings.TrimSpace(os.Getenv("OSV7_MCP_HTTP_ADDR")),
	}
	if c.DatabaseURL == "" {
		return c, fmt.Errorf("DATABASE_URL is required")
	}
	if c.EmbedModel == "" {
		c.EmbedModel = "Pro/BAAI/bge-m3"
	}
	return c, nil
}

func firstEnv(keys ...string) string {
	for _, k := range keys {
		if v := strings.TrimSpace(os.Getenv(k)); v != "" {
			return v
		}
	}
	return ""
}

func intEnv(key string, def int) int {
	v := strings.TrimSpace(os.Getenv(key))
	if v == "" {
		return def
	}
	n, err := strconv.Atoi(v)
	if err != nil {
		return def
	}
	return n
}
