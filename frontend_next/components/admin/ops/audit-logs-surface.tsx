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

export function AdminAuditLogsSurface() {
  const { token, user } = useAuth();
  const actorId = user?.id;
  const { locale } = useUiPreferences();
  const [query, setQuery] = useState("");
  const [actionFilter, setActionFilter] = useState("");
  const [resourceFilter, setResourceFilter] = useState("");
  const [actorFilter, setActorFilter] = useState("");
  const [windowFilter, setWindowFilter] = useState("all");
  const [page, setPage] = useState(1);
  const [perPage, setPerPage] = useState(25);
  const auditQuery: AdminAuditLogQuery = {
    query,
    action: actionFilter || null,
    resource_type: resourceFilter || null,
    actor: actorFilter || null,
    window: windowFilter === "all" ? null : windowFilter,
    page,
    per_page: perPage,
  };
  const auditLogsQuery = useAdminAuditLogsQuery(actorId, token, auditQuery);
  const exportMutation = useExportAdminAuditLogsCsvMutation(actorId, token);
  const response = auditLogsQuery.data ?? null;
  const totalPages = response ? Math.max(1, Math.ceil(response.total / response.per_page)) : 1;
  const items = response?.items ?? [];
  const loading = Boolean(token) && auditLogsQuery.isPending;
  const error = auditLogsQuery.error ?? exportMutation.error ?? null;

  async function handleExport() {
    const csv = await exportMutation.mutateAsync({
      query,
      action: actionFilter || null,
      resource_type: resourceFilter || null,
      actor: actorFilter || null,
      window: windowFilter === "all" ? null : windowFilter,
    });

    downloadTextFile("audit-logs.csv", csv);
  }

  return (
    <section style={{ display: "grid", gap: "1rem" }}>
      <AdminPageHeading
        title={adminText(locale, "admin.auditLogs.sectionTitle")}
        subtitle={adminText(locale, "admin.auditLogs.sectionSubtitle")}
      />
      {error ? <ErrorState message={formatAdminError(locale, error)} /> : null}
      <section className="app-inline-surface" style={{ display: "grid", gap: "0.8rem" }}>
        <div style={{ display: "grid", gap: "0.8rem", gridTemplateColumns: "1.2fr repeat(4, minmax(10rem, 1fr)) 8rem" }}>
          <div>
            <label className="app-form-label" htmlFor="admin-audit-query">
              {adminText(locale, "admin.searchLabel")}
            </label>
            <input className="app-input" id="admin-audit-query" onChange={(event) => { setQuery(event.target.value); setPage(1); }} placeholder={adminText(locale, "admin.searchPlaceholder")} type="text" value={query} />
          </div>
          <div>
            <label className="app-form-label" htmlFor="admin-audit-action">
              {adminText(locale, "common.action")}
            </label>
            <input className="app-input" id="admin-audit-action" onChange={(event) => { setActionFilter(event.target.value); setPage(1); }} placeholder="task_failed" type="text" value={actionFilter} />
          </div>
          <div>
            <label className="app-form-label" htmlFor="admin-audit-resource">
              {adminText(locale, "common.resource")}
            </label>
            <input className="app-input" id="admin-audit-resource" onChange={(event) => { setResourceFilter(event.target.value); setPage(1); }} placeholder="document" type="text" value={resourceFilter} />
          </div>
          <div>
            <label className="app-form-label" htmlFor="admin-audit-actor">
              {adminText(locale, "common.actor")}
            </label>
            <input className="app-input" id="admin-audit-actor" onChange={(event) => { setActorFilter(event.target.value); setPage(1); }} placeholder={adminText(locale, "audit.actorIdPlaceholder")} type="text" value={actorFilter} />
          </div>
          <div>
            <label className="app-form-label" htmlFor="admin-audit-window">
              {adminText(locale, "admin.filter.windowLabel")}
            </label>
            <select className="app-input" id="admin-audit-window" onChange={(event) => { setWindowFilter(event.target.value); setPage(1); }} value={windowFilter}>
              <option value="all">{adminText(locale, "audit.allTime")}</option>
              <option value="24h">{adminText(locale, "audit.last24h")}</option>
              <option value="7d">{adminText(locale, "audit.last7d")}</option>
              <option value="30d">{adminText(locale, "audit.last30d")}</option>
              <option value="90d">{adminText(locale, "audit.last90d")}</option>
            </select>
          </div>
          <div>
            <label className="app-form-label" htmlFor="admin-audit-page-size">
              {adminText(locale, "admin.filter.pageSizeLabel")}
            </label>
            <select className="app-input" id="admin-audit-page-size" onChange={(event) => { setPerPage(Number(event.target.value)); setPage(1); }} value={perPage}>
              <option value={25}>25</option>
              <option value={50}>50</option>
              <option value={100}>100</option>
            </select>
          </div>
        </div>
        <div style={{ display: "flex", justifyContent: "space-between", gap: "1rem", flexWrap: "wrap", fontSize: "0.82rem", color: "hsl(var(--muted-foreground))" }}>
          <div style={{ display: "flex", gap: "1rem", flexWrap: "wrap" }}>
            <span>{adminText(locale, "audit.matchingLogs")} {response?.total ?? 0}</span>
            <span>{adminText(locale, "common.page")} {Math.min(page, totalPages)}/{totalPages}</span>
          </div>
          <button className="app-button-secondary" disabled={exportMutation.isPending} type="button" onClick={() => void handleExport()}>
            {exportMutation.isPending ? adminText(locale, "common.processing") : adminText(locale, "audit.exportCsv")}
          </button>
        </div>
      </section>

      {loading ? (
        <LoadingState copy={adminText(locale, "common.loading")} />
      ) : items.length === 0 ? (
        <EmptyState copy={adminText(locale, "audit.empty")} />
      ) : (
        <>
          <section className="app-inline-surface" style={{ overflowX: "auto", padding: 0 }}>
            <table style={{ width: "100%", borderCollapse: "collapse" }}>
              <thead style={{ background: "hsl(var(--surface-muted))" }}>
                <tr>
                  {[
                    adminText(locale, "common.action"),
                    adminText(locale, "common.resource"),
                    adminText(locale, "common.resourceId"),
                    adminText(locale, "audit.orgId"),
                    adminText(locale, "common.actor"),
                    adminText(locale, "common.time"),
                  ].map((heading) => (
                    <th key={heading} style={{ padding: "0.85rem 1rem", textAlign: "left", fontSize: "0.76rem", color: "hsl(var(--muted-foreground))" }}>
                      {heading}
                    </th>
                  ))}
                </tr>
              </thead>
              <tbody>
                {items.map((entry: AdminAuditLogEntry) => (
                  <tr key={entry.id} style={{ borderTop: "1px solid hsl(var(--border))" }}>
                    <td style={{ padding: "1rem" }}>{auditActionLabel(locale, entry.action)}</td>
                    <td style={{ padding: "1rem" }}>{auditResourceTypeLabel(locale, entry.resource_type)}</td>
                    <td style={{ padding: "1rem" }}>{entry.resource_id}</td>
                    <td style={{ padding: "1rem" }}>{entry.org_id ?? "—"}</td>
                    <td style={{ padding: "1rem" }}>{entry.actor_id ?? "—"}</td>
                    <td style={{ padding: "1rem" }}>{formatTimestamp(entry.created_at, locale)}</td>
                  </tr>
                ))}
              </tbody>
            </table>
          </section>
          <div className="app-button-row">
            <button className="app-button-secondary" disabled={page <= 1} type="button" onClick={() => setPage((currentPage) => Math.max(1, currentPage - 1))}>
              {adminText(locale, "audit.previous")}
            </button>
            <button className="app-button-secondary" disabled={page >= totalPages} type="button" onClick={() => setPage((currentPage) => Math.min(totalPages, currentPage + 1))}>
              {adminText(locale, "audit.next")}
            </button>
          </div>
        </>
      )}
    </section>
  );
}
