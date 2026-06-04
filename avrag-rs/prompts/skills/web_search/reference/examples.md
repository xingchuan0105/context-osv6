# Examples

## General web search

```json
{ "query": "Rust latest release version features 2025", "vertical": "web" }
```
Result:
```json
{
  "query_type": "web",
  "sub_queries": ["Rust latest release version features 2025"],
  "results": [
    {
      "title": "Announcing Rust 1.85.0",
      "url": "https://blog.rust-lang.org/2025/02/...",
      "snippet": "The Rust team is happy to announce a new version of Rust, 1.85.0...",
      "citation_index": 1
    }
  ],
  "synthesized_answer": "Rust 1.85.0 was released in February 2025...",
  "llm_usage": null
}
```

## News search

```json
{ "query": "OpenAI GPT-5 announcement", "vertical": "news" }
```
Result: News articles about recent OpenAI announcements, with `citation_index` values.

## Simple factual query

```json
{ "query": "capital of France" }
```
Result: Results confirming Paris is the capital of France.

## Error: missing query

```json
{}
```
Result: `{"error": "missing query"}` (status: Error)

## Error: no provider configured

```json
{ "query": "test" }
```
When no search provider is configured:
Result: `{"error": "search provider not available"}` (status: Error)

## Query rewriting example

User asks: "Hey, can you find out what the weather was like during the Tokyo Olympics in 2021?"

Good query:
```json
{ "query": "Tokyo Olympics 2021 weather conditions summer" }
```

Bad query (do not do this):
```json
{ "query": "Hey, can you find out what the weather was like during the Tokyo Olympics in 2021?" }
```
