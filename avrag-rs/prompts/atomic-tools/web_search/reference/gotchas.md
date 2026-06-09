# Gotchas

## Results quality depends on the search provider

The `web_search` tool delegates to a configured search provider (Brave, etc.). If no provider is configured or the API key is invalid, the call returns an error. Always handle the error case gracefully.

## The `vertical` parameter is strictly `"web"` or `"news"`

Any other value (e.g. `"images"`, `"videos"`, `"maps"`,
`"shopping"`) is rejected by the runtime as an
`INVALID_VERTICAL` error. There is no silent fallback —
the call fails rather than degrading to `"web"` with the
wrong result type. If you need a non-web vertical that
is not supported here, fall back to a domain-specific tool.

## Always rewrite the query

Do not pass the user's raw conversational text as the `query`.
Search engines expect keyword-rich, standalone queries. See
`reference/decision-rules.md` for the full rewriting checklist
and `reference/examples.md` for before/after examples.

## No guarantee of recency

Web search results reflect what the search provider has indexed. Very recent content (minutes to hours old) may not yet be indexed. For ultra-breaking news, combine `web_search` with `vertical: "news"`.

## `synthesized_answer` is provider-dependent and may be null or absent

The `synthesized_answer` field is populated only when the
provider supports answer synthesis (e.g. Brave LLM Context).
For basic Brave Search or when the provider has no synthesis
pipeline, the field is `null` (or absent in older responses).

**Caller rules**:
- Treat `synthesized_answer` as a **convenience hint**, not
  as a citation. It may not correspond 1:1 to the `results`
  array's snippets.
- When the field is non-null, treat it as a draft answer
  that must still be verified against `results` before the
  final user-facing response.
- When the field is null, the planner is fully responsible
  for synthesizing the answer from `results` snippets.

## Citation indices are 1-based and per-call

Each `web_search` call's `results` array is numbered
starting from 1 **independently**. If a plan issues multiple
`web_search` calls (e.g. in Search mode, where queries are
decomposed), the citations `[1]`, `[2]`, ... reset per call.

**When citing across multiple calls in one answer**: prefix
the citation with a call tag (e.g. `[search-1.1]`,
`[search-2.3]`) OR re-number globally by post-processing the
`results` arrays in the order they appear in the plan.
The runtime does NOT do this automatically.

Format guidance: cite as `[1]`, `[2]`, etc. inline, with a
numbered reference list at the end of the answer containing
`title — url`.

Do not paste raw `snippet` text in the answer; paraphrase.
Do not include the search provider name in the citation
unless the user asks.

## Empty or very few results

`results: []` does NOT automatically mean "nothing exists on
this topic." Common causes and recovery:

| Cause | Detection | Action |
|-------|-----------|--------|
| Query is too narrow (over-specific) | High specificity, e.g. exact model + exact date | Drop a token, broaden, retry |
| Query is too broad | Single common word ("rust", "python") | Add qualifiers |
| Time filter mismatch | News query about >1 year ago event | Switch to `vertical: "web"` |
| Provider just returned nothing | Reasonable query, 0 results | Retry once; if persistent, inform the user |
| Provider API error | `status: "error"` returned | See Error codes in `args-schema.md` |

**Rule of thumb**: 0 results from a reasonable query on the
public web is suspicious. Treat persistent `[]` as either a
poorly-formed query or a provider issue, not "no such thing
exists on the internet."

## Missing or empty `query`

A missing or empty `query` field returns an error object (see
Error codes in `args-schema.md`). Do not retry — fix the
caller to provide a non-empty `query` string.
