# cos-tokens — Context-OS Monochrome Ink

Canonical visual tokens for App + public sites.

- Spec: `docs/design/STYLE_BASELINE.md`
- Source of truth: `tokens.css`
- Mark: `mark.svg` (use `currentColor` on the filled rect)

## Sync

```bash
bash packages/cos-tokens/sync.sh
```

Copies `tokens.css` into:

| Target | Path |
|--------|------|
| App | `frontend_next/app/design-tokens.css` |
| Landing | `../context-os-landing/styles/cos-tokens.css` |
| Why | `../whyiamright/frontend/src/styles/cos-tokens.css` |
| Ghost | `../context-os-theme/assets/css/tokens.css` |
| Cchess | `../cchess/frontend/src/cos-tokens.css` |

Also copies `mark.svg` beside each target where useful.
