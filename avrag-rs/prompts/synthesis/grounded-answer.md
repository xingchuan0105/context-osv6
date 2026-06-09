---
name: grounded-answer
description: "Use when synthesizing answers from retrieved documents and needing to prevent hallucination by grounding every claim in evidence. Use when the user asks a question that requires RAG-style document-based reasoning. Skip for open-ended creative writing with no retrieval step or pure reasoning tasks with no external evidence."
version: "1.0"
depends: []
---

# Grounded Answer

## Overview

Only present a claim as fact when supported by retrieved evidence.
If evidence is insufficient, flag it transparently.

## When to Use

- Answering questions based on retrieved documents (RAG, retrieval,
  document-grounded, evidence-based synthesis).
- Synthesizing information from search results or index lookups.
- When the user asks "according to the documents", "what do the
  docs say", or similar evidence-seeking phrasing.

When NOT to use:
- Open-ended creative writing with no retrieval step.
- Pure reasoning tasks with no external evidence to ground claims.

## Core Principle

Only present a claim as fact when it is supported by the retrieved evidence.

## Evidence Levels

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

## Scope of Evidence

- Use retrieved documents as the source of factual grounding.
- Do not use session history as evidence for document claims.
- Do not use user preference memory to alter evidence-based conclusions.
- Do not answer a rewritten subquery when the user's original question asks something broader or different.

## Fallback Marker (System Signal)

When the retrieved evidence is insufficient to answer the user's question and you must respond using your general knowledge (i.e. the documents do not actually cover the question), include the exact marker `EVIDENCE_INSUFFICIENT_FALLBACK` somewhere in your response.

The system uses this marker to:
1. Record a `Degraded` trace item so the UI can show "no grounded evidence" to the user.
2. Distinguish a "good fallback" (transparently flagged) from a "silent hallucination" (the previous bug).

## Common Mistakes

| Mistake | Fix |
|---------|-----|
| Using session history as evidence for document claims | Session history = context, not evidence. Only retrieved docs count. |
| Silently falling back to general knowledge without marker | Always include `EVIDENCE_INSUFFICIENT_FALLBACK` when documents don't cover the question. |
| Answering a rewritten subquery when the original question is broader | Stick to the user's original question scope. |
| Presenting an unsupported claim as fact because it "sounds right" | Downgrade to Partially supported or Unsupported per the levels above. |

## Red Flags — STOP and Check

- "I'll just fill in the gaps with what I know..."
- "The user probably wants a complete answer even if docs are thin..."
- "Session context counts as evidence for this claim..."

All of these mean: check the Evidence Levels and Scope of Evidence rules above.
If unsupported, use the fallback marker.
