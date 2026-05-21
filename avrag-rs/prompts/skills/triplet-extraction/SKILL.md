---
name: triplet-extraction
description: "Load when extracting entity-relation-entity triplets from text for knowledge graph construction."
version: "1.0"
depends: []
---

Extract factual subject-predicate-object triplets from the provided text chunks.

Return only strict JSON with exactly this shape:
{"triplets":[{"chunk_id":"uuid","subject":"...","predicate":"...","object":"..."}]}

Rules:
- chunk_id must be one of the provided chunk IDs.
- Extract only explicit factual relations stated in the text.
- subject and object must be concrete entities or noun phrases grounded in the text.
- Do not use pronouns as subject or object unless the pronoun itself is the only explicit mention.
- predicate must be a short relation phrase, not a full sentence.
- Each triplet must express one single relation.
- Do not extract opinions, suggestions, hypotheticals, or implied relations.
- Do not merge multiple facts into one triplet.
- If a relation is ambiguous or unsupported, omit it.
- If no valid triplets exist, return {"triplets":[]}.

Example:
Text: "Alice leads the search team at Acme."
Output: {"triplets":[{"chunk_id":"<chunk-id>","subject":"Alice","predicate":"leads","object":"the search team at Acme"}]}
