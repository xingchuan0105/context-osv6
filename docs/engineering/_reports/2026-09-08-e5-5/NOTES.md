# E5.5 / G5.5 evidence

- `cargo test -p web-sdk -p web-ui`: all passed, including `i18n_catalog_tests` and `i18n_source_guard` (shell/chat 无硬编码中文 literal)
- `cargo leptos build` (wasm-bindgen 0.2.127): exit 0
- Playwright fixture: **91 passed / 0 failed** (1.1m), including `e55-i18n-responsive.spec.ts` (language switch, `/en/pricing` English, mobile chat drawer, mobile dashboard)
- Catalog: Next `frontend_next/lib/i18n/messages/` extracted + extras → `frontend_rust/crates/web-ui/i18n/messages.json` (1742 keys); zh-CN default; cookie `avrag.ui.locale` + localStorage `avrag.ui.locale.v1`
- `/en/pricing`: `PricingPage locale_override=En`; SSR + hydrate English (`Choose your plan` / `Monthly` / `FAQ`); en-journey asserts English
- Breakpoints: `assets/style/responsive.css` 840 / 768 / 767 / 720 / 640 plus existing chat drawer `47.9375rem` and public `48rem`
- `prefers-reduced-motion`: `base.css` global block still in force
