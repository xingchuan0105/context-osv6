# Workspace Page Spec

## 1. Page role

`Workspace` is the primary research operating surface for one notebook.
It must let the user complete one closed loop on a single screen:

- choose or create a thread
- ask or continue a question
- inspect the evidence scope
- capture private notes

The page is not a dashboard and not a settings page.
Its first principle is continuous research flow with low context switching.

## 2. Immutable layout contract

The page is a fixed three-column workspace under one top bar.

- Top bar: brand, editable notebook title, primary actions, user/settings access
- Left rail: thread creation, thread search, thread list, row actions
- Center panel: conversation history and a bottom compose surface
- Right rail: source management on top, notes management on bottom

Desktop proportions:

- top bar height: 56px
- left rail width: 248px to 256px visual target
- right rail width: 328px to 340px visual target
- center panel: fill remaining width

Mobile behavior:

- center panel remains the primary surface
- left rail and right rail become overlay drawers
- top bar exposes drawer triggers

## 3. Visual contract

The visual system should read as a Perplexity-like research shell:

- mostly neutral gray-white surfaces
- dark primary action pills
- low chroma borders
- soft shadows only where they clarify hierarchy
- typography should feel product-grade, dense, and editorial rather than marketing-heavy

Visual priorities by region:

- top bar: crisp, restrained, utility-first
- left rail: quiet navigation surface
- center panel: maximum readability and breathing room
- right rail: compact operational sidecar

Surface hierarchy:

- app shell background: muted warm gray
- top bar: white with a hairline bottom border
- center conversation surface: white
- side rails: slightly tinted off-white
- menus and modals: white with stronger shadow and clear border separation

Shape hierarchy:

- row items: 10px to 12px
- search and text inputs: pill or high-radius rounded rectangle
- cards and side panels: 14px to 16px
- primary CTA buttons: full pill

## 4. Functional shell boundaries

The live implementation must be organized around four shell components:

- `TopBar`
- `LeftRail`
- `ChatArea`
- `RightRail`

These components own layout and presentation.
They do not own business fetch logic.
Runtime state continues to come from the existing workspace setup/runtime layer.

## 5. Top bar contract

Required behaviors:

- show the `Context-OS` brand on the left
- show the active notebook name in editable form
- allow inline title editing
- show primary creation action for notebook creation
- expose `Analyze`, `Share`, `API`, `Settings`, and user entry affordances

Visual rules:

- title editing happens inline, not in modal
- action group is compact and horizontally aligned
- icon buttons use subtle hover, not filled emphasis
- only the primary create action may read as the strongest CTA

## 6. Left rail contract

Required blocks in order:

- `New Thread` primary pill
- thread search box
- `Threads` section label
- thread list

Required row states:

- default
- hover
- active
- active menu open
- empty search result
- global empty state

Each row must support:

- open thread
- pin or unpin
- rename
- delete

## 7. Chat area contract

The center panel has two zones:

- scrolling message history
- sticky bottom compose area

Message history rules:

- user message is visually lighter and right-aligned
- assistant message is left-aligned and reads as the dominant narrative block
- citations render as small source pills
- message footer actions are lightweight and secondary

Compose surface rules:

- remains anchored at bottom with gradient fade above it
- includes mode/menu entry on the left
- includes send button on the right
- input area must allow multi-line entry
- focus state must visibly strengthen border/shadow

## 8. Right rail contract

The right rail is split vertically:

- upper section: `Sources`
- lower section: `Notes`

Sources section requirements:

- show total count
- expose `New Source`
- support select all
- support row check state
- support row pin/delete actions
- support URL add flow and upload flow
- support document detail drill-in

Notes section requirements:

- show `New Note`
- show saved note cards
- support open/edit/delete/promote
- support note editor mode in-place inside the right rail

## 9. Editing mode contract

When a note enters editing mode:

- the standard right rail list layout is replaced by the note editor shell
- editing remains inside the right rail, not a modal
- save, promote to source, and delete remain visible without extra navigation

## 10. Business preservation contract

This refactor is visual and structural, not a capability rollback.

Must preserve:

- live notebook title update
- live notebook creation
- live thread loading, opening, renaming, pinning, deleting
- live chat submission and SSE streaming
- live source selection, URL import, upload flow, pin, delete, reindex
- live note create, autosync, delete, promote

Allowed changes:

- shell markup
- semantic class naming
- local presentational decomposition
- copy tweaks where they do not change user meaning

Not allowed:

- replacing live runtime with fake-only data on the live route
- moving core actions behind extra navigation steps
- removing current API-backed operations to simplify the visuals

## 11. Acceptance target

Primary gate for this phase:

- workspace page visual parity under fixed viewport screenshot comparison

Target frame:

- viewport: `1440x1024`
- screenshot route: live workspace route and preview workspace route as needed for debugging
- priority reference: the supplied workspace screenshot and extracted React sample

This phase only needs one page to pass.
The resulting shell and spec format become the template for the remaining pages.
