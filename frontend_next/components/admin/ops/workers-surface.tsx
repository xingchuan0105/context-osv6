"use client";

import { useState } from "react";

import type {
  AdminAuditLogEntry,
  AdminAuditLogQuery,
  AdminFeatureFlagChangeRequest,
  AdminFeatureFlagEntry,
} from "../../../lib/admin/client";
import { useAuth } from "../../../lib/auth/context";
import { useUiPreferences } from "../../../lib/ui-preferences";
import {
  adminText,
  auditActionLabel,
  auditResourceTypeLabel,
  featureFlagCategoryLabel,
  featureFlagSourceLabel,
  featureFlagStatusLabel,
  formatAdminError,
  workerRuntimeLabel,
} from "../admin-i18n";
import {
  useAdminAuditLogsQuery,
  useAdminBillingOverviewQuery,
  useAdminDegradationStatusQuery,
  useAdminFeatureFlagRequestsQuery,
  useAdminFeatureFlagsQuery,
  useAdminRagHealthQuery,
  useAdminWorkerStatusQuery,
  useExportAdminAuditLogsCsvMutation,
  useRequestAdminFeatureFlagChangeMutation,
  useReviewAdminFeatureFlagChangeMutation,
} from "../admin-queries";
import {
  AdminMetricCard,
  AdminPageHeading,
  EmptyState,
  ErrorState,
  LoadingState,
} from "../admin-core-surfaces";

function formatTimestamp(value: number, locale: "zh-CN" | "en") {
  return new Intl.DateTimeFormat(locale, {
    year: "numeric",
    month: "short",
    day: "numeric",
    hour: "2-digit",
    minute: "2-digit",
  }).format(new Date(value * 1000));
}

function downloadTextFile(filename: string, contents: string) {
  const blob = new Blob([contents], { type: "text/plain;charset=utf-8" });
  const url = URL.createObjectURL(blob);
  const link = document.createElement("a");
  link.href = url;
  link.download = filename;
  link.click();
  URL.revokeObjectURL(url);
}

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

