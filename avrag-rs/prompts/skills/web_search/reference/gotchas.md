# Gotchas

## Results quality depends on the search provider

The `web_search` tool delegates to a configured search provider (Brave, etc.). If no provider is configured or the API key is invalid, the call returns an error. Always handle the error case gracefully.

## The `vertical` parameter only supports "web" and "news"

Any other value silently falls back to `"web"`. Do not invent new verticals like "images" or "videos" — they are not supported.

## Always rewrite the query

Do not pass the user's raw conversational text as the `query`. Search engines expect keyword-rich, standalone queries. Rewrite:
- User: "What's the latest version of Rust?"
- Query: `"Rust latest release version 2025"`

## No guarantee of recency

Web search results reflect what the search provider has indexed. Very recent content (minutes to hours old) may not yet be indexed. For ultra-breaking news, combine `web_search` with `vertical: "news"`.

## Synthesized answer may be absent

The `synthesized_answer` field is provider-dependent. Brave LLM Context may populate it; basic Brave Search may not. Do not rely on it — always check the `results` array.

## Citation indices are 1-based

Results are numbered starting from 1. Maintain these indices when citing in your answer.

## Empty query returns Error

A missing or empty `query` field returns `ToolStatus::Error` with `"missing query"`.
