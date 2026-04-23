# Stylance Migration Plan

## Goal

Retire Tailwind as the primary styling layer and migrate `frontend_rust` to:

- `design_tokens.css`
- shared global base CSS in `index.css`
- colocated Stylance CSS Modules for route and component visuals

## Rules

- No new Tailwind utility strings in new code.
- Approved core pages migrate first.
- Tailwind remains only as a temporary compatibility layer until legacy pages are migrated.

## Migration order

1. `dashboard` approved shell and notebook list/card views
2. `workspace` approved shell and primary rails
3. `shared` route cluster and `components/share` public collaboration panels
4. auth/settings/help/search/admin long-tail routes
5. remove final Tailwind watcher, package dependency, and config only after the last legacy consumer is gone

## Current audit snapshot

- Approved pages already migrated:
  - `dashboard`
  - `workspace`
- Highest remaining legacy hotspots by template class volume:
  - `components/admin/mod.rs`
  - `routes/shared/shared_kb_page.rs`
  - `components/share/mod.rs`
  - `routes/search.rs`
  - `routes/invite.rs`
  - `routes/admin/feature_flags/*`
- Existing `ui_drift_guard.py` is clean, which means current debt is mostly pre-existing utility strings and global classes rather than new violations.

## Next execution slice

1. migrate `routes/shared/shared_kb_page.rs`
2. migrate `routes/shared/shared_kb_overview.rs`
3. migrate `routes/shared/share_center_page.rs`
4. migrate `components/share/mod.rs`
5. remove the `components/share/mod.rs` allowlist entry once inline style debt is gone

## Acceptance gates

- `cargo check -p frontend-web-ui`
- dev server serves both `/pkg/index.css` and `/pkg/stylance.css`
- approved pages render correctly on `http://127.0.0.1:3000`
- screenshot gates remain green for preview pages
- no new Tailwind utility strings are introduced in migrated files

## Risk controls

- migrate page shells first, then shared widgets
- do not mix notebook title semantics into visual logic
- keep API behavior unchanged while moving visuals
- remove old global classes only after migrated routes stop referencing them
