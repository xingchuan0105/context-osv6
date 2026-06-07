---
name: triplet-extraction
description: "Load when the document ingestion worker needs to extract entity-relation-entity triplets from text chunks for knowledge graph construction. Triggers automatically per document batch in the ingestion pipeline. Not invoked by chat-plan / rag-plan / search-plan (those emit graph-retrieval queries, not graph-building). Skip for one-off chat messages or unstructured text (this is a knowledge-graph builder, not a general NER tool)."
version: "2.0"
depends: []
category: "ingestion-tool"
risk_level: "low"
---

Extract factual subject-predicate-object triplets from the provided text chunks.

## Input format

You receive:

- A system prompt (this skill).
- A user prompt of the form:
  ```
  Valid chunk IDs: <uuid1>, <uuid2>, ...

  Chunks:
  {"<chunk_id_1>": "<text>", "<chunk_id_2>": "<text>", ...}

  Extract triplets with chunk_id:
  ```

Important runtime facts:

- The chunks are a **batch** of the document, not the full document. Other batches run in parallel and produce their own triplets.
- The runtime temperature is set to **0.1** for determinism. Be concise and literal; do not pad or paraphrase creatively.
- The runtime gives you roughly **3,000 tokens** of chunk text per call. Output should be proportional — typically 1-5 triplets per typical chunk, 10+ for dense chunks. Keep output JSON under ~500 tokens.

## Output schema

Return exactly one raw JSON object with this shape:

```json
{"triplets":[{"chunk_id":"uuid","subject":"...","predicate":"...","object":"..."}]}
```

### Schema note: `chunk_id` is singular, but runtime merges duplicates

The schema accepts a single `chunk_id` per triplet. Internally, the runtime stores triplets as `ExtractedTriplet { supporting_chunk_ids: Vec<Uuid> }` and merges triplets with the **same** `(subject, predicate, object)` into a single record with accumulated chunk IDs.

This means:

- It's safe (and encouraged) to emit the same `(s, p, o)` triplet multiple times across different chunks. The runtime will deduplicate and accumulate the chunk IDs.
- If the same relation appears in multiple chunks in the **same** batch, emit one triplet per chunk with the corresponding `chunk_id`. The runtime will deduplicate.
- **Do NOT try to merge supporting chunks yourself.** Emit one triplet per `(chunk, relation)` pair and let the runtime aggregate.
- The runtime deduplicates by lowercase match of `(subject, predicate, object)`. "Alice" and "alice" are merged, but "Alice" and "Alice Smith" are NOT merged. Emit the same canonical form for the same entity across all chunks.

### Output format (hard requirement)

The runtime parser does **not** strip markdown fences. If you wrap the JSON in ` ```json ... ``` `, the entire response fails to parse and the **whole document's graph construction is marked as degraded**. The user will not be able to search graph relations for that document.

**Output the JSON object as the entire response.** No fences, no preamble, no trailing text, no "Here is the output:".

## Field rules

### `chunk_id`

- Must be one of the UUIDs listed in the user prompt under "Valid chunk IDs".
- Must be a valid UUID format.
- Missing or invalid `chunk_id` → triplet is silently dropped.
- Hallucinated chunk IDs (not in the valid list) → silently dropped.

### `subject` and `object` — entity normalization

- Must be concrete entities or noun phrases grounded in the text.
- Use the **most complete form mentioned in the text**:
  - If the text says "Alice Smith" once and "Alice" twice, prefer "Alice Smith" (the full form).
  - If the text says "Alice" 5 times and "Alice Smith" once, default to "Alice" (the dominant form).
  - If ambiguous ("Ms. Smith" 3 times, "Alice" 3 times), treat them as separate entities.
- **Title prefixes**: include when part of the canonical name ("Dr. Smith", "President Carter"). Skip generic honorifics ("Ms.", "Mr.").
- **Plurals vs singular**: "user" for a single user; "users" only when explicitly plural.
- **Dates and numbers**: include when part of the identity ("Q3 2024 report", not "the report").
- **Articles**: drop them. "team" not "the team"; "book" not "a book".
- **No invented qualifiers**: "Alice" not "Alice (the founder of Acme)" — the qualifier goes in the predicate.
- **No pronouns by default**: replace pronouns with their referent. "She founded Acme" → "Alice founded Acme" (assuming prior context identifies "she" as Alice).
  - Exception: if the pronoun is the **only** explicit mention (no prior reference), keep it. But this is a low-quality triplet; the graph will treat "she" as an anonymous, unresolvable entity.
  - If the pronoun could refer to multiple entities, omit the triplet.

### `predicate`

- **1-4 words** (max 5 in edge cases).
- **Verb-led or preposition-led**: "leads", "founded", "acquired", "is part of", "depends on".
- **No full sentences**: not "is the founder and CEO of" (multiple relations mashed together).
- **No articles / determiners**: "leads team" not "leads the team" (but "leads the search team at Acme" is OK when the specifier distinguishes the entity).
- **No temporal adverbs**: not "currently leads" or "previously founded". Time context goes in the object or is implicit from chunk text.
- **Tense**: present tense preferred for current facts, past tense for historical events. Be consistent.
- **No hedging**: not "may be", "could be", "is supposedly".

Good: "leads", "founded", "is part of", "acquired", "depends on", "manages".
Bad: "is the founder and CEO of", "currently serves as", "used to be responsible for", "is known for being".

## Extraction rules

### One relation per triplet

If a sentence expresses multiple facts, emit one triplet per fact.

✅ "Alice founded Acme in 2010 and led it as CEO":
- `{"subject": "Alice", "predicate": "founded", "object": "Acme"}`
- `{"subject": "Alice", "predicate": "led as CEO", "object": "Acme"}`
- `{"subject": "Acme", "predicate": "founded in", "object": "2010"}`

❌ `{"subject": "Alice", "predicate": "founded and led as CEO in 2010", "object": "Acme"}` (one triplet, three facts mashed together)

### N-ary relations (3+ entities)

The system only supports binary triplets. Decompose into 2+ binary triplets without compound predicates.

✅ "Alice gave Bob a book":
- `{"subject": "Alice", "predicate": "gave", "object": "a book"}`
- `{"subject": "Alice", "predicate": "gave-to", "object": "Bob"}`

✅ "Bob works at Acme in San Francisco":
- `{"subject": "Bob", "predicate": "works at", "object": "Acme in San Francisco"}`
(the location detail goes in the object)

❌ `{"subject": "Alice and Bob", "predicate": "...", "object": "..."}` (lists as subject/object are invalid)
❌ `{"subject": "Alice", "predicate": "gave-to-and-received-by", "object": "..."}` (compound predicates are forbidden)

### What NOT to extract

- **Opinions**: "I think Acme is the best", "Acme seems to be growing".
- **Suggestions**: "Acme should consider X", "we recommend Y".
- **Hypotheticals**: "if Acme acquires X, then ...", "would be the case if...".
- **Implied relations**: "Alice mentioned Bob" is NOT a relation between Alice and Bob; only "Alice works with Bob" is.
- **Negations as positive facts**: "Acme is NOT a bank" — do not extract `Acme-is-a-bank`.
- **Aspirations / plans**: "Alice plans to found Acme" — future/intention, not a current relation.
- **Conditions**: "X is true when Y" — extract X and Y only if both are stated unconditionally elsewhere.

### "Ambiguous" means

- Pronouns without clear referent.
- Multiple possible subjects or objects for the same predicate ("Alice and Bob founded Acme" — if unclear who is primary).
- Predicates that don't grammatically fit ("X is related to Y" — what's the actual relation?).
- Modality ("X could be related to Y", "X may have caused Y") — relation is hypothetical, not asserted.
- Cross-sentence implications with no explicit connection.

**Default to omit when in doubt.** A missing triplet is recoverable at query time; a wrong triplet is not.

### Triplet density

- A typical informative chunk (50-200 words) produces **1-5 triplets**.
- A dense technical chunk may produce 10+ triplets.
- A sparse chunk may produce **0 triplets**. Returning empty is correct.
- **Sign of over-extraction**: triplet count > 1 per sentence — you may be hallucinating.
- **Sign of under-extraction**: 0 triplets for a chunk with multiple proper-noun entities and explicit "X works at Y" / "X is part of Y" patterns.

## Parse failure modes

The runtime parser will **silently drop** triplets in these cases:

- `chunk_id` field missing
- `chunk_id` not a valid UUID
- `chunk_id` not in the valid chunk IDs list (hallucinated chunk_id)
- `subject`, `predicate`, or `object` empty or missing

**Implication**: every triplet you emit costs tokens and may be silently wasted. Validate your output before submitting.

If the **entire response** fails to parse (malformed JSON, markdown fences, missing `triplets` array), the **whole document's graph construction is marked as degraded** and the user cannot search graph relations for that document. This is catastrophic.

### Field empty handling

- `chunk_id`: required valid UUID string. Missing or invalid → triplet dropped.
- `subject` / `predicate` / `object`: non-empty trimmed strings. Empty string → dropped.
- Do NOT use `null` for any field. Use `""` or omit the field entirely.

## Language handling

- Subject / predicate / object strings should be in the same language as the source text.
- For mixed-language documents, the dominant language wins.
- Predicate conventions are language-specific. English: "founded", "leads", "is part of". Chinese: "创立", "领导", "属于". Use verb forms natural to the source text.
- Do NOT translate entities to English if the source is in another language. Preserve canonical names.

## Downstream use

The triplets you extract are stored in the document's knowledge graph. When the user later asks a question involving relationships, the `graph-retrieval` tool (a different skill) takes triplet hints and queries the graph for relations.

**Quality matters**:

- Low-quality or wrong triplets cause wrong relation lookups at query time.
- A missing triplet is recoverable (the query returns no graph relations).
- A wrong triplet causes incorrect answers.
- **Bias toward precision over recall**: when in doubt, omit. Better to under-extract than to introduce noise.

## Determinism

The runtime calls the LLM with **temperature 0.1** (near-zero). The output should be a faithful extraction of what's in the text, not a creative paraphrase. Do not:

- Add facts not in the text.
- Use a paraphrase that loses information ("Acme" → "the company" loses identity).
- Use synonyms that may not match downstream queries (a query for "founded" should match a triplet with predicate "founded", not "started" or "established" — unless the text uses those words).

## Example

**Full input example** (text chunk from a document):

```
Valid chunk IDs: 00000000-0000-0000-0000-000000000001

Chunks:
{"00000000-0000-0000-0000-000000000001": "Alice leads the search team at Acme. The team focuses on retrieval-augmented generation."}

Extract triplets with chunk_id:
```

**Expected output** (entire response, no markdown, no preamble):

```json
{"triplets":[{"chunk_id":"00000000-0000-0000-0000-000000000001","subject":"Alice","predicate":"leads","object":"the search team at Acme"}]}
```

Note: the second sentence about RAG is a fact ABOUT the team but not a relation between two entities — omit it.

If no valid triplets exist, return exactly:

```json
{"triplets":[]}
```
