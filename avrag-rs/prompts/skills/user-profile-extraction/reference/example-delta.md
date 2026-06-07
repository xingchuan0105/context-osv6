# Example user-profile-extraction deltas

These examples are reference material for the dream layer. They show what a
well-formed, grounded delta looks like for different user scenarios.

---

## Example 1 — Senior backend engineer, strong signals

**Recent sessions:**
1. Debugged a Postgres deadlock with `pg_locks` and `pg_stat_activity`.
2. Reviewed a Rust PR about trait objects vs generics.
3. Asked a meta-question: "GraphQL vs REST for a new microservice?"
4. Said: "always show real SQL, not hand-waving."

**Reasonable delta:**

```json
{
  "expertise_domain_updates": [
    {
      "tag": "rust-trait-system",
      "action": "add",
      "description": "Comfortable with trait objects and dyn dispatch; reviews PRs in this area",
      "evidence": [
        "reviewed Rust PR about trait objects",
        "asked clarifying question about dyn vs impl Trait"
      ],
      "confidence_signal": "strong"
    },
    {
      "tag": "postgres-concurrency",
      "action": "reinforce",
      "description": "Senior-level; debugs deadlocks confidently",
      "evidence": ["debugged Postgres deadlock using pg_locks and pg_stat_activity"],
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
  "tool_preference_updates": [
    {
      "tag": "rag",
      "action": "reinforce",
      "reason": "Used RAG to look up Postgres internals",
      "evidence": ["queried internal docs about pg_stat_activity"],
      "confidence_signal": "medium"
    }
  ],
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

---

## Example 2 — Weak single signal → prefer no update

**Recent session:**
User asked one casual question: "What's a monad?"

**Correct delta:**

```json
{
  "expertise_domain_updates": [],
  "preferred_answer_style_update": {
    "action": "none",
    "confidence_signal": "weak",
    "evidence": []
  },
  "preferred_language_update": {
    "action": "none",
    "confidence_signal": "weak",
    "evidence": []
  },
  "tool_preference_updates": [],
  "important_constraint_updates": [],
  "session_continuity_hints": [],
  "observed_conflicts": [],
  "global_summary": "No durable profile updates. One exploratory question about functional programming concepts."
}
```

**Why this is correct:**
A single casual question is not enough to infer a durable expertise domain or
preference. Adding "functional-programming" here would be speculative.

---

## Example 3 — Observed conflict, no forced resolution

**Recent sessions:**
- Two weeks ago: user praised "ultra-concise answers."
- Today: user said "your last answer was too short; give me the full reasoning."

**Correct delta:**

```json
{
  "expertise_domain_updates": [],
  "preferred_answer_style_update": {
    "action": "none",
    "confidence_signal": "weak",
    "evidence": []
  },
  "preferred_language_update": {
    "action": "none",
    "confidence_signal": "weak",
    "evidence": []
  },
  "tool_preference_updates": [],
  "important_constraint_updates": [],
  "session_continuity_hints": [],
  "observed_conflicts": [
    {
      "field": "preferred_answer_style",
      "old_view": "prefers ultra-concise answers",
      "new_view": "wants full reasoning, not too short",
      "evidence": [
        "'ultra-concise answers' (sess:2026-05-20-003)",
        "'your last answer was too short; give me the full reasoning'"
      ]
    }
  ],
  "global_summary": "An emerging conflict in preferred answer style: prior preference for concise answers is now contested."
}
```

**Why this is correct:**
The new signal is not clearly stronger than the old one, and it may be
context-dependent. Record the conflict and let future sessions resolve it.

---

## Example 4 — Session continuity hint, not a durable preference

**Recent session:**
User started designing a schema migration but ran out of time. They asked to
"continue tomorrow."

**Correct delta:**

```json
{
  "expertise_domain_updates": [],
  "preferred_answer_style_update": {
    "action": "none",
    "confidence_signal": "weak",
    "evidence": []
  },
  "preferred_language_update": {
    "action": "none",
    "confidence_signal": "weak",
    "evidence": []
  },
  "tool_preference_updates": [],
  "important_constraint_updates": [],
  "session_continuity_hints": [
    {
      "hint": "User was in the middle of designing a schema migration and asked to continue tomorrow",
      "source_session_id": "sess:2026-06-05-042",
      "priority": "high"
    }
  ],
  "observed_conflicts": [],
  "global_summary": "No durable preference update. Added a high-priority continuity hint for the pending schema migration."
}
```

**Why this is correct:**
"Wants to continue a schema migration" is a one-time contextual bridge, not a
stable identity trait. It belongs in `session_continuity_hints` (7-day FIFO)
rather than `expertise_domain_updates`.
