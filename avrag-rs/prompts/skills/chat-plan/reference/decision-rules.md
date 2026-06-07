# Decision Rules

## Default behavior

Default to:
- `"action": "answer"`
- empty `calls`
- empty `writing_styles`
- `behavior_mode: null`

Do this for greetings, ordinary requests, creative tasks, rewrites, explanations, and other messages that can be handled well without more information.

## Use `"clarify"` only when:

- The request is genuinely ambiguous.
- A key variable is missing, such as goal, scope, format, audience, or constraint.
- Different assumptions would produce materially different answers.

## Do not clarify when:

- A reasonable default would work.
- The user is casually chatting.
- The request is exploratory but still answerable with options.
- The missing detail is minor and does not block useful progress.

## `intent` field

`intent` is dual-purpose:

1. **Default**: a one-sentence factual summary of the user's
   goal. Example: `"Calculate compound interest on $10,000"`.

2. **Mode recommendation** (when applicable): append a short
   redirect hint pointing to a different workspace mode. The
   answer agent renders this as a one-sentence suggestion to
   the user. Example:
   `"User asks for today's AI news — recommend Web Search"`.

   Do not use a structured `mode_recommendation` field — keep
   the hint inline in `intent`. The intent string should
   remain one sentence total.

   "One sentence" includes a main clause and an optional appended
   recommendation joined by an em dash, comma, or semicolon. Keep
   the recommendation to under half the sentence.

## Calls

### Decision criteria

Include `calls` only when **all three** are true:
1. **The tool is required for correctness** — without it, the
   answer would be wrong, fabricated, or stale. (Examples:
   calculator for a non-trivial expression; weather_query for
   "right now in city X"; code_interpreter for data analysis.)
2. **The user did not already provide the answer** — e.g., if
   the user said "what's 2+2", you do NOT need a tool call.
3. **The tool's args are derivable from the request** — no
   critical missing parameters. If a parameter is missing,
   clarify instead.

### Do NOT call a tool when

- The answer is in the user's message itself.
- The model can answer reliably from training data and the
  user has not asked for fresh / real-time data.
- A tool call would be speculative ("maybe the user wants
  this") — leave `calls` empty in that case and let the answer
  agent handle it.
- The tool would return data the user has not requested.

### Examples

| User says | `calls` should be |
|-----------|-------------------|
| "What's 2+2?" | `[]` (mental math) |
| "Calculate 1583 * 47 + sqrt(1024) - pow(2, 8)" | `[{calculator: ...}]` |
| "What's the weather in Beijing?" | `[{weather_query: ...}]` |
| "Tell me a joke" | `[]` (no tool) |
| "What's the temperature in Tokyo?" | `[{weather_query: ...}]` |
| "Help me write a poem" | `[]` (creative task) |
| "What is Rust?" | `[]` (training data) |
| "What is the latest Rust version?" | `[]` (may need `web_search`, not available in chat mode — embed mode-recommendation hint in `intent` instead) |

## Clarification message

When clarification is needed:
- Ask only the highest-leverage question first.
- Use multiple-choice when practical.
- Keep it short and concrete.

## Schema constraints

- **Do not invent fields not documented in `SKILL.md`**. The
  runtime parser silently ignores unknown top-level fields, so
  invented fields have no effect and may confuse future
  maintainers.
- **Do not invent args inside tool calls**. `args` must strictly
  match the tool's `args-schema` (auto-injected into the planner
  prompt). Invented args are either rejected by the runtime or
  silently dropped, depending on the tool's strictness.
- Keep `intent` to one sentence total (main clause + optional
  appended recommendation).
