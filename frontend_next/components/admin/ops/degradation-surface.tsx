"use client";

import {
  AdminMetricCard,
  AdminPageHeading,
  EmptyState,
  ErrorState,
  LoadingState,
  adminText,
  formatAdminError,
  useAdminDegradationStatusQuery,
  useAuth,
  useUiPreferences,
} from "./shared";

export function AdminDegradationSurface() {
  const { token, user } = useAuth();
  const actorId = user?.id;
  const { locale } = useUiPreferences();
  const degradationQuery = useAdminDegradationStatusQuery(actorId, token);
  const status = degradationQuery.data ?? null;
  const loading = Boolean(token) && degradationQuery.isPending;

  return (
    <section style={{ display: "grid", gap: "1rem" }}>
      <AdminPageHeading
        title={adminText(locale, "admin.degradation.sectionTitle")}
        subtitle={adminText(locale, "admin.degradation.sectionSubtitle")}
      />
      {degradationQuery.error ? <ErrorState message={formatAdminError(locale, degradationQuery.error)} /> : null}
      {loading ? (
        <LoadingState copy={adminText(locale, "common.loading")} />
      ) : !status ? (
        <EmptyState copy={adminText(locale, "ops.empty")} />
      ) : (
        <div style={{ display: "grid", gap: "0.8rem", gridTemplateColumns: "repeat(auto-fit, minmax(12rem, 1fr))" }}>
          <AdminMetricCard label={adminText(locale, "ops.failedDocuments")} tone="danger" value={status.failed_documents.toString()} />
          <AdminMetricCard label={adminText(locale, "degradation.guardEvents24h")} tone="warning" value={status.recent_guard_events.toString()} />
          <AdminMetricCard label={adminText(locale, "degradation.shareAccessEvents24h")} value={status.share_access_events.toString()} />
        </div>
      )}
    </section>
  );
}
