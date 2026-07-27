# Cross-Surface Message Virtualization And Pretext Design

> Scope: `frontend_rust/crates/web-ui` only. This design covers long-text result virtualization for the main workspace chat, public shared notebook Q&A, and global search answer surfaces. It does not redesign unrelated admin, settings, or billing views.

## Goal

Introduce a shared long-text virtualization layer that keeps scroll performance stable for knowledge-base style answers, while preserving the existing DOM-based visual rendering and interaction model.

The target surfaces are:

- main workspace chat in `frontend_rust/crates/web-ui/src/components/chat/chat_panel.rs`
- public shared notebook Q&A in `frontend_rust/crates/web-ui/src/routes/shared.rs`
- global search answer surface in `frontend_rust/crates/web-ui/src/routes/search.rs`

## Non-Goals

- No rewrite of final message rendering from DOM to Canvas, SVG, or WebGL
- No expansion into admin tables, settings tabs, billing pages, or generic list virtualization across the entire frontend
- No first-pass virtualization of every auxiliary chip, metric badge, or table row
- No image-height prediction through `pretext`
- No backend API contract changes
- No requirement that SSR use `pretext`; SSR may fall back to non-predictive behavior

## Problem Summary

`context-osv6` is a knowledge-base product, not a short-message chat client. A realistic session can contain:

- long assistant answers, often hundreds or thousands of characters
- repeated citations and source metadata
- multimodal blocks such as inline image cards
- dozens of rounds, enough to produce hundreds of viewport-heights of scroll distance

The current frontend renders these flows as ordinary DOM lists with no windowing:

- workspace chat maps every message directly into the scroll container
- shared notebook Q&A appends streamed and final answer sections directly into the page flow
- global search renders answer and source blocks directly into the page flow

This is simple and correct, but it leaves performance exposed in the exact user journey that matters:

- dragging upward through long historical answers
- reopening long sessions
- streaming a new long answer while old long answers stay mounted
- maintaining scroll position when long text height changes as streaming progresses

The current codebase does not yet show pathological `getBoundingClientRect()` / `offsetHeight` measurement loops for text layout. That is important: the immediate problem is not current measurement thrash in existing code. The problem is total DOM size, layout work, and paint work once long-answer sessions become large.

This matters because a pure DOM virtualization layer with naive estimated heights would reduce DOM count, but would still leave visible jumpiness for long, variable-height answers. The design therefore combines:

- variable-height DOM virtualization
- browser-side `pretext` prediction for text-body height
- sparse DOM reconciliation for final correctness

## Why Pretext Is In Scope

The motivating external evidence is not that `pretext` replaces the entire frontend renderer. The useful claim is narrower:

- `pretext` can predict multiline text height before DOM layout
- it avoids reflow-triggering measurement as the primary estimation path
- it has emerging community validation in long-text and virtualized list use cases

For `context-osv6`, the realistic value is:

1. reduce jumpiness when a long answer first enters the virtualized window
2. reduce dependence on immediate DOM measurement for off-screen item sizing
3. make cross-surface long-text virtualization share one prediction model

`pretext` is therefore a measurement and prediction dependency, not a UI rendering replacement.

## Design Principles

### 1. Keep DOM As The Final Rendering Model

Existing `Leptos` components remain the source of truth for visible UI:

- `ChatBubble`
- shared notebook answer sections
- search result answer and source cards

The project should not switch to canvas-drawn messages. Accessibility, native selection, image behavior, and existing click handling are worth preserving.

### 2. Virtualize The Scroll Containers, Not The Product Semantics

Each target surface keeps its existing user-visible structure. The change is in how list items are mounted:

- off-screen historical items become spacers plus cached height state
- in-window items remain ordinary DOM nodes
- the actively streaming tail remains mounted

### 3. Predict First, Reconcile Later

Prediction should be the default for off-screen and newly entering items:

- use `pretext` for primary text-body height prediction
- use deterministic non-text block additions for chips, images, and metadata containers
- once mounted in the window, reconcile to actual DOM height

This avoids over-rotating into fragile exactness requirements before the item is visible.

### 4. Scroll Stability Is A Product Requirement

The implementation must not trade “fewer DOM nodes” for “more jumps.”

The final behavior must preserve:

- bottom-follow when the user is already at the bottom
- no forced bottom-follow when the user scrolls upward into history
- position-stable upward dragging even when mounted item heights are corrected

## Target Architecture

The feature is implemented as one shared frontend subsystem with four layers.

### 1. `VirtualTextList`

A shared `Leptos` component that owns:

- the scroll container integration
- viewport window calculation
- overscan rules
- top and bottom spacer sizing
- visible range derivation
- scroll-anchor preservation
- bottom-follow behavior

This component does not know whether its items came from workspace chat, search, or public sharing. It only knows about virtual items and height state.

### 2. `TextLayoutPredictor`

A browser-side adapter that:

- loads and owns the `pretext` integration
- caches `prepare()` results by text and typography profile
- runs `layout()` by container width bucket
- returns predicted text height to the Rust/Leptos side

This layer is browser-only. On SSR or unsupported environments, it returns a fallback result without blocking render.

### 3. `MeasurementReconciler`

A small DOM measurement layer that:

- measures only currently mounted items
- stores the final measured height
- updates virtual list height state incrementally
- applies scroll compensation instead of global repositioning

This is intentionally sparse. The design explicitly rejects a full DOM measurement pass across the whole history.

### 4. `SurfaceAdapters`

Small adapters that translate page-specific data into shared virtual items:

- workspace chat adapter
- shared notebook Q&A adapter
- global search answer adapter

These adapters are responsible for mapping current product state into a common item model without flattening page-specific behavior into one giant component.

## Shared Data Model

The design standardizes long-text list entries into three core structures.

### `VirtualTextItem`

Each windowed row is modeled as:

- `id: String`
  Stable identity for caching, scroll anchoring, and reconciliation.

- `kind: VirtualItemKind`
  Distinguishes `chat_message`, `shared_answer`, `search_answer`, `citation_group`, `source_group`, and small supporting variants.

- `text_body: String`
  The text content whose multiline height is predicted via `pretext`.

- `aux_blocks: Vec<VirtualAuxBlock>`
  Non-primary substructures such as:
  - inline citation button zone
  - image block
  - status banner
  - source metadata row
  - chip cluster

- `style_profile: TypographyProfile`
  Explicit typography input for prediction:
  - font family
  - font size
  - line height
  - horizontal padding
  - width deductions caused by avatars, gutters, badges, or buttons

- `render_mode: RenderMode`
  Identifies `static`, `streaming`, `tail_pinned`, or other lifecycle modes that affect mounting and recalculation.

### `VirtualHeightState`

Each item has tracked height state:

- `predicted_px: f64`
- `measured_px: Option<f64>`
- `effective_px: f64`
- `width_bucket: u32`
- `confidence: HeightConfidence`

Rules:

- `effective_px = measured_px` when available
- otherwise `effective_px = predicted_px`
- width-bucket changes invalidate only affected predictions

### `VirtualAnchor`

Scroll stability uses an item anchor instead of raw `scrollTop`:

- `item_id: String`
- `offset_within_item: f64`
- `mode: AnchorMode`

This is mandatory because predicted and measured heights may differ. Raw `scrollTop` alone is too unstable for long-answer sessions.

## Height Prediction Model

### Primary Height Source

For text-heavy items:

1. `pretext.prepare()` is cached by:
   - normalized text body
   - typography profile
   - locale

2. `pretext.layout()` runs by width bucket and returns text-body height.

3. The predictor then adds deterministic shell offsets:
   - outer padding
   - block spacing
   - known fixed chrome such as headers or metadata rows

### Auxiliary Block Handling

The design does not attempt to force every visual element through `pretext`.

Instead:

- pure text body uses `pretext`
- image blocks use explicit reserved dimensions from existing metadata or a fixed fallback
- citation chips and badges use coarse additive estimates
- measured DOM height corrects any residual mismatch once the item becomes visible

This is the right trade-off for first pass:

- prediction quality improves where variance is largest
- implementation complexity stays bounded

### Width Bucketing

Predictions are keyed by width bucket, not raw width, to avoid excessive recalculation during drag or responsive resizing.

Initial recommended buckets:

- workspace desktop message column
- workspace mobile message column
- shared notebook answer column
- search answer column

If fine-grained bucketing becomes necessary later, it should be introduced only after runtime traces show real prediction instability.

## Surface Integration

### 1. Workspace Chat

Current surface:

- `frontend_rust/crates/web-ui/src/components/chat/chat_panel.rs`
- `frontend_rust/crates/web-ui/src/components/chat/chat_bubble.rs`

Integration rules:

- Replace the current full `messages.get().into_iter().map(...)` list with `VirtualTextList`
- Keep `ChatBubble` as the visible renderer for mounted items
- One user or assistant message maps to one `VirtualTextItem`
- Citation chips and inline image blocks stay inside the message item as `aux_blocks`
- The currently streaming assistant message remains mounted and is never windowed out

This preserves all current click behavior and citation interactions while reducing DOM size for historical messages.

### 2. Shared Workspace Q&A

Current surface:

- `frontend_rust/crates/web-ui/src/routes/shared.rs`

Integration rules:

- The interactive Q&A result region becomes a virtualized stack
- Streaming answer and final answer are modeled as long-text virtual items
- Citation and retrieved-source sections become sectioned virtual items
- The surrounding page shell, prompt form, and non-result page content remain ordinary DOM

This keeps the page behavior intact while virtualizing the highest-cost region.

### 3. Global Search

Current surface:

- `frontend_rust/crates/web-ui/src/routes/search.rs`

Integration rules:

- The answer block and long source preview region use the shared virtual stack
- Workspace and session result cards remain non-virtualized in the first pass unless they grow into an actual bottleneck
- Search continues to render as a single page, but its long-result region uses the shared measurement and scrolling model

This intentionally focuses on long-text variance, not on short metadata cards.

## Scroll Behavior

Scroll handling is the highest-risk part of the design. The following behaviors are fixed.

### 1. Bottom-Follow

When the user is within a small threshold of the bottom, the list auto-follows new streamed content.

Recommended initial threshold:

- `80px` from the bottom edge

If the user scrolls above that threshold:

- automatic bottom-follow is disabled
- new streamed content must not steal scroll position

### 2. Historical Scroll Anchoring

When browsing history:

- preserve the top visible item plus in-item offset
- compensate scroll position when heights above the anchor are corrected

The design explicitly rejects naive “save and restore `scrollTop`” logic for long variable-height items.

### 3. Incremental Height Reconciliation

When a mounted item’s measured height differs from its predicted height:

- update only that item’s effective height
- adjust aggregate spacer totals
- apply anchor compensation or bottom compensation depending on current scroll mode

The design rejects global full-list recomputation during reconciliation.

### 4. Streaming Tail Handling

The newest streaming answer is special:

- it stays mounted
- prediction updates are throttled
- DOM reconciliation is allowed to dominate while it is visible

This avoids pathological re-windowing or flicker on the most interactive item.

## Fallback Behavior

The feature must degrade safely in three layers.

### 1. Predictor Fallback

If `pretext` is unavailable:

- fall back to heuristic height estimates plus DOM reconciliation
- do not block rendering
- do not disable virtualization automatically

### 2. Surface Flag Fallback

Each target surface can independently disable the virtualization stack if needed:

- workspace chat
- shared notebook Q&A
- global search

### 3. Global Kill Switch

One frontend feature flag disables the full subsystem and restores the current all-DOM list behavior.

This is required for rollout safety.

## Testing Strategy

### Unit Tests

Required coverage:

- item-model mapping from each surface adapter
- width bucket invalidation rules
- anchor compensation math
- effective height selection logic
- streaming tail pinning rules

### Component Tests

Required coverage:

- only visible window items mount
- spacer totals match windowed range
- streaming tail stays mounted
- width changes invalidate predictions correctly

### Browser Integration Tests

Required coverage:

- long session upward drag remains position-stable
- user scrolling upward is not pulled back by bottom-follow
- returning to bottom resumes bottom-follow
- predictor failure falls back without blanking content
- long shared notebook answer remains stable during streaming
- long search result answer remains stable during resize

### Performance Regression Checks

At minimum compare current behavior against the feature on synthetic long-answer fixtures:

- DOM node count
- initial long-session open time
- upward drag smoothness over a long historical session
- memory trend during long streaming sessions

The first implementation does not need a perfect browser lab benchmark page, but it does need repeatable evidence that the change improves the actual product surfaces.

## Rollout Plan

The feature should ship in four stages.

### Stage 1: Workspace Chat Virtualization Without Pretext

Goal:

- establish stable variable-height virtualization
- validate anchor and tail behavior
- keep prediction simple

### Stage 2: Workspace Chat Pretext Prediction

Goal:

- replace coarse height estimates for long text body
- verify reduced jumpiness for entering items

### Stage 3: Shared Workspace Q&A And Global Search

Goal:

- reuse the stabilized shared infrastructure
- align cross-surface answer behavior

### Stage 4: Prediction Refinement

Goal:

- extend prediction coverage to additional long preview blocks if needed
- tune width buckets and shell offsets based on production traces

## Acceptance Criteria

The design is considered successful only if all of the following are true.

### User-visible criteria

- Long workspace sessions no longer keep the entire message history mounted in the DOM
- Upward dragging through long historical answers is visibly smoother than current behavior
- Streaming a new long answer does not pull the user to bottom if they are reading history
- Entering items do not visibly “jump” in a severe way when their actual height is reconciled
- Shared notebook Q&A and global search answer surfaces behave consistently with workspace chat

### Engineering criteria

- `pretext` integration is optional at runtime and has a safe fallback
- mounted DOM measurement is sparse and window-limited
- shared infrastructure is reused across all three target surfaces
- existing page-level rendering components remain intact

## Risks And Mitigations

### Risk 1: Prediction mismatch still causes visible jumps

Mitigation:

- keep measured reconciliation
- use anchor compensation
- start with text-body prediction only instead of trying to model every sub-block exactly

### Risk 2: JS interop complexity leaks across the codebase

Mitigation:

- isolate `pretext` behind one browser-side predictor module
- keep Rust-side code dependent on a narrow predictor interface

### Risk 3: Streaming updates become more complex than the list virtualization itself

Mitigation:

- pin the streaming tail
- throttle prediction updates for streaming text
- reconcile the tail differently from historical stable items

### Risk 4: Over-generalizing the feature slows delivery

Mitigation:

- limit scope to three long-text surfaces only
- reject a generic all-list virtualization framework in the first implementation

## File-Level Direction

Expected new or modified areas include:

- `frontend_rust/crates/web-ui/src/components/chat/chat_panel.rs`
- `frontend_rust/crates/web-ui/src/routes/shared.rs`
- `frontend_rust/crates/web-ui/src/routes/search.rs`
- `frontend_rust/crates/web-ui/src/state/chat.rs`
- new shared virtual list component files under `frontend_rust/crates/web-ui/src/components/`
- new virtual list state / model files under `frontend_rust/crates/web-ui/src/state/`
- new browser capability / predictor integration files under `frontend_rust/crates/web-ui/src/platform/`

Exact task decomposition belongs in the implementation plan. This design intentionally freezes the architecture and behavior boundaries first.
