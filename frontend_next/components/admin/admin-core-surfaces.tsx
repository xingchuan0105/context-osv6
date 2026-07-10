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
export { AdminAccountDetailSurface } from "./admin-account-detail-surface";
export { AdminAccountsSurface } from "./admin-accounts-surface";
export { AdminUsageSurface } from "./admin-usage-surface";
export { AdminUsersSurface } from "./admin-users-surface";
