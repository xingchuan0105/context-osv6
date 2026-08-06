# Product email templates

User-facing SMTP copy (password reset, workspace invite). **Not** LLM prompts — those live under `prompts/`.

## Layout

| File | Placeholders |
|------|----------------|
| `password-reset.subject.{zh,en}.txt` | — |
| `password-reset.body.{zh,en}.txt` | `{code}`, `{expires_at}` |
| `workspace-invite.subject.{zh,en}.txt` | `{workspace_title}` |
| `workspace-invite.body.{zh,en}.txt` | `{inviter}`, `{workspace_title}`, `{accept_url}` |

Runtime loads via `include_str!` in `app-bootstrap` (`services/email_copy.rs`) and substitutes `{name}` placeholders. Prefer plain UTF-8 text; keep subjects one line.

## Locale

- API `lang` / invite `locale_zh` → `zh` default, `en` when explicitly English.
- Default product locale is Chinese.
