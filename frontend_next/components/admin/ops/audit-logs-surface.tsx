"use client";

import { useState } from "react";

import type { AdminAuditLogEntry, AdminAuditLogQuery } from "./shared";
import {
  AdminPageHeading,
  EmptyState,
  ErrorState,
  LoadingState,
  adminText,
  auditActionLabel,
  auditResourceTypeLabel,
  downloadTextFile,
  formatAdminError,
  formatTimestamp,
  useAdminAuditLogsQuery,
  useAuth,
  useExportAdminAuditLogsCsvMutation,
  useUiPreferences,
} from "./shared";

import styles from "./audit-logs-surface.module.css";

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
    <section className={styles.container}>
      <AdminPageHeading
        title={adminText(locale, "admin.auditLogs.sectionTitle")}
        subtitle={adminText(locale, "admin.auditLogs.sectionSubtitle")}
      />
      {error ? <ErrorState message={formatAdminError(locale, error)} /> : null}
      <section className={`app-inline-surface ${styles.panel}`}>
        <div className={styles.filtersGrid}>
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
        <div className={styles.footerRow}>
          <div className={styles.footerMeta}>
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
          <section className={`app-inline-surface ${styles.tableWrapper}`}>
            <table className={styles.table}>
              <thead className={styles.tableHead}>
                <tr>
                  {[
                    adminText(locale, "common.action"),
                    adminText(locale, "common.resource"),
                    adminText(locale, "common.resourceId"),
                    adminText(locale, "audit.ownerUserId"),
                    adminText(locale, "common.actor"),
                    adminText(locale, "common.time"),
                  ].map((heading) => (
                    <th className={styles.tableHeaderCell} key={heading}>
                      {heading}
                    </th>
                  ))}
                </tr>
              </thead>
              <tbody>
                {items.map((entry: AdminAuditLogEntry) => (
                  <tr className={styles.tableRow} key={entry.id}>
                    <td className={styles.tableCell}>{auditActionLabel(locale, entry.action)}</td>
                    <td className={styles.tableCell}>{auditResourceTypeLabel(locale, entry.resource_type)}</td>
                    <td className={styles.tableCell}>{entry.resource_id}</td>
                    <td className={styles.tableCell}>{entry.owner_user_id ?? "—"}</td>
                    <td className={styles.tableCell}>{entry.actor_id ?? "—"}</td>
                    <td className={styles.tableCell}>{formatTimestamp(entry.created_at, locale)}</td>
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
