
This task maintains a long-term user profile for Context OS.

The input is recent conversation turns; the output is a proposed **memory update** (a small delta), not a full rewritten profile.
Scoring, decay, expiration, eviction, and merge rules run later in the pipeline — they are not part of this step.

Input:
- existing user profile (slot-based memory state)
- recent raw conversation turns (user and assistant messages)
- today's date

Output:
The output of this step is exactly one raw JSON object with this schema:

{
  "expertise_domain_updates": [
    {
      "tag": "string",
      "action": "add" | "reinforce" | "revise" | "weaken" | "remove",
      "description": "string (≤ 200 chars)",
      "evidence": ["string (≤ 200 chars each, max 5 items)"],
      "confidence_signal": "weak" | "medium" | "strong"
    }
  ],
  "preferred_answer_style_update": {
    "tag": "concise-writing" | "professional-writing" | "storytelling" | "academic-writing" | "teaching" | "framework-extraction" | null,
    "modifiers": ["concise" | "detailed" | "examples-first" | "socratic"],
    "action": "set" | "reinforce" | "revise" | "weaken" | "clear" | "none",
    "description": "string (≤ 200 chars)",
    "evidence": ["string (≤ 200 chars each, max 5 items)"],
    "confidence_signal": "weak" | "medium" | "strong"
  },
  "preferred_language_update": {
    "value": "zh" | "en" | null,
    "action": "set" | "reinforce" | "weaken" | "clear" | "none",
    "evidence": ["string (≤ 200 chars each, max 5 items)"],
    "confidence_signal": "weak" | "medium" | "strong"
  },
  "tool_preference_updates": [
    {
      "tag": "rag" | "search" | "chat",
      "action": "add" | "reinforce" | "weaken" | "remove",
      "reason": "string (≤ 200 chars)",
      "evidence": ["string (≤ 200 chars each, max 5 items)"],
      "confidence_signal": "weak" | "medium" | "strong"
    }
  ],
  "important_constraint_updates": [
    {
      "tag": "string",
      "action": "add" | "reinforce" | "revise" | "remove",
      "description": "string (≤ 200 chars)",
      "expires_at": "YYYY-MM-DD" | null,
      "evidence": ["string (≤ 200 chars each, max 5 items)"],
      "confidence_signal": "weak" | "medium" | "strong"
    }
  ],
  "session_continuity_hints": [
    {
      "hint": "string (≤ 200 chars)",
      "source_session_id": "string",
      "priority": "low" | "medium" | "high"
    }
  ],
  "observed_conflicts": [
    {
      "field": "string",
      "old_view": "string (≤ 200 chars)",
      "new_view": "string (≤ 200 chars)",
      "evidence": ["string (≤ 200 chars each, max 5 items)"]
    }
  ],
  "global_summary": "string (1-3 sentences, ≤ 400 chars)"
}

## Evidence field format

`evidence` is a list of short verbatim quotes (≤ 200 characters each) copied from the recent session summaries, or session IDs in the form `"sess:YYYY-MM-DD-NNN"`. The runtime uses these to:
- Display to the user "where did we infer this?"
- Backfill if a preference is later contested in `observed_conflicts`

Evidence entries are verbatim copies of the exact text, never paraphrases. A session ID evidence entry carries the `sess:` prefix. Each update carries at most 5 evidence items.

## Privacy constraints

- Direct identifiers (real name, email, phone, address, SSN, government IDs) never appear in any `description`, `reason`, or `evidence` field. The user profile is shared with other agents — it stays categorical, not identifying.
- Aggregation is preferred: "works in backend infrastructure" rather than "works at [Company] on the [Team] in [City]".
- Sensitive PII from a session summary is not stored in the profile. Categorical framing applies even when the source text is specific.

## Expiration semantics

Only `important_constraint_updates` supports `expires_at`. Other slot types are durable until an explicit `remove` action. An `expires_at` on `expertise_domain_updates` or `tool_preference_updates` is ignored by the runtime.

## Rules

- Every proposed update is grounded in the provided session summaries.
- A single weak signal does not produce a durable preference.
- No update is preferred over a speculative update.
- `add`: a new stable trait, preference, constraint, or domain clearly appears.
- `reinforce`: recent sessions support an existing memory item.
- `revise`: the slot remains the same conceptually but its description changes.
- `weaken`: recent sessions suggest the prior memory is less reliable or less active.
- `remove`: recent sessions clearly invalidate an existing memory item.
- `clear` or `none`: no reliable update is proposed for singleton fields.
- Tags stay stable when the underlying preference is the same and only the wording becomes more specific.
- `modifiers` records secondary traits that combine with the primary tag. The canonical modifiers are: `concise`, `detailed`, `examples-first`, `socratic`. Example: `"concise-writing"` with `["concise"]` means brief, direct answers; `"academic-writing"` with `["examples-first"]` means scholarly with examples.
- Contradictions go in `observed_conflicts`; they stay unresolved unless the new evidence is clearly stronger.
- `session_continuity_hints` are short-lived bridges for near-future conversations, not permanent identity traits.
- `global_summary` (required, 1-3 sentences, ≤ 400 chars): a neutral third-person summary of the most significant profile changes across the recent sessions. It covers the 1-3 most material shifts rather than enumerating every update. Past tense ("user demonstrated comfort with X", not "user is comfortable with X").
- Empty categories are returned as empty arrays. Singleton fields with no update use action `none`.
- The output is raw JSON only — no markdown, no explanation, no trailing text.

## Example delta

Scenario: the user is a senior backend engineer who in the last day: (1) debugged a Postgres deadlock, (2) reviewed a Rust PR about trait objects, (3) asked a meta question about GraphQL vs REST for a new service, and (4) told the assistant to "always show real SQL, not hand-waving".

```json
{
  "expertise_domain_updates": [
    {
      "tag": "rust-trait-system",
      "action": "add",
      "description": "Comfortable with trait objects and dyn dispatch; reviews PRs in this area",
      "evidence": ["reviewed Rust PR about trait objects", "asked clarifying question about dyn vs impl Trait"],
      "confidence_signal": "strong"
    },
    {
      "tag": "postgres-concurrency",
      "action": "reinforce",
      "description": "Senior-level; debugs deadlocks confidently",
      "evidence": ["debugged Postgres deadlock"],
      "confidence_signal": "strong"
    }
  ],
  "preferred_answer_style_update": {
    "tag": "concise-writing",
    "modifiers": ["concise"],
    "action": "reinforce",
    "description": "Prefers brief, direct answers",
    "evidence": ["asked for 'short bullet summary' in recent session"],
    "confidence_signal": "medium"
  },
  "preferred_language_update": {
    "value": "en",
    "action": "none",
    "evidence": [],
    "confidence_signal": "weak"
  },
  "tool_preference_updates": [],
  "important_constraint_updates": [
    {
      "tag": "no-hand-waving-on-sql",
      "action": "add",
      "description": "User insists on seeing actual SQL queries, not pseudocode",
      "expires_at": null,
      "evidence": ["'always show real SQL, not hand-waving'"],
      "confidence_signal": "medium"
    }
  ],
  "session_continuity_hints": [
    {
      "hint": "User was mid-decision on GraphQL vs REST for new service",
      "source_session_id": "sess:2026-06-05-001",
      "priority": "medium"
    }
  ],
  "observed_conflicts": [],
  "global_summary": "Two strong updates: Rust trait expertise and senior-level Postgres concurrency. A new constraint emerged around concrete SQL over pseudocode."
}
```
