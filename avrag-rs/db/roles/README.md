# DB role provisioning (operator-run, not sqlx migrations)

This directory holds **hand-run operator SQL** for the three-layer role model.
These files are **not** part of the sqlx migration chain — they require
cluster-level privileges (`CREATE ROLE`, `ALTER DATABASE ... OWNER`) that the
migration role must not have.

| File | Purpose |
|------|---------|
| `002_runtime_grants.sql` | Create/harden `avrag` (owner/migrator; holds `CREATE` on schema `public` — required for `CREATE INDEX`/`CREATE TABLE` migrations), create `avrag_runtime` (DML only), grant business tables, set default privileges, assert invariants. |
| `002_runtime_grants.down.sql` | Revoke runtime grants (keeps the role). |

## Runbook connection

Execute with a **cluster-admin** session (local superuser on the server), never
with the runtime DSN:

```bash
psql "$MIGRATION_DATABASE_URL" -v runtime_password='…' \
  -f avrag-rs/db/roles/002_runtime_grants.sql
```

`MIGRATION_DATABASE_URL` points at the owner/migrator role `avrag` for the
application database; provisioning requires elevating to a superuser session
(`SET ROLE avrag_cluster_admin;` or a `postgres`-user shell).

## Invariants asserted at the end of 002

- `avrag_runtime` has no `SUPERUSER` / `BYPASSRLS` / `CREATEDB` / `CREATEROLE`,
  does not `INHERIT`, and holds **no role memberships**.
- Every `public` table is owned by `avrag` (schema/database owned by
  `avrag_cluster_admin`).
- `_sqlx_migrations` and `_org_owner_map_mig` are excluded from runtime grants.

## Why a file, not a migration

Migrations run under the owner role through sqlx and must stay replayable on
fresh dev databases where the cluster roles do not exist. Role/`OWNER` changes
are cluster state, not schema state — an operator concern, asserted (not
assumed) by the runtime guard in the API/worker binaries.