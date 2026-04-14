# auth

Tenant-aware authentication primitives for the Rust rewrite.

Scope for M1 + M2:
- hold the authenticated org/user/api-key context
- expose permission checks
- provide fail-closed guards before downstream storage access

This crate intentionally avoids HTTP framework coupling. Middleware adapters can wrap these types later.
