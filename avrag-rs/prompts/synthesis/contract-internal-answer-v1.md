The output of this step is exactly one JSON object (no markdown fences):
{"schema_version":"internal_answer_v1","answer_text":"prose with [[cite:CHUNK_ID]]","citations":[{"chunk_id":"..."}],"coverage":"full","refusal_reason":null}
Every citations[].chunk_id appears as [[cite:CHUNK_ID]] in answer_text.
