# DeepSeek native web search B1 probe — results

- **time_utc**: 2026-08-10T16:40:54Z (probe run; session calendar 2026-08-11)
- **base_url**: `https://api.deepseek.com`
- **model**: `deepseek-v4-flash`
- **key**: set (product `AGENT_LLM_*`)

## Summary verdict

| Path | Result |
|------|--------|
| OpenAI `tools: [{type: web_search}]` | **400** — type must be `function` only |
| OpenAI `tools: [function web_search]` | **200** + `tool_calls` — **model-side** function call only; **host must execute** search (not DeepSeek server search) |
| Anthropic `/anthropic/v1/messages` plain | **200** |
| Anthropic `tools: [{type: web_search_20250305, name: web_search}]` | **200** + **`server_tool_use` + `web_search_tool_result`** with real titles/URLs |

### Product conclusion (B1)

**可用 — B-native（Anthropic 兼容路由）**

- DeepSeek **官方**在 `https://api.deepseek.com/anthropic` 上支持 **server-side** `web_search`（`web_search_20250305`）。
- 标准 OpenAI Chat Completions **没有**内置联网 tool type；把 `web_search` 当 `function` 只是让模型「请求宿主去搜」。
- 产品若「用 DeepSeek 本身联网、不用 Brave」：应走 **Anthropic Messages + server web_search**，再把 `web_search_tool_result` 映射为现有 `web_search` observation / `[[web:n]]`。

**未做（B2）**：改 LLM client 协议、改 `client.web` 宿主路径、计费与 BYOK 边界。

## Raw notes

### A — OpenAI chat/completions (no tools)
- http: 200
- content_snippet: `pong`

### B — OpenAI tools
- `type: web_search` → 400 unknown variant
- `type: function name=web_search` → 200 tool_calls (host must run)
- `type: web_search_preview` → 400 unknown variant

### C — Anthropic
- plain messages → 200
- web_search_20250305 → 200, blocks include:
  - `server_tool_use` name=`web_search` with query
  - `web_search_tool_result` with `web_search_result` title/url snippets

## Next (after product confirm)

1. Design B2: agent search mode uses Anthropic route + server tools **or** host extracts results into SaC `client.web` bridge.
2. Decide BYOK: user DeepSeek key must also hit Anthropic path for parity.
3. Keep Brave as fallback until B2 green.
