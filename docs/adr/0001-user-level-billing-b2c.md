# ADR 0001: User-Level Billing for B2C Business Model

## Status
Decided

## Context
The codebase was originally structured around B2B/Team billing, where a subscription and its associated quotas were bound to an `OrgId` (Organization ID). However, the business model has pivoted to B2C, requiring subscriptions, checkout sessions, customer portal sessions, and billing quotas to be managed on an individual user basis (`UserId`).

This shift implies significant architectural changes:
1. All billing API routing handlers and implementations under `avrag-rs/crates/transport-http/src/routes/billing.rs` and `avrag-rs/crates/billing` must switch from using `org_id` to `user_id`.
2. Database schema mappings (e.g. `subscriptions` table mapping) must store and query by `user_id` instead of `org_id`.
3. The quota check and enforcement services (e.g. `quota_service.rs`) must evaluate limits relative to individual users.

## Decision
All subscriptions, checkout flows, and Stripe webhook synchronization actions will be bound to individual `UserId` objects. The database and backend interfaces will be refactored to support user-scoped billing entities.

## Consequences
- **Code refactoring**: Extensive updates across `crates/billing` and `transport-http` crates.
- **Database schema changes**: Tables tracking Stripe subscriptions and billing metrics will transition fields from `org_id` to `user_id`.
- **B2C Alignment**: The product frontend can directly prompt personal upgrade pages without requiring complex B2B workspace setup.
