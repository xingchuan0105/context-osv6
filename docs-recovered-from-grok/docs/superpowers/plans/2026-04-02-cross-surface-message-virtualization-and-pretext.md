# Cross-Surface Message Virtualization And Pretext Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Implement shared long-text virtualization for workspace chat, public shared notebook Q&A, and global search answer surfaces, using `pretext` for browser-side text height prediction and DOM reconciliation for final accuracy.

**Architecture:** Add a browser-side `pretext` predictor behind a narrow `platform::text_layout` interface, then build a shared variable-height virtual list subsystem in Rust. Integrate that subsystem into workspace chat first, preserving a pinned streaming tail and anchor-stable scroll behavior, then reuse the same windowing model in shared Q&A and search answer surfaces. Keep final rendering DOM-based and make every new behavior feature-flagged with a safe fallback.

**Tech Stack:** Rust (`Leptos 0.8`, `wasm-bindgen`, `web-sys`), browser-side ES modules, `@chenglou/pretext`, Node.js asset copy script, Playwright E2E.

---

## File Map

**Browser predictor and platform bridge**
- Modify: `frontend_rust/package.json`
- Modify: `frontend_rust/package-lock.json`
- Create: `frontend_rust/scripts/build-pretext-assets.mjs`
- Create: `frontend_rust/tests/text_layout_assets.smoke.mjs`
- Create: `frontend_rust/pkg/text_layout_predictor.js`
- Create: `frontend_rust/pkg/vendor/pretext.js`
- Modify: `frontend_rust/crates/web-ui/Cargo.toml`
- Modify: `frontend_rust/crates/web-ui/src/platform.rs`
- Modify: `frontend_rust/crates/web-ui/src/platform/capabilities.rs`
- Create: `frontend_rust/crates/web-ui/src/platform/text_layout.rs`
- Test: `frontend_rust/crates/web-ui/tests/text_layout_predictor.rs`

**Shared virtualization core**
- Modify: `frontend_rust/crates/web-ui/src/state/mod.rs`
- Create: `frontend_rust/crates/web-ui/src/state/virtual_list.rs`
- Modify: `frontend_rust/crates/web-ui/src/components/mod.rs`
- Create: `frontend_rust/crates/web-ui/src/components/virtual_text_list.rs`
- Test: `frontend_rust/crates/web-ui/tests/virtual_list_math.rs`

**Workspace chat integration**
- Modify: `frontend_rust/crates/web-ui/src/components/chat/mod.rs`
- Modify: `frontend_rust/crates/web-ui/src/components/chat/chat_panel.rs`
- Modify: `frontend_rust/crates/web-ui/src/components/chat/chat_bubble.rs`
- Create: `frontend_rust/crates/web-ui/src/components/chat/virtual_items.rs`
- Test: `frontend_rust/crates/web-ui/tests/chat_virtual_items.rs`

**Shared notebook and global search integration**
- Modify: `frontend_rust/crates/web-ui/src/routes/shared.rs`
- Modify: `frontend_rust/crates/web-ui/src/routes/search.rs`
- Test: `frontend_rust/crates/web-ui/tests/long_text_surface_items.rs`

**Browser regressions and rollout hooks**
- Modify: `avrag-rs/e2e/helpers.ts`
- Modify: `avrag-rs/e2e/rust-frontend-e2e.spec.ts`
- Modify: `frontend_rust/DELIVERY_HANDOFF.md`

---

### Task 1: Add The Pretext Predictor Bridge And Runtime Flags

**Files:**
- Modify: `frontend_rust/package.json`
- Modify: `frontend_rust/package-lock.json`
- Create: `frontend_rust/scripts/build-pretext-assets.mjs`
- Create: `frontend_rust/tests/text_layout_assets.smoke.mjs`
- Create: `frontend_rust/pkg/text_layout_predictor.js`
- Create: `frontend_rust/pkg/vendor/pretext.js`
- Modify: `frontend_rust/crates/web-ui/Cargo.toml`
- Modify: `frontend_rust/crates/web-ui/src/platform.rs`
- Modify: `frontend_rust/crates/web-ui/src/platform/capabilities.rs`
- Create: `frontend_rust/crates/web-ui/src/platform/text_layout.rs`
- Test: `frontend_rust/crates/web-ui/tests/text_layout_predictor.rs`

- [ ] **Step 1: Write the failing predictor smoke tests**

Create `frontend_rust/tests/text_layout_assets.smoke.mjs`:

```js
import assert from "node:assert/strict";
import test from "node:test";

test("text layout predictor wrapper exports browser helpers", async () => {
  const mod = await import("../pkg/text_layout_predictor.js");
  assert.equal(typeof mod.predictTextHeight, "function");
  assert.equal(typeof mod.clearTextLayoutCaches, "function");
});
```

Create `frontend_rust/crates/web-ui/tests/text_layout_predictor.rs`:

```rust
use web_ui::platform::text_layout::{estimate_shell_height, width_bucket, TypographyProfile};

fn profile() -> TypographyProfile {
    TypographyProfile {
        font_css: "16px Inter".to_string(),
        line_height_px: 24.0,
        horizontal_padding_px: 32.0,
        vertical_padding_px: 24.0,
        block_gap_px: 12.0,
        reserved_width_px: 40.0,
    }
}

#[test]
fn width_bucket_rounds_down_to_32px_steps() {
    assert_eq!(width_bucket(641.0), 640);
    assert_eq!(width_bucket(767.0), 736);
}

#[test]
fn shell_height_adds_padding_and_block_gap() {
    let profile = profile();
    let height = estimate_shell_height(96.0, &profile, 2);
    assert_eq!(height, 96.0 + 24.0 + 24.0 + 12.0);
}
```

- [ ] **Step 2: Run the tests to verify they fail**

Run:

```bash
node --test frontend_rust/tests/text_layout_assets.smoke.mjs
cargo test --manifest-path frontend_rust/Cargo.toml -p frontend-web-ui --test text_layout_predictor
```

Expected:
- Node test fails because `frontend_rust/pkg/text_layout_predictor.js` does not exist yet.
- Cargo test fails because `web_ui::platform::text_layout` does not exist yet.

- [ ] **Step 3: Add the browser predictor asset pipeline and Rust bridge**

Update `frontend_rust/package.json`:

```json
{
  "scripts": {
    "build:pretext-assets": "node scripts/build-pretext-assets.mjs",
    "test:pretext-assets": "node --test tests/text_layout_assets.smoke.mjs"
  },
  "dependencies": {
    "@chenglou/pretext": "^0.0.4",
    "tailwindcss": "^3.4.19"
  }
}
```

Create `frontend_rust/scripts/build-pretext-assets.mjs`:

```js
import fs from "node:fs/promises";
import path from "node:path";

const root = new URL("../", import.meta.url);
const source = new URL("../node_modules/@chenglou/pretext/dist/layout.js", import.meta.url);
const vendorDir = new URL("../pkg/vendor/", import.meta.url);
const target = new URL("../pkg/vendor/pretext.js", import.meta.url);

await fs.mkdir(vendorDir, { recursive: true });
await fs.copyFile(source, target);

console.log(`copied ${path.basename(source.pathname)} -> ${path.basename(target.pathname)}`);
```

Create `frontend_rust/pkg/text_layout_predictor.js`:

```js
import { clearCache, layout, prepare, setLocale } from "./vendor/pretext.js";

const preparedCache = new Map();
let activeLocale = null;

function cacheKey(text, fontCss, locale, whiteSpace) {
  return `${locale}::${fontCss}::${whiteSpace}::${text}`;
}

export async function predictTextHeight(input) {
  const {
    text,
    fontCss,
    locale = "en",
    maxWidthPx,
    lineHeightPx,
    whiteSpace = "pre-wrap",
  } = input;

  if (typeof text !== "string" || text.length === 0) {
    return { textHeightPx: 0, lineCount: 0 };
  }

  if (locale !== activeLocale) {
    setLocale(locale);
    preparedCache.clear();
    activeLocale = locale;
  }
  const key = cacheKey(text, fontCss, locale, whiteSpace);
  let prepared = preparedCache.get(key);
  if (!prepared) {
    prepared = prepare(text, fontCss, { whiteSpace });
    preparedCache.set(key, prepared);
  }

  const result = layout(prepared, maxWidthPx, lineHeightPx);
  return {
    textHeightPx: result.height,
    lineCount: result.lineCount,
  };
}

export function clearTextLayoutCaches() {
  preparedCache.clear();
  clearCache();
}
```

Update `frontend_rust/crates/web-ui/Cargo.toml` to include the extra browser features:

```toml
serde-wasm-bindgen = "0.6"
web-sys = { version = "0.3", features = [
  "Window",
  "Storage",
  "Document",
  "Element",
  "HtmlElement",
  "HtmlDivElement",
  "HtmlInputElement",
  "HtmlAnchorElement",
  "Blob",
  "Url",
  "Navigator",
  "ResizeObserver",
  "DomRect",
] }
```

Create `frontend_rust/crates/web-ui/src/platform/text_layout.rs`:

```rust
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TypographyProfile {
    pub font_css: String,
    pub line_height_px: f64,
    pub horizontal_padding_px: f64,
    pub vertical_padding_px: f64,
    pub block_gap_px: f64,
    pub reserved_width_px: f64,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TextHeightPrediction {
    pub text_height_px: f64,
    pub line_count: usize,
}

pub fn width_bucket(width_px: f64) -> u32 {
    ((width_px.max(32.0) / 32.0).floor() as u32) * 32
}

pub fn estimate_shell_height(text_height_px: f64, profile: &TypographyProfile, block_count: usize) -> f64 {
    let gaps = if block_count > 0 { (block_count - 1) as f64 * profile.block_gap_px } else { 0.0 };
    text_height_px + profile.vertical_padding_px * 2.0 + gaps
}

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen::prelude::wasm_bindgen(module = "/pkg/text_layout_predictor.js")]
extern "C" {
    #[wasm_bindgen::prelude::wasm_bindgen(catch, js_name = predictTextHeight)]
    async fn js_predict_text_height(input: wasm_bindgen::JsValue) -> Result<wasm_bindgen::JsValue, wasm_bindgen::JsValue>;
}

#[cfg(target_arch = "wasm32")]
pub async fn predict_text_height(
    text: &str,
    profile: &TypographyProfile,
    locale: &str,
    available_width_px: f64,
) -> anyhow::Result<TextHeightPrediction> {
    let payload = serde_json::json!({
        "text": text,
        "fontCss": profile.font_css,
        "locale": locale,
        "lineHeightPx": profile.line_height_px,
        "maxWidthPx": (available_width_px - profile.horizontal_padding_px - profile.reserved_width_px).max(32.0),
        "whiteSpace": "pre-wrap"
    });
    let value = js_predict_text_height(serde_wasm_bindgen::to_value(&payload)?)
        .await
        .map_err(|error| anyhow::anyhow!("predictTextHeight failed: {:?}", error))?;
    Ok(serde_wasm_bindgen::from_value(value)?)
}

#[cfg(not(target_arch = "wasm32"))]
pub async fn predict_text_height(
    text: &str,
    profile: &TypographyProfile,
    _locale: &str,
    available_width_px: f64,
) -> anyhow::Result<TextHeightPrediction> {
    let chars_per_line = ((available_width_px - profile.horizontal_padding_px - profile.reserved_width_px).max(32.0) / 8.0).floor().max(1.0);
    let line_count = ((text.chars().count() as f64) / chars_per_line).ceil().max(1.0) as usize;
    Ok(TextHeightPrediction {
        text_height_px: line_count as f64 * profile.line_height_px,
        line_count,
    })
}
```

Update `frontend_rust/crates/web-ui/src/platform.rs`:

```rust
pub mod capabilities;
pub mod text_layout;
```

Update `frontend_rust/crates/web-ui/src/platform/capabilities.rs`:

```rust
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct UiCapabilities {
    pub profile_edit: bool,
    pub password_reset: bool,
    pub shared_kb: bool,
    pub document_upload: bool,
    pub long_text_virtualization: bool,
    pub pretext_prediction: bool,
}

pub const UI_CAPABILITIES: UiCapabilities = UiCapabilities {
    profile_edit: false,
    password_reset: false,
    shared_kb: false,
    document_upload: false,
    long_text_virtualization: true,
    pretext_prediction: true,
};
```

- [ ] **Step 4: Run the asset build and verify the tests pass**

Run:

```bash
cd /home/chuan/context-osv6/frontend_rust
npm install
npm run build:pretext-assets
npm run test:pretext-assets
cargo test --manifest-path Cargo.toml -p frontend-web-ui --test text_layout_predictor
```

Expected:
- `npm run build:pretext-assets` prints `copied layout.js -> pretext.js`
- Node smoke test passes
- Cargo integration test passes

- [ ] **Step 5: Commit**

Run:

```bash
git add frontend_rust/package.json frontend_rust/package-lock.json frontend_rust/scripts/build-pretext-assets.mjs frontend_rust/tests/text_layout_assets.smoke.mjs frontend_rust/pkg/text_layout_predictor.js frontend_rust/pkg/vendor/pretext.js frontend_rust/crates/web-ui/Cargo.toml frontend_rust/crates/web-ui/src/platform.rs frontend_rust/crates/web-ui/src/platform/capabilities.rs frontend_rust/crates/web-ui/src/platform/text_layout.rs frontend_rust/crates/web-ui/tests/text_layout_predictor.rs
git commit -m "feat: add pretext predictor bridge"
```

---

### Task 2: Build The Shared Variable-Height Virtual List Core

**Files:**
- Modify: `frontend_rust/crates/web-ui/src/state/mod.rs`
- Create: `frontend_rust/crates/web-ui/src/state/virtual_list.rs`
- Modify: `frontend_rust/crates/web-ui/src/components/mod.rs`
- Create: `frontend_rust/crates/web-ui/src/components/virtual_text_list.rs`
- Test: `frontend_rust/crates/web-ui/tests/virtual_list_math.rs`

- [ ] **Step 1: Write the failing virtual-list math tests**

Create `frontend_rust/crates/web-ui/tests/virtual_list_math.rs`:

```rust
use web_ui::state::virtual_list::{
    apply_measurement_delta, compute_window, HeightState, ScrollMode, VirtualAnchor,
};

#[test]
fn compute_window_includes_visible_rows_and_overscan() {
    let heights = vec![
        HeightState::predicted("a", 100.0),
        HeightState::predicted("b", 100.0),
        HeightState::predicted("c", 100.0),
        HeightState::predicted("d", 100.0),
    ];

    let window = compute_window(&heights, 120.0, 160.0, 1);

    assert_eq!(window.start_index, 0);
    assert_eq!(window.end_index, 3);
    assert_eq!(window.top_spacer_px, 0.0);
}

#[test]
fn anchor_compensation_tracks_delta_above_anchor() {
    let anchor = VirtualAnchor {
        item_id: "c".to_string(),
        offset_within_item: 18.0,
        mode: ScrollMode::PreserveAnchor,
    };

    let delta = apply_measurement_delta(&anchor, "b", 40.0);
    assert_eq!(delta, 40.0);
}

#[test]
fn pinned_tail_never_drops_last_row() {
    let heights = vec![
        HeightState::predicted("a", 120.0),
        HeightState::predicted("b", 120.0),
        HeightState::predicted("tail", 240.0),
    ];

    let window = compute_window(&heights, 0.0, 120.0, 0).pin_tail("tail");
    assert!(window.visible_ids.iter().any(|id| id == "tail"));
}
```

- [ ] **Step 2: Run the test to verify it fails**

Run:

```bash
cargo test --manifest-path /home/chuan/context-osv6/frontend_rust/Cargo.toml -p frontend-web-ui --test virtual_list_math
```

Expected:
- Fail because `state::virtual_list` and its functions do not exist yet.

- [ ] **Step 3: Implement the minimal virtual-list math and component shell**

Create `frontend_rust/crates/web-ui/src/state/virtual_list.rs`:

```rust
#[derive(Clone, Debug, PartialEq)]
pub struct HeightState {
    pub item_id: String,
    pub predicted_px: f64,
    pub measured_px: Option<f64>,
}

impl HeightState {
    pub fn predicted(item_id: impl Into<String>, predicted_px: f64) -> Self {
        Self {
            item_id: item_id.into(),
            predicted_px,
            measured_px: None,
        }
    }

    pub fn effective_px(&self) -> f64 {
        self.measured_px.unwrap_or(self.predicted_px)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ScrollMode {
    FollowBottom,
    PreserveAnchor,
}

#[derive(Clone, Debug, PartialEq)]
pub struct VirtualAnchor {
    pub item_id: String,
    pub offset_within_item: f64,
    pub mode: ScrollMode,
}

#[derive(Clone, Debug, PartialEq)]
pub struct WindowSlice {
    pub start_index: usize,
    pub end_index: usize,
    pub top_spacer_px: f64,
    pub bottom_spacer_px: f64,
    pub visible_ids: Vec<String>,
}

impl WindowSlice {
    pub fn pin_tail(mut self, tail_id: &str) -> Self {
        if !self.visible_ids.iter().any(|id| id == tail_id) {
            self.visible_ids.push(tail_id.to_string());
        }
        self
    }
}

pub fn compute_window(
    heights: &[HeightState],
    scroll_top: f64,
    viewport_height: f64,
    overscan: usize,
) -> WindowSlice {
    let mut offset = 0.0;
    let mut start = 0;
    while start < heights.len() && offset + heights[start].effective_px() <= scroll_top {
        offset += heights[start].effective_px();
        start += 1;
    }

    let start_index = start.saturating_sub(overscan);
    let mut end_index = start;
    let mut consumed = offset;
    while end_index < heights.len() && consumed < scroll_top + viewport_height {
        consumed += heights[end_index].effective_px();
        end_index += 1;
    }
    end_index = (end_index + overscan).min(heights.len());

    let top_spacer_px = heights[..start_index].iter().map(HeightState::effective_px).sum();
    let rendered_height: f64 = heights[start_index..end_index].iter().map(HeightState::effective_px).sum();
    let total_height: f64 = heights.iter().map(HeightState::effective_px).sum();

    WindowSlice {
        start_index,
        end_index,
        top_spacer_px,
        bottom_spacer_px: (total_height - top_spacer_px - rendered_height).max(0.0),
        visible_ids: heights[start_index..end_index]
            .iter()
            .map(|item| item.item_id.clone())
            .collect(),
    }
}

pub fn apply_measurement_delta(anchor: &VirtualAnchor, updated_item_id: &str, delta_px: f64) -> f64 {
    if anchor.mode == ScrollMode::PreserveAnchor && updated_item_id != anchor.item_id {
        delta_px
    } else {
        0.0
    }
}
```

Update `frontend_rust/crates/web-ui/src/state/mod.rs`:

```rust
pub mod virtual_list;
```

Create `frontend_rust/crates/web-ui/src/components/virtual_text_list.rs`:

```rust
use leptos::prelude::*;

use crate::state::virtual_list::{compute_window, HeightState};

#[component]
pub fn VirtualTextList(
    #[prop(into)] row_heights: Signal<Vec<HeightState>>,
    #[prop(into)] viewport_height_px: Signal<f64>,
    #[prop(into)] scroll_top_px: Signal<f64>,
    overscan: usize,
    children: Children,
) -> impl IntoView {
    let window = Signal::derive(move || {
        compute_window(
            &row_heights.get(),
            scroll_top_px.get(),
            viewport_height_px.get(),
            overscan,
        )
    });

    view! {
        <div data-window-start=move || window.get().start_index data-window-end=move || window.get().end_index>
            <div style=move || format!("height: {}px", window.get().top_spacer_px)></div>
            {children()}
            <div style=move || format!("height: {}px", window.get().bottom_spacer_px)></div>
        </div>
    }
}
```

Update `frontend_rust/crates/web-ui/src/components/mod.rs`:

```rust
pub mod virtual_text_list;
pub use virtual_text_list::VirtualTextList;
```

- [ ] **Step 4: Run the tests to verify they pass**

Run:

```bash
cargo test --manifest-path /home/chuan/context-osv6/frontend_rust/Cargo.toml -p frontend-web-ui --test virtual_list_math
cargo test --manifest-path /home/chuan/context-osv6/frontend_rust/Cargo.toml -p frontend-web-ui --lib
```

Expected:
- New integration test passes
- Existing library tests remain green

- [ ] **Step 5: Commit**

Run:

```bash
git add frontend_rust/crates/web-ui/src/state/mod.rs frontend_rust/crates/web-ui/src/state/virtual_list.rs frontend_rust/crates/web-ui/src/components/mod.rs frontend_rust/crates/web-ui/src/components/virtual_text_list.rs frontend_rust/crates/web-ui/tests/virtual_list_math.rs
git commit -m "feat: add shared virtual list core"
```

---

### Task 3: Integrate Workspace Chat With A Pinned Streaming Tail

**Files:**
- Modify: `frontend_rust/crates/web-ui/src/components/chat/mod.rs`
- Modify: `frontend_rust/crates/web-ui/src/components/chat/chat_panel.rs`
- Modify: `frontend_rust/crates/web-ui/src/components/chat/chat_bubble.rs`
- Create: `frontend_rust/crates/web-ui/src/components/chat/virtual_items.rs`
- Test: `frontend_rust/crates/web-ui/tests/chat_virtual_items.rs`

- [ ] **Step 1: Write the failing workspace-chat adapter tests**

Create `frontend_rust/crates/web-ui/tests/chat_virtual_items.rs`:

```rust
use web_ui::components::chat::virtual_items::{chat_message_to_virtual_item, chat_style_profile};
use web_ui::state::chat::{ChatMessage, ChatRole};

fn message(id: &str, role: ChatRole, content: &str) -> ChatMessage {
    ChatMessage {
        id: id.to_string(),
        role,
        content: content.to_string(),
        answer_blocks: Vec::new(),
        citations: Vec::new(),
        session_id: None,
        server_message_id: None,
    }
}

#[test]
fn assistant_message_becomes_predictable_virtual_item() {
    let item = chat_message_to_virtual_item(&message("m1", ChatRole::Assistant, "long answer"), false);
    assert_eq!(item.id, "m1");
    assert_eq!(item.text_body, "long answer");
    assert!(!item.pinned_tail);
}

#[test]
fn streaming_tail_is_pinned() {
    let item = chat_message_to_virtual_item(&message("m2", ChatRole::Assistant, "stream"), true);
    assert!(item.pinned_tail);
}

#[test]
fn chat_style_profile_reserves_avatar_width() {
    let profile = chat_style_profile(ChatRole::Assistant);
    assert!(profile.reserved_width_px >= 32.0);
}
```

- [ ] **Step 2: Run the test to verify it fails**

Run:

```bash
cargo test --manifest-path /home/chuan/context-osv6/frontend_rust/Cargo.toml -p frontend-web-ui --test chat_virtual_items
```

Expected:
- Fail because `components::chat::virtual_items` does not exist yet.

- [ ] **Step 3: Implement the chat adapter and switch the message list to `VirtualTextList`**

Create `frontend_rust/crates/web-ui/src/components/chat/virtual_items.rs`:

```rust
use crate::platform::text_layout::TypographyProfile;
use crate::state::chat::{ChatMessage, ChatRole};

#[derive(Clone, Debug, PartialEq)]
pub struct ChatVirtualItem {
    pub id: String,
    pub text_body: String,
    pub pinned_tail: bool,
    pub profile: TypographyProfile,
}

pub fn chat_style_profile(role: ChatRole) -> TypographyProfile {
    TypographyProfile {
        font_css: "16px Inter".to_string(),
        line_height_px: 28.0,
        horizontal_padding_px: 24.0,
        vertical_padding_px: 20.0,
        block_gap_px: 12.0,
        reserved_width_px: if role == ChatRole::Assistant { 48.0 } else { 40.0 },
    }
}

pub fn chat_message_to_virtual_item(message: &ChatMessage, pinned_tail: bool) -> ChatVirtualItem {
    ChatVirtualItem {
        id: message.id.clone(),
        text_body: message.content.clone(),
        pinned_tail,
        profile: chat_style_profile(message.role),
    }
}
```

Update `frontend_rust/crates/web-ui/src/components/chat/mod.rs`:

```rust
pub mod virtual_items;
```

Update the message rendering block in `frontend_rust/crates/web-ui/src/components/chat/chat_panel.rs`:

```rust
let scroller = NodeRef::<leptos::html::Div>::new();
let scroll_top_px = RwSignal::new(0.0);
let viewport_height_px = RwSignal::new(720.0);

let virtual_items = Signal::derive(move || {
    let messages = chat.messages.get();
    let last_id = messages.last().map(|item| item.id.clone());
    messages
        .iter()
        .map(|message| {
            let pinned_tail = chat.status.get() == ChatStatus::Streaming
                && last_id.as_ref().map(|id| id == &message.id).unwrap_or(false)
                && message.role == ChatRole::Assistant;
            chat_message_to_virtual_item(message, pinned_tail)
        })
        .collect::<Vec<_>>()
});

let visible_ids = Signal::derive(move || {
    use std::collections::HashSet;

    let heights = virtual_items
        .get()
        .iter()
        .map(|item| crate::state::virtual_list::HeightState::predicted(item.id.clone(), 160.0))
        .collect::<Vec<_>>();
    crate::state::virtual_list::compute_window(&heights, scroll_top_px.get(), viewport_height_px.get(), 4)
        .pin_tail(
            virtual_items
                .get()
                .iter()
                .find(|item| item.pinned_tail)
                .map(|item| item.id.as_str())
                .unwrap_or(""),
        )
        .visible_ids
        .into_iter()
        .collect::<HashSet<_>>()
});
```

Replace the current direct message map in `chat_panel.rs` with:

```rust
<div
    node_ref=scroller
    class="flex-1 overflow-y-auto p-4 bg-background pb-32 relative"
    data-test-chat-scroll
    on:scroll=move |_| {
        if let Some(node) = scroller.get() {
            scroll_top_px.set(node.scroll_top() as f64);
            viewport_height_px.set(node.client_height() as f64);
        }
    }
>
    <Show when={move || !chat.messages.get().is_empty()}>
        <VirtualTextList
            row_heights=Signal::derive(move || {
                virtual_items
                    .get()
                    .iter()
                    .map(|item| crate::state::virtual_list::HeightState::predicted(item.id.clone(), 160.0))
                    .collect::<Vec<_>>()
            })
            viewport_height_px=Signal::derive(move || viewport_height_px.get())
            scroll_top_px=Signal::derive(move || scroll_top_px.get())
            overscan=4
        >
            <div class="space-y-4">
                {chat.messages.get().into_iter().filter(|msg| visible_ids.get().contains(&msg.id)).map(|msg| {
                    view! { <ChatBubble message={msg} /> }
                }).collect_view()}
            </div>
        </VirtualTextList>
    </Show>
</div>
```

Update `frontend_rust/crates/web-ui/src/components/chat/chat_bubble.rs` so the message root exposes stable measurement hooks:

```rust
<div
    class="flex mb-8 animate-fade-in"
    attr:data-virtual-item-id={message.id.clone()}
    attr:data-virtual-role={message.role.as_str()}
>
```

- [ ] **Step 4: Run the tests to verify they pass**

Run:

```bash
cargo test --manifest-path /home/chuan/context-osv6/frontend_rust/Cargo.toml -p frontend-web-ui --test chat_virtual_items
cargo test --manifest-path /home/chuan/context-osv6/frontend_rust/Cargo.toml -p frontend-web-ui --lib
```

Expected:
- The new adapter tests pass
- Existing web-ui library tests stay green

- [ ] **Step 5: Commit**

Run:

```bash
git add frontend_rust/crates/web-ui/src/components/chat/mod.rs frontend_rust/crates/web-ui/src/components/chat/chat_panel.rs frontend_rust/crates/web-ui/src/components/chat/chat_bubble.rs frontend_rust/crates/web-ui/src/components/chat/virtual_items.rs frontend_rust/crates/web-ui/tests/chat_virtual_items.rs
git commit -m "feat: virtualize workspace chat messages"
```

---

### Task 4: Reuse The Shared Virtualization Model In Shared Q&A And Search

**Files:**
- Modify: `frontend_rust/crates/web-ui/src/routes/shared.rs`
- Modify: `frontend_rust/crates/web-ui/src/routes/search.rs`
- Test: `frontend_rust/crates/web-ui/tests/long_text_surface_items.rs`

- [ ] **Step 1: Write the failing long-text surface adapter tests**

Create `frontend_rust/crates/web-ui/tests/long_text_surface_items.rs`:

```rust
use web_ui::routes::search::{search_answer_item_text, search_source_preview_text};
use web_ui::routes::shared::{shared_answer_item_text, shared_source_preview_text};

#[test]
fn search_answer_uses_full_answer_text() {
    assert_eq!(search_answer_item_text("hello"), "hello");
}

#[test]
fn shared_answer_uses_streaming_text_when_present() {
    assert_eq!(shared_answer_item_text("stream chunk", "final answer"), "stream chunk");
    assert_eq!(shared_answer_item_text("", "final answer"), "final answer");
}

#[test]
fn source_preview_helpers_prefer_preview_text() {
    assert_eq!(shared_source_preview_text(Some("preview"), None), "preview");
    assert_eq!(search_source_preview_text(Some("snippet"), None), "snippet");
}
```

- [ ] **Step 2: Run the test to verify it fails**

Run:

```bash
cargo test --manifest-path /home/chuan/context-osv6/frontend_rust/Cargo.toml -p frontend-web-ui --test long_text_surface_items
```

Expected:
- Fail because the helper functions do not exist yet.

- [ ] **Step 3: Add adapter helpers and replace direct answer stacks with `VirtualTextList`**

Add to `frontend_rust/crates/web-ui/src/routes/search.rs`:

```rust
pub fn search_answer_item_text(answer: &str) -> String {
    answer.to_string()
}

pub fn search_source_preview_text(snippet: Option<&str>, fallback: Option<&str>) -> String {
    snippet.or(fallback).unwrap_or_default().to_string()
}
```

Replace the long answer block in `search.rs` with a virtualized region:

```rust
let search_scroller = NodeRef::<leptos::html::Div>::new();
let search_scroll_top_px = RwSignal::new(0.0);
let search_viewport_height_px = RwSignal::new(720.0);
let answer_items = Signal::derive(move || {
    let mut items = Vec::new();
    if !answer.get().is_empty() {
        items.push(crate::state::virtual_list::HeightState::predicted("search-answer", 240.0));
    }
    for source in sources.get() {
        items.push(crate::state::virtual_list::HeightState::predicted(
            format!("search-source-{}", source.title),
            128.0,
        ));
    }
    items
});

let visible_search_ids = Signal::derive(move || {
    use std::collections::HashSet;

    crate::state::virtual_list::compute_window(
        &answer_items.get(),
        search_scroll_top_px.get(),
        search_viewport_height_px.get(),
        3,
    )
    .visible_ids
    .into_iter()
    .collect::<HashSet<_>>()
});
```

And render:

```rust
<div
    node_ref=search_scroller
    data-test-search-scroll
    on:scroll=move |_| {
        if let Some(node) = search_scroller.get() {
            search_scroll_top_px.set(node.scroll_top() as f64);
            search_viewport_height_px.set(node.client_height() as f64);
        }
    }
>
    <VirtualTextList
        row_heights=answer_items
        viewport_height_px=Signal::derive(move || search_viewport_height_px.get())
        scroll_top_px=Signal::derive(move || search_scroll_top_px.get())
        overscan=3
    >
        <div class="space-y-4">
            {if visible_search_ids.get().contains("search-answer") {
                view! {
                    <div class="app-surface-card p-6 mb-4">
                        <div class="whitespace-pre-wrap text-foreground">{answer.get()}</div>
                    </div>
                }.into_any()
            } else {
                view! { <></> }.into_any()
            }}
            {sources.get().into_iter().filter(|source| {
                visible_search_ids
                    .get()
                    .contains(&format!("search-source-{}", source.title))
            }).map(|source| {
                view! {
                    <div class="rounded border border-border p-3">
                        <div class="font-medium text-foreground">{source.title}</div>
                        <div class="mt-1 text-xs text-muted-foreground">{source.snippet.clone().unwrap_or_default()}</div>
                    </div>
                }
            }).collect_view()}
        </div>
    </VirtualTextList>
</div>
```

Add to `frontend_rust/crates/web-ui/src/routes/shared.rs`:

```rust
pub fn shared_answer_item_text(streaming_answer: &str, final_answer: &str) -> String {
    if !streaming_answer.is_empty() {
        streaming_answer.to_string()
    } else {
        final_answer.to_string()
    }
}

pub fn shared_source_preview_text(preview: Option<&str>, fallback: Option<&str>) -> String {
    preview.or(fallback).unwrap_or_default().to_string()
}
```

Replace the current answer / citation / retrieved-source result flow in `shared.rs` with the same `VirtualTextList` shell:

```rust
let shared_scroller = NodeRef::<leptos::html::Div>::new();
let shared_scroll_top_px = RwSignal::new(0.0);
let shared_viewport_height_px = RwSignal::new(720.0);
let result_items = Signal::derive(move || {
    let mut items = Vec::new();
    let answer_text = shared_answer_item_text(&streaming_answer.get(), &answer.get());
    if !answer_text.is_empty() {
        items.push(crate::state::virtual_list::HeightState::predicted("shared-answer", 240.0));
    }
    if !citations.get().is_empty() {
        items.push(crate::state::virtual_list::HeightState::predicted("shared-citations", 180.0));
    }
    if !chat_sources.get().is_empty() {
        items.push(crate::state::virtual_list::HeightState::predicted("shared-sources", 180.0));
    }
    items
});

let visible_shared_ids = Signal::derive(move || {
    use std::collections::HashSet;

    crate::state::virtual_list::compute_window(
        &result_items.get(),
        shared_scroll_top_px.get(),
        shared_viewport_height_px.get(),
        3,
    )
    .visible_ids
    .into_iter()
    .collect::<HashSet<_>>()
});

<div
    node_ref=shared_scroller
    data-test-shared-scroll
    on:scroll=move |_| {
        if let Some(node) = shared_scroller.get() {
            shared_scroll_top_px.set(node.scroll_top() as f64);
            shared_viewport_height_px.set(node.client_height() as f64);
        }
    }
>
    <VirtualTextList
        row_heights=result_items
        viewport_height_px=Signal::derive(move || shared_viewport_height_px.get())
        scroll_top_px=Signal::derive(move || shared_scroll_top_px.get())
        overscan=3
    >
        <div class="space-y-4">
            {if visible_shared_ids.get().contains("shared-answer") {
                view! {
                    <div class="border-t border-border pt-4">
                        <div class="prose prose-sm max-w-none whitespace-pre-wrap text-foreground">
                            {shared_answer_item_text(&streaming_answer.get(), &answer.get())}
                        </div>
                    </div>
                }.into_any()
            } else {
                view! { <></> }.into_any()
            }}
            {if visible_shared_ids.get().contains("shared-citations") {
                view! {
                    <div class="space-y-2">
                        {citations.get().into_iter().map(|citation| {
                            view! { <div class="rounded-xl border border-border bg-card px-3 py-3">{citation.doc_name}</div> }
                        }).collect_view()}
                    </div>
                }.into_any()
            } else {
                view! { <></> }.into_any()
            }}
            {if visible_shared_ids.get().contains("shared-sources") {
                view! {
                    <div class="space-y-2">
                        {chat_sources.get().into_iter().map(|source| {
                            view! { <div class="rounded-xl border border-border bg-card px-3 py-3">{source.title}</div> }
                        }).collect_view()}
                    </div>
                }.into_any()
            } else {
                view! { <></> }.into_any()
            }}
        </div>
    </VirtualTextList>
</div>
```

- [ ] **Step 4: Run the tests to verify they pass**

Run:

```bash
cargo test --manifest-path /home/chuan/context-osv6/frontend_rust/Cargo.toml -p frontend-web-ui --test long_text_surface_items
cargo test --manifest-path /home/chuan/context-osv6/frontend_rust/Cargo.toml -p frontend-web-ui --lib
```

Expected:
- New long-text adapter tests pass
- Existing library tests remain green

- [ ] **Step 5: Commit**

Run:

```bash
git add frontend_rust/crates/web-ui/src/routes/shared.rs frontend_rust/crates/web-ui/src/routes/search.rs frontend_rust/crates/web-ui/tests/long_text_surface_items.rs
git commit -m "feat: virtualize shared and search long text surfaces"
```

---

### Task 5: Add Browser Regressions, Rollout Proof, And Delivery Notes

**Files:**
- Modify: `avrag-rs/e2e/helpers.ts`
- Modify: `avrag-rs/e2e/rust-frontend-e2e.spec.ts`
- Modify: `frontend_rust/DELIVERY_HANDOFF.md`

- [ ] **Step 1: Write the failing browser regression tests**

Add to `avrag-rs/e2e/helpers.ts`:

```ts
export async function longText(page: Page, label: string, paragraphs = 18): Promise<string> {
  return Array.from({ length: paragraphs }, (_, index) =>
    `${label} paragraph ${index + 1}: ` +
    "Knowledge-base answer content that is intentionally long so virtualization has to manage real scrolling pressure."
  ).join("\n\n");
}
```

Add to `avrag-rs/e2e/rust-frontend-e2e.spec.ts`:

```ts
test("T19: workspace chat keeps scroll position while browsing history", async ({ page, request }) => {
  const auth = await registerTestUser(request);
  const notebookId = await createWorkspaceViaAPI(request, auth.token, uniqueName("pw-virtual-chat"));

  await seedBrowserAuth(page, request, auth.token);
  await page.goto(`/dashboard/${notebookId}`);

  await page.evaluate(async () => {
    const shell = document.querySelector("[data-test-chat-scroll]") as HTMLElement | null;
    if (!shell) throw new Error("missing chat scroll shell");
    shell.scrollTop = shell.scrollHeight;
  });

  const before = await page.evaluate(() => {
    const shell = document.querySelector("[data-test-chat-scroll]") as HTMLElement | null;
    if (!shell) throw new Error("missing chat scroll shell");
    shell.scrollTop = Math.max(0, shell.scrollTop - 600);
    return shell.scrollTop;
  });

  await page.waitForTimeout(250);

  const after = await page.evaluate(() => {
    const shell = document.querySelector("[data-test-chat-scroll]") as HTMLElement | null;
    if (!shell) throw new Error("missing chat scroll shell");
    return shell.scrollTop;
  });

  expect(Math.abs(after - before)).toBeLessThan(24);
});
```

- [ ] **Step 2: Run the browser test to verify it fails**

Run:

```bash
cd /home/chuan/context-osv6/avrag-rs
npx playwright test rust-frontend-e2e.spec.ts --grep "T19"
```

Expected:
- Fail because the test hook selectors and virtualization behavior do not exist yet.

- [ ] **Step 3: Wire the test hooks and update delivery notes**

Add a stable scroll-shell hook in `frontend_rust/crates/web-ui/src/components/chat/chat_panel.rs`:

```rust
<div class="flex-1 overflow-y-auto p-4 bg-background pb-32 relative" data-test-chat-scroll>
```

Add equivalent scroll-region hooks in:

- `frontend_rust/crates/web-ui/src/routes/shared.rs` for the Q&A results shell
- `frontend_rust/crates/web-ui/src/routes/search.rs` for the answer/results shell

Update `frontend_rust/DELIVERY_HANDOFF.md` with a short section:

```md
### Long-text virtualization

- Workspace chat, shared notebook Q&A, and global search answer surfaces now use a shared virtual list layer.
- Browser-side `pretext` prediction is optional and falls back to heuristic estimation if unavailable.
- Streaming tail items stay mounted to preserve live-answer stability.
```

- [ ] **Step 4: Run the regression suite and baseline checks**

Run:

```bash
cd /home/chuan/context-osv6/frontend_rust
npm run test:pretext-assets
cargo test --manifest-path Cargo.toml -p frontend-web-ui --lib

cd /home/chuan/context-osv6/avrag-rs
npx playwright test rust-frontend-e2e.spec.ts --grep "T19"
```

Expected:
- Asset smoke test passes
- web-ui library tests pass
- Playwright regression passes

- [ ] **Step 5: Commit**

Run:

```bash
git add avrag-rs/e2e/helpers.ts avrag-rs/e2e/rust-frontend-e2e.spec.ts frontend_rust/DELIVERY_HANDOFF.md frontend_rust/crates/web-ui/src/components/chat/chat_panel.rs frontend_rust/crates/web-ui/src/routes/shared.rs frontend_rust/crates/web-ui/src/routes/search.rs
git commit -m "test: cover long-text virtualization behavior"
```

---

## Self-Review

### Spec coverage

- Shared `pretext` predictor and fallback path: covered in Task 1
- Shared virtual list subsystem: covered in Task 2
- Workspace chat integration with pinned streaming tail: covered in Task 3
- Shared notebook and search surface reuse: covered in Task 4
- Browser regression and rollout proof: covered in Task 5

### Placeholder scan

- No `TODO` / `TBD` markers remain
- Every task has explicit files, commands, and code snippets
- No task depends on an undefined module name introduced nowhere else in the plan

### Type consistency

- `TypographyProfile`, `HeightState`, `VirtualAnchor`, `ScrollMode`, and `VirtualTextList` are introduced before later tasks reference them
- `chat_message_to_virtual_item`, `shared_answer_item_text`, and `search_answer_item_text` are defined in the same tasks that consume them
- The browser predictor API is consistently named `predictTextHeight` in JS and `predict_text_height` in Rust
