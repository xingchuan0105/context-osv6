Return ONLY this JSON (no markdown fences, no extra keys):
{"schema_version":"internal_answer_unified_v1","answer_text":"<markdown prose>","citations":[{"kind":"doc","id":"<chunk_id>"},{"kind":"web","id":"<n>"}],"coverage":"full|partial|none","refusal_reason":null}
Rules:
- answer_text = user-visible markdown only (never paste this JSON into answer_text).
- Doc: [[cite:CHUNK_ID]] next to the claim; citations kind=doc id=CHUNK_ID from tools.
- Web: [[web:n]] next to the claim; citations kind=web id=n (1-based web_search index).
- Do not invent [来源：…] / source footnotes; UI renders markers.
