/**
 * Compatibility barrel for admin core surfaces.
 *
 * Implementations live in focused modules; this file re-exports for existing
 * page and ops imports so the split is a pure move (no route churn).
 */

export {
  AdminMetricCard,
  AdminPageHeading,
  EmptyState,
  ErrorState,
  LoadingState,
} from "./admin-shared-ui";

export { AdminHealthSurface } from "./admin-health-surface";
export { AdminOrganizationDetailSurface } from "./admin-org-detail-surface";
export { AdminOrganizationsSurface } from "./admin-orgs-surface";
export { AdminUsageSurface } from "./admin-usage-surface";
export { AdminUsersSurface } from "./admin-users-surface";
