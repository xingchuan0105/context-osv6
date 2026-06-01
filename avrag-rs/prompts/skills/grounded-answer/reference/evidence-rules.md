# Evidence Rules

## Core principle

Only present a claim as fact when it is supported by the retrieved evidence.

## Evidence levels

### Supported
Use confident language when the retrieved material directly states or clearly supports the claim.

### Partially supported
Use qualified language when the evidence is suggestive but incomplete.
Examples:
- "Based on the available materials..."
- "The retrieved documents suggest..."
- "It appears that..."

### Unsupported
If the evidence does not support the claim:
- do not present it as fact
- say the retrieved material does not confirm it
- explain what is missing if helpful

## Scope of evidence

- Use retrieved documents as the source of factual grounding.
- Do not use session history as evidence for document claims.
- Do not use user preference memory to alter evidence-based conclusions.
- Do not answer a rewritten subquery when the user's original question asks something broader or different.

## Fallback marker (system signal)

When the retrieved evidence is insufficient to answer the user's question
and you must respond using your general knowledge (i.e. the documents
do not actually cover the question), include the exact marker
`EVIDENCE_INSUFFICIENT_FALLBACK` somewhere in your response.

The system uses this marker to:
1. Record a `Degraded` trace item so the UI can show "no grounded
   evidence" to the user.
2. Distinguish a "good fallback" (transparently flagged) from a
   "silent hallucination" (the previous bug).

If the evidence is sufficient, answer normally with `[[cite:CHUNK_ID]]`
citations and do NOT include the marker.
