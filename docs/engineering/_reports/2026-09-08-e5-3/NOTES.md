# E5.3 / G5.3 evidence

- `cargo test -p web-sdk -p web-ui`: all passed (includes `test_done_payload_commits_tool_degrade_and_guard`, markdown code/figure)
- `cargo leptos build` (wasm-bindgen 0.2.127): exit 0
- `pnpm exec playwright test --config playwright.config.ts`: **82 passed / 0 failed** (53.1s)
- live smoke: `LIVE_BACKEND=1 LIVE_API_BASE=http://127.0.0.1:18090 pnpm exec playwright test --config playwright.live.config.ts`: **2 passed / 0 failed** (9.0s)
- Chat-first: still one `ChatCanvasModel` + `reduce_chat_event`; workspace embed reuses `ChatPage`

New Playwright chat assertions (E5.3 describe, ≫10):

1. `chat-hero` visible
2. composer `role=slider` + ArrowUp `aria-valuenow=112`
3. mobile `chat-rail-toggle` opens `session-list`
4. `chat-code-block` / `chat-code-lang` / copy clipboard `fn main() {}`
5. `chat-figure` visible
6. `tool-result-card` contains `web_search`
7. `workspace-web-sources-modal` lists `example.com/alloy`
8. `chat-degrade-notice` visible
9. `workspace-progress-elapsed` visible
10. `scroll-to-bottom` appears after scroll-up, hides after click
11. session empty / error+retry / loading
12. session items `disabled` while streaming; URL does not jump
