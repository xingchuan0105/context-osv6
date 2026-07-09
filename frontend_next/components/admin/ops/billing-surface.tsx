"use client";

import {
  AdminMetricCard,
  AdminPageHeading,
  EmptyState,
  ErrorState,
  LoadingState,
  adminText,
  formatAdminError,
  useAdminBillingOverviewQuery,
  useAuth,
  useUiPreferences,
} from "./shared";

export function AdminBillingSurface() {
  const { token, user } = useAuth();
  const actorId = user?.id;
  const { locale } = useUiPreferences();
  const billingQuery = useAdminBillingOverviewQuery(actorId, token);
  const overview = billingQuery.data ?? null;
  const loading = Boolean(token) && billingQuery.isPending;

  return (
    <section style={{ display: "grid", gap: "1rem" }}>
      <AdminPageHeading
        title={adminText(locale, "admin.billing.sectionTitle")}
        subtitle={adminText(locale, "admin.billing.sectionSubtitle")}
      />
      {billingQuery.error ? <ErrorState message={formatAdminError(locale, billingQuery.error)} /> : null}
      {loading ? (
        <LoadingState copy={adminText(locale, "common.loading")} />
      ) : !overview ? (
        <EmptyState copy={adminText(locale, "ops.empty")} />
      ) : (
        <div style={{ display: "grid", gap: "0.8rem", gridTemplateColumns: "repeat(auto-fit, minmax(12rem, 1fr))" }}>
          <AdminMetricCard label={adminText(locale, "billing.active")} tone="success" value={overview.active_subscriptions.toString()} />
          <AdminMetricCard label={adminText(locale, "billing.pastDue")} tone="warning" value={overview.past_due_subscriptions.toString()} />
          <AdminMetricCard label={adminText(locale, "billing.unpaid")} tone="danger" value={overview.unpaid_subscriptions.toString()} />
          <AdminMetricCard label={adminText(locale, "billing.canceled")} value={overview.canceled_subscriptions.toString()} />
        </div>
      )}
    </section>
  );
}
