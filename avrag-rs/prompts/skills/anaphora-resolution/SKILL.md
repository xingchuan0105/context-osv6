---
name: anaphora-resolution
description: "Load when the user's message contains pronouns or references to previous entities, to resolve what those pronouns refer to."
version: "1.0"
applicable_strategies: ["chat"]
risk_level: "low"
category: "standard"
---

<instructions>
1. When the user uses pronouns like "it", "that", "this", "he", "she", "they", resolve them to the most recent relevant entity from the conversation history.
2. If the reference is ambiguous, ask the user for clarification rather than guessing.
3. Maintain continuity across turns by tracking active topics and entities.
</instructions>
