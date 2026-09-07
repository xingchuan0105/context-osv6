# E5.1 / G5.1 evidence

- `cargo test -p web-ui --test style_baseline_guard`: 3 passed
- `cargo test -p web-sdk -p web-ui`: all passed
- `cargo leptos build` (wasm-bindgen 0.2.127): exit 0
- `pnpm exec playwright test --config playwright.config.ts`: **74 passed / 0 failed** (41.7s)
- Visual walkthrough (1280×800, cream body `rgb(247,247,243)`, `box-sizing: border-box`, `/style/app.css` loaded): `chat.png` `/chat`, `login.png` `/login`, `pricing.png` `/pricing`, `help.png` `/help`, `settings.png` `/settings`
