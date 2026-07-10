# Migrations

This directory holds the PostgreSQL schema migrations for the Rust rewrite.

## Current product rules (B2C personal account)

- PostgreSQL remains the authority for permissions, versions, audit, and body storage.
- Tenant root is **`owner_user_id`** (maps to `users.id`). There is **no** `organizations` table and **no** product `org_id`.
- Row-level security uses `current_setting('app.current_user', true)` (not `app.current_org`).
- Every migration must provide both `up` and `down` scripts.
- New migrations **must not** reintroduce `org_id`, `organizations`, or `app.current_org`.

## Historical note (do not “fix” old numbers casually)

Migrations **before** `0056_remove_org_tenant` still contain intermediate `org_id` / `organizations` DDL. That is **applied history** for existing databases and for replaying the chain on a fresh DB. Changing those files rewrites checksums and breaks `sqlx migrate`.

- **0056** remaps `org_id` → `owner_user_id` and drops `organizations`.
- **0057** residual cleanup: drop leftover `users.org_id`, add `users.blocked`.

Do not squash or rewrite pre-0056 SQL unless you intentionally reset all environments and recompute checksums.

## Initial M1 + M2 focus (historical)

- account-aware core entities (formerly multi-tenant org)
- workspace/document/chunk storage
- chat sessions
- audit trail
- usage events
