## Job

Multiple window-level JSON objects for the same document are provided below. Fuse them into **one** document-level JSON with the same schema as a single profile+summary extraction.

**Output:** one single-line JSON only (no fences): `metadata`, `summary`, `sections` (with `overview`, nested `children` optional). No `chunk_id`.

## Fusion observations

- `summary` becomes one coherent document-level summary (not a list of "window 1 / window 2" paste blocks).
- `sections` form one ordered tree; duplicate titles may merge overviews; invented chapters without source support stay absent.
- `metadata` conflicts prefer the more specific/complete side; unsupported guesses stay null or "unknown".
- Numbers, proper names, and stable identifiers present in any window remain visible in the fused result when still accurate.
