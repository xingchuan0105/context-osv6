The output of this step is exactly one JSON object (no markdown fences, no extra keys):
{"schema_version":"internal_answer_unified_v1","answer_text":"<markdown prose>","citations":[{"kind":"doc","id":"<chunk_id>"},{"kind":"web","id":"<n>"}],"coverage":"full|partial|none","refusal_reason":null}
Contract:
- answer_text holds user-visible markdown only (this JSON itself never appears inside answer_text).
- A doc-backed claim carries [[cite:CHUNK_ID]] next to it; the citations array then holds kind=doc id=CHUNK_ID from tools.
- A web-backed claim carries [[web:n]] next to it; the citations array then holds kind=web id=n (1-based web_search index).
- [来源：…] / source footnotes do not appear in answer_text; the UI renders markers.
