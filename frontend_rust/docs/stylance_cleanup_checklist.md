# Stylance Cleanup Checklist

## Objective

Retire the remaining high-value legacy global styling outside `dashboard` and `workspace` before any new feature wiring work starts.

## Scope order

1. `shared` public collaboration flows
2. `invite` acceptance flow
3. `search` route
4. `admin/feature_flags`
5. `admin` legacy panels

## Batch 1: shared flows

- [ ] Add route-level Stylance module for `routes/shared/*`
- [ ] Add component-level Stylance module for `components/share/mod.rs`
- [ ] Replace `app-page-shell`, `app-surface-card`, `app-tab-bar`, `app-tab-button` in shared routes
- [ ] Replace remaining utility strings in:
  - `shared_kb_page.rs`
  - `shared_kb_overview.rs`
  - `share_center_page.rs`
- [ ] Remove inline style debt from `components/share/mod.rs`
- [ ] Remove `components/share/mod.rs` from `scripts/ui_drift_allowlist.txt`
- [ ] Run `cargo fmt`
- [ ] Run `cargo check -p frontend-web-ui`

## Batch 2: supporting public pages

- [ ] Migrate `routes/invite.rs`
- [ ] Migrate `routes/search.rs`
- [ ] Re-run `ui_drift_guard.py`

## Batch 3: admin long tail

- [ ] Migrate `routes/admin/feature_flags/page.rs`
- [ ] Migrate `routes/admin/feature_flags/flag_card.rs`
- [ ] Migrate `routes/admin/feature_flags/request_card.rs`
- [ ] Audit `components/admin/mod.rs` and split by panel if needed

## Exit criteria

- No new utility strings introduced in migrated files
- No inline `style=` left outside explicit allowlist debt
- `shared` route cluster uses only token-driven Stylance modules for visuals
- Preview and live pages continue to compile without behavior changes
