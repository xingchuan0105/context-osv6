# cache-redis

Org-scoped Redis key construction for cache, lock, and idempotency concerns.

The M1 + M2 implementation is intentionally thin:
- require `org_id` to generate any key
- centralize naming for lock and idempotency semantics
- avoid a direct Redis client dependency until the app crate wires the runtime
