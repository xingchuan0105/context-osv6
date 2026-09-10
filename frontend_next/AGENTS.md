# Frontend agent rules — frontend_next

Scope: `frontend_next/` (Next.js, React, TypeScript, pnpm). Inherits [repository rules](../AGENTS.md). Paths below are relative to this directory unless stated otherwise.

## Product IA and navigation

Before changing global navigation, top-bar entries, shells, or monetization entry points, update [Product IA](../docs/design/PRODUCT_IA.md) (Jobs / Sitemap / Canonical / Shell), then implement. Audit context: [IA audit](../docs/design/PRODUCT_IA_AUDIT.md).

- Canonical destinations live only in `lib/navigation/nav-config.ts`. `lib/site-map.ts` is for multi-site discovery, not in-app IA.
- New/personal conversation and auth completion: `/chat`; workspace overview: `/dashboard`; membership checkout: `/pricing`; top-up: `/pricing#topup`; BYOK: `/settings?tab=providers`; client: `/desktop`.
- Other CTAs may deep-link, but must not create another completion path. Upgrade modals explain the offer; payment belongs on the canonical page.
- Onboarding/product-map help belongs in a modal or `/help`, opened from a weak entry such as top-bar「上手」. Do not add a permanent encyclopedia sidebar beside the workspace list.

## Style

- Font-weight tokens are `400`; no numeric font-weight ≥ `500`.
- No bare hex colors outside token files.
- No drop shadows except allowlisted floating overlays.
- The mechanical baseline is `tests/style/design-baseline.test.ts`; route existence is guarded by `tests/navigation/nav-config.test.ts`.

## Verification

Follow the root time-cost rule before runs. For frontend code changes, run relevant `pnpm test run` checks and `pnpm typecheck` from `frontend_next/`. Include the style/navigation checks when those areas change. Documentation-only edits need no frontend build.
