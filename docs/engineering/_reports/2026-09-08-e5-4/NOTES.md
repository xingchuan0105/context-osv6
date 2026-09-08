# E5.4 / G5.4 evidence

- `cargo test -p web-sdk -p web-ui`: all passed
- `cargo leptos build` (wasm-bindgen 0.2.127): exit 0
- Playwright fixture: **87 passed / 0 failed** (1.1m), including `e54-journey.spec.ts` (5) and existing billing/workspace/auth-settings/admin journeys
- Hardcoded placeholders `"98.5"` / `"42"` / `"128"` / usage `"0"` cards: grep = 0 in `frontend_rust`
- `role="dialog"` instances: AppDialog (search / rename / delete / upload) + 新建工作区 + 扫码支付 + Chat 网页来源 + 通知铃 = ≥6
- Toast: `Toaster` used on dashboard / settings / workbench (≥3 pages)
- Tiptap: vanilla JS bridge `assets/js/tiptap-editor-bridge.mjs` (minified ESM from `@tiptap/core` + starter-kit + markdown), mounted by `NoteEditor`

Analyze: `/dashboard/:id/analyze` replace-navigates to share center (Next baseline).
