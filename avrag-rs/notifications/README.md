# In-app notification copy

User-visible **bell** titles/bodies (persisted on `notifications`). **Not** LLM prompts (`prompts/`) and **not** SMTP (`email/`).

## Layout

Each event has four files: `{event}.title.{zh,en}.txt` and `{event}.body.{zh,en}.txt`.

| Event key | Emitters |
|-----------|----------|
| `ingestion-success` / `ingestion-failed` | worker document pipeline |
| `funds-required` | app-chat when payer balance empty |
| `password-changed` | profile password update |
| `share-enabled` | create share link |
| `subscription-paid` / `subscription-expired` / `billing-update` | billing outbox |
| `degrade-general` / `degrade-search` / `degrade-rag` | chat degrade_trace |

Runtime: `common::notification_copy` (`include_str!` + optional `{placeholder}`). Default locale is **zh**.

## Placeholders (optional)

Most events are fixed copy. Use `{name}` only when the emitter passes vars (currently none required).
