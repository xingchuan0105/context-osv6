"use client";

import {
  AdminMetricCard,
  AdminPageHeading,
  EmptyState,
  ErrorState,
  LoadingState,
  adminText,
  formatAdminError,
  useAdminRagHealthQuery,
  useAuth,
  useUiPreferences,
} from "./shared";

export function AdminRagHealthSurface() {
  const { token, user } = useAuth();
  const actorId = user?.id;
  const { locale } = useUiPreferences();
  const ragHealthQuery = useAdminRagHealthQuery(actorId, token);
  const status = ragHealthQuery.data ?? null;
  const loading = Boolean(token) && ragHealthQuery.isPending;

  return (
    <section style={{ display: "grid", gap: "1rem" }}>
      <AdminPageHeading
        title={adminText(locale, "admin.nav.ragHealth")}
        subtitle={adminText(locale, "rag.subtitle")}
      />
      {ragHealthQuery.error ? <ErrorState message={formatAdminError(locale, ragHealthQuery.error)} /> : null}
      {loading ? (
        <LoadingState copy={adminText(locale, "common.loading")} />
      ) : !status ? (
        <EmptyState copy={adminText(locale, "ops.empty")} />
      ) : (
        <div style={{ display: "grid", gap: "0.8rem", gridTemplateColumns: "repeat(auto-fit, minmax(12rem, 1fr))" }}>
          <AdminMetricCard label={adminText(locale, "common.failedDocs")} tone="danger" value={status.failed_documents.toString()} />
          <AdminMetricCard label={adminText(locale, "common.queuedTasks")} tone="warning" value={status.queued_tasks.toString()} />
          <AdminMetricCard label={adminText(locale, "ops.processing")} value={status.processing_tasks.toString()} />
          <AdminMetricCard label={adminText(locale, "ops.guardEvents")} tone="success" value={status.recent_guard_events.toString()} />
        </div>
      )}
    </section>
  );
}
