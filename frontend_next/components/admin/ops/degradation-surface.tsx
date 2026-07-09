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

