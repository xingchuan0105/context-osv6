# In-app notification copy

User-visible **bell** titles/bodies (persisted on `notifications`). **Not** LLM prompts (`prompts/`) and **not** SMTP (`email/`).

## Layout

Each event has four files: `{event}.title.{zh,en}.txt` and `{event}.body.{zh,en}.txt`.

| Event key | Emitters |
|-----------|----------|
| `funds-required` | app-chat when payer balance empty |
| `password-changed` | profile password update |
| `share-enabled` | create share link |
| `subscription-paid` / `subscription-expired` / `billing-update` | billing outbox |

Retired emitters (2026-08-18): `ingestion.success/failed` and `system.degrade` no longer create notifications — ingestion state stays visible on the document itself, and chat degrade signals stay in telemetry/eval labels only.

Runtime: `common::notification_copy` (`include_str!` + optional `{placeholder}`). Default locale is **zh**.

## Placeholders (optional)

Most events are fixed copy. Use `{name}` only when the emitter passes vars (currently none required).
