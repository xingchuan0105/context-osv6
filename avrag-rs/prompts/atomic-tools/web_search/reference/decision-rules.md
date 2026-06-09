# Decision Rules

## When `web_search` is the right tool

- The user asks for information that is likely newer than the training cutoff.
- The user asks about current events, breaking news, or recent announcements.
- The user asks about sports scores, stock prices, or other real-time data.
- The user asks about a person, company, or product whose details may have changed recently.
- In Search mode, `web_search` is the only primary retrieval channel.

## When to prefer a different tool

- **Information already in workspace documents** → RAG tools (`dense_retrieval`, `lexical_retrieval`, etc.). Do not search the web for data the user has already uploaded.
- **Simple math or data analysis** → `calculator` or `code_interpreter`.
- **Current weather** → `weather_query`. Web search may return stale or generic weather pages.
- **Questions answerable from training data with high confidence** → answer directly without tool call to save latency and cost.

## Query rewriting best practices

`web_search` expects keyword-rich, search-engine-ready queries.
Do not pass raw conversational text. See `reference/examples.md`
for before/after examples.

- **Strip conversational filler**: "Can you tell me...",
  "Hey, I was wondering..." → remove entirely.
- **Add keywords for specificity**: "latest" → "latest release
  date"; "rust" → "Rust latest release version".
- **Use English for technical topics** when the provider performs
  better in English.
- **For news, include a time window keyword**: "today",
  "this week", "this year".
- **Do not copy the user's question verbatim** — search engines
  expect standalone keywords, not full sentences with question
  marks.

## Interaction with other tools

- **In Chat mode**, `web_search` is an optional atomic tool. Only
call it when the query clearly needs external data.
- **In RAG mode**, `web_search` can supplement document retrieval
when the uploaded docs lack the answer.
- **In Search mode**, `web_search` is the only primary tool. The
Search strategy decomposes queries into sub-queries and calls
`web_search` in parallel.
- **With `web_fetch`**: `web_search` discovers URLs;
`web_fetch` retrieves the full page content. Typical
workflow: search first to find the URL, then fetch to read
the article. Do NOT call `web_fetch` without a known,
fully-qualified URL.
