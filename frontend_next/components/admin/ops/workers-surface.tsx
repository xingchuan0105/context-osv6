"use client";

import {
  AdminMetricCard,
  AdminPageHeading,
  EmptyState,
  ErrorState,
  LoadingState,
  adminText,
  formatAdminError,
  useAdminWorkerStatusQuery,
  useAuth,
  useUiPreferences,
  workerRuntimeLabel,
} from "./shared";

export function AdminWorkersSurface() {
  const { token, user } = useAuth();
  const actorId = user?.id;
  const { locale } = useUiPreferences();
  const workersQuery = useAdminWorkerStatusQuery(actorId, token);
  const status = workersQuery.data ?? null;
  const loading = Boolean(token) && workersQuery.isPending;

  return (
    <section style={{ display: "grid", gap: "1rem" }}>
      <AdminPageHeading
        title={adminText(locale, "admin.workers.sectionTitle")}
        subtitle={adminText(locale, "admin.workers.sectionSubtitle")}
      />
      {workersQuery.error ? <ErrorState message={formatAdminError(locale, workersQuery.error)} /> : null}
      {loading ? (
        <LoadingState copy={adminText(locale, "common.loading")} />
      ) : !status ? (
        <EmptyState copy={adminText(locale, "ops.empty")} />
      ) : (
        <div style={{ display: "grid", gap: "0.8rem", gridTemplateColumns: "repeat(auto-fit, minmax(12rem, 1fr))" }}>
          <AdminMetricCard label={adminText(locale, "common.runtime")} value={workerRuntimeLabel(locale, status.runtime_mode)} />
          <AdminMetricCard label={adminText(locale, "common.queued")} tone="warning" value={status.queued_tasks.toString()} />
          <AdminMetricCard label={adminText(locale, "ops.processing")} tone="success" value={status.processing_tasks.toString()} />
          <AdminMetricCard label={adminText(locale, "common.failedDocs")} tone="danger" value={status.failed_documents.toString()} />
        </div>
      )}
    </section>
  );
}
