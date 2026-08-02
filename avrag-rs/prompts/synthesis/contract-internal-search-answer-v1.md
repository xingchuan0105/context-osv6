The output of this step is exactly one JSON object (no markdown fences):
{"schema_version":"internal_search_answer_v1","answer_text":"...","citations":[{"index":1}],"coverage":"full","refusal_reason":null}
Each citation appears as [[n]] in answer_text, matching citations[].index from search observations.
