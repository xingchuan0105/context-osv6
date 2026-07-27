# Figma Design Sign-off Template

## Goal

Use 3 sample pages to derive the full product UI system, complete all key pages and states in Figma, and lock visual decisions before coding.

## Inputs

- Product PRD: `PRD_RUST.md`
- Visual references: 3 sample pages from user
- Existing domain model and API contracts (read-only context)

## Workflow

1. Build foundations
- Color tokens (semantic + component-level)
- Typography scale
- Spacing and radius scale
- Shadow and border rules
- Layout grid (desktop + mobile)

2. Build core components
- Top navigation/header
- Left thread rail
- Chat message cards and composer
- Source list items and actions
- Notes cards and editor blocks
- Buttons, inputs, badges, tabs, menus, modals

3. Build pages (v1 scope)
- Dashboard/Home
- Workspace/Chat
- Workspace list and detail transitions
- Source management view
- Notes management view
- Settings/auth-related pages needed by PRD flow

4. Build state variants (required)
- Empty states
- Loading/skeleton states
- Error/failure states
- Hover/focus/selected/disabled states
- Responsive (desktop + mobile)

5. Sign-off checklist
- PRD consistency
- Interaction consistency
- Design token consistency
- Accessibility baseline (contrast and focus visibility)
- Copy and localization readiness

## Review gates

Gate 1: Foundations complete  
Gate 2: Core components complete  
Gate 3: Page coverage complete  
Gate 4: State coverage complete  
Gate 5: Final sign-off and implementation handoff

## Handoff package (after sign-off)

- Final Figma file URL
- Page inventory with status
- Component inventory with status
- Token list snapshot
- Interaction notes
- Implementation priority order
