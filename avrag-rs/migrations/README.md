# Migrations

This directory holds the PostgreSQL schema migrations for the Rust rewrite.

Rules for M1 + M2:
- PostgreSQL remains the authority for permissions, versions, audit, and body storage.
- Core business tables must include `org_id`.
- Row-level security is mandatory and is expected to rely on `current_setting('app.current_org', true)`.
- Every migration must provide both `up` and `down` scripts.

Initial M1 + M2 focus:
- tenant-aware core entities
- notebook/document/chunk storage
- chat sessions
- audit trail
- usage events

The first migration intentionally stays minimal so the rest of the workspace can start integrating against stable table names and the RLS convention.
