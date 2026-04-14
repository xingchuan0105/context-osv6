# storage-pg

Tenant-safe PostgreSQL access helpers.

Key constraints encoded here:
- all business queries must execute inside a transaction with `app.current_org` configured
- PostgreSQL remains the authority for permissions, body retrieval, and audit
- callers should fail closed when `org_id` is missing or mismatched
