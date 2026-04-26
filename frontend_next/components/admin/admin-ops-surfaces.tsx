"use client";

import { useState } from "react";

import type {
  AdminAuditLogEntry,
  AdminAuditLogQuery,
  AdminFeatureFlagChangeRequest,
  AdminFeatureFlagEntry,
} from "../../lib/admin/client";
import { useAuth } from "../../lib/auth/context";
import { useUiPreferences } from "../../lib/ui-preferences";
import {
  adminMessage,
  adminText,
  auditActionLabel,
  auditResourceTypeLabel,
  featureFlagCategoryLabel,
  featureFlagSourceLabel,
  featureFlagStatusLabel,
  formatAdminError,
  workerRuntimeLabel,
} from "./admin-i18n";
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
} from "./admin-queries";
import {
  AdminMetricCard,
  AdminPageHeading,
  EmptyState,
  ErrorState,
  LoadingState,
} from "./admin-core-surfaces";

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
        title={adminMessage(locale, "admin.billing.sectionTitle")}
        subtitle={adminMessage(locale, "admin.billing.sectionSubtitle")}
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
        title={adminMessage(locale, "admin.nav.ragHealth")}
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
        title={adminMessage(locale, "admin.workers.sectionTitle")}
        subtitle={adminMessage(locale, "admin.workers.sectionSubtitle")}
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
        title={adminMessage(locale, "admin.degradation.sectionTitle")}
        subtitle={adminMessage(locale, "admin.degradation.sectionSubtitle")}
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

export function AdminFeatureFlagsSurface() {
  const { token, user } = useAuth();
  const actorId = user?.id;
  const { locale } = useUiPreferences();
  const [flagQuery, setFlagQuery] = useState("");
  const [requestStatus, setRequestStatus] = useState("all");
  const [requestReasons, setRequestReasons] = useState<Record<string, string>>({});
  const [reviewNotes, setReviewNotes] = useState<Record<string, string>>({});
  const [busyAction, setBusyAction] = useState("");
  const flagsQuery = useAdminFeatureFlagsQuery(actorId, token);
  const requestsQuery = useAdminFeatureFlagRequestsQuery(actorId, token, requestStatus);
  const requestMutation = useRequestAdminFeatureFlagChangeMutation(actorId, token);
  const reviewMutation = useReviewAdminFeatureFlagChangeMutation(actorId, token);
  const flags = flagsQuery.data ?? [];
  const requests = requestsQuery.data ?? [];
  const filteredFlags = flags.filter((flag) => {
    const query = flagQuery.trim().toLowerCase();

    if (!query) {
      return true;
    }

    return (
      flag.key.toLowerCase().includes(query) ||
      flag.description.toLowerCase().includes(query) ||
      flag.category.toLowerCase().includes(query) ||
      flag.source.toLowerCase().includes(query)
    );
  });
  const error = flagsQuery.error ?? requestsQuery.error ?? requestMutation.error ?? reviewMutation.error ?? null;
  const loading = Boolean(token) && (flagsQuery.isPending || requestsQuery.isPending);

  async function handleRequest(flag: AdminFeatureFlagEntry) {
    const reason = requestReasons[flag.key]?.trim() ?? "";

    if (!reason) {
      return;
    }

    const actionKey = `request:${flag.key}`;
    setBusyAction(actionKey);

    try {
      await requestMutation.mutateAsync({
        flagKey: flag.key,
        requestedEnabled: !flag.enabled,
        reason,
      });
      setRequestReasons((currentReasons) => ({
        ...currentReasons,
        [flag.key]: "",
      }));
    } finally {
      setBusyAction("");
    }
  }

  async function handleReview(request: AdminFeatureFlagChangeRequest, approved: boolean) {
    const actionKey = `${approved ? "approve" : "reject"}:${request.id}`;
    setBusyAction(actionKey);

    try {
      await reviewMutation.mutateAsync({
        requestId: request.id,
        approved,
        note: reviewNotes[request.id],
      });
    } finally {
      setBusyAction("");
    }
  }

  return (
    <section style={{ display: "grid", gap: "1rem" }}>
      <AdminPageHeading
        title={adminMessage(locale, "admin.featureFlags.sectionTitle")}
        subtitle={adminMessage(locale, "admin.featureFlags.sectionSubtitle")}
      />
      {error ? <ErrorState message={formatAdminError(locale, error)} /> : null}
      {loading ? (
        <LoadingState copy={adminText(locale, "common.loading")} />
      ) : flags.length === 0 ? (
        <EmptyState copy={adminText(locale, "featureFlags.empty")} />
      ) : (
        <>
          <div style={{ display: "grid", gap: "0.8rem", gridTemplateColumns: "repeat(auto-fit, minmax(12rem, 1fr))" }}>
            <AdminMetricCard label={adminText(locale, "common.totalFlags")} value={flags.length.toString()} />
            <AdminMetricCard label={adminText(locale, "featureFlags.pendingRequests")} tone="warning" value={flags.filter((flag) => flag.has_pending_request).length.toString()} />
            <AdminMetricCard label={adminText(locale, "featureFlags.configBlockers")} tone="danger" value={flags.filter((flag) => flag.requires_config && !flag.config_ready).length.toString()} />
            <AdminMetricCard label={adminText(locale, "featureFlags.drift")} tone="success" value={flags.filter((flag) => flag.enabled !== flag.effective_enabled).length.toString()} />
          </div>

          <section className="app-inline-surface" style={{ display: "grid", gap: "0.8rem" }}>
            <div style={{ display: "grid", gap: "0.8rem", gridTemplateColumns: "minmax(0, 1fr) minmax(12rem, 14rem)" }}>
              <div>
                <label className="app-form-label" htmlFor="admin-feature-flags-search">
                  {adminMessage(locale, "admin.searchLabel")}
                </label>
                <input
                  className="app-input"
                  id="admin-feature-flags-search"
                  onChange={(event) => setFlagQuery(event.target.value)}
                  placeholder={adminText(locale, "featureFlags.filterPlaceholder")}
                  type="text"
                  value={flagQuery}
                />
              </div>
              <div>
                <label className="app-form-label" htmlFor="admin-feature-flags-status">
                  {adminMessage(locale, "admin.filter.statusLabel")}
                </label>
                <select className="app-input" id="admin-feature-flags-status" onChange={(event) => setRequestStatus(event.target.value)} value={requestStatus}>
                  <option value="all">{adminText(locale, "common.allStatuses")}</option>
                  <option value="pending">{featureFlagStatusLabel(locale, "pending")}</option>
                  <option value="approved">{featureFlagStatusLabel(locale, "approved")}</option>
                  <option value="rejected">{featureFlagStatusLabel(locale, "rejected")}</option>
                  <option value="executed">{featureFlagStatusLabel(locale, "executed")}</option>
                </select>
              </div>
            </div>
          </section>

          {filteredFlags.length === 0 ? (
            <EmptyState copy={adminText(locale, "featureFlags.matchingEmpty")} />
          ) : (
            <div style={{ display: "grid", gap: "0.8rem" }}>
              {filteredFlags.map((flag) => (
                <section className="app-inline-surface" key={flag.key} style={{ display: "grid", gap: "0.7rem" }}>
                  <div style={{ display: "flex", justifyContent: "space-between", gap: "1rem", alignItems: "start" }}>
                    <div style={{ display: "grid", gap: "0.45rem" }}>
                      <strong>{flag.key}</strong>
                      <span style={{ color: "hsl(var(--muted-foreground))" }}>{flag.description}</span>
                      <div style={{ display: "flex", gap: "0.45rem", flexWrap: "wrap", fontSize: "0.82rem" }}>
                        <span className="app-inline-surface">{featureFlagCategoryLabel(locale, flag.category)}</span>
                        <span className="app-inline-surface">{adminText(locale, "featureFlags.source")}{featureFlagSourceLabel(locale, flag.source)}</span>
                        <span className="app-inline-surface">{adminText(locale, "featureFlags.desired")}{flag.enabled ? adminText(locale, "common.on") : adminText(locale, "common.off")}</span>
                        <span className="app-inline-surface">{adminText(locale, "featureFlags.effective")}{flag.effective_enabled ? adminText(locale, "common.on") : adminText(locale, "common.off")}</span>
                        <span className="app-inline-surface">{adminText(locale, "common.config")}: {flag.config_ready ? adminText(locale, "common.ready") : adminText(locale, "common.missing")}</span>
                        {flag.has_pending_request ? <span className="app-inline-surface">{adminText(locale, "common.pendingRequest")}</span> : null}
                      </div>
                    </div>
                    <span style={{ fontSize: "0.78rem", color: "hsl(var(--muted-foreground))" }}>
                      {flag.updated_at ? `${adminText(locale, "common.updated")} ${formatTimestamp(flag.updated_at, locale)}` : adminText(locale, "featureFlags.seeded")}
                    </span>
                  </div>
                  <div style={{ display: "flex", gap: "0.6rem", flexWrap: "wrap" }}>
                    <input
                      className="app-input"
                      onChange={(event) =>
                        setRequestReasons((currentReasons) => ({
                          ...currentReasons,
                          [flag.key]: event.target.value,
                        }))
                      }
                      placeholder={adminText(locale, "featureFlags.reasonPlaceholder")}
                      style={{ flex: 1 }}
                      type="text"
                      value={requestReasons[flag.key] ?? ""}
                    />
                    <button
                      className="app-button-secondary"
                      disabled={!requestReasons[flag.key]?.trim() || flag.has_pending_request || busyAction === `request:${flag.key}`}
                      type="button"
                      onClick={() => void handleRequest(flag)}
                    >
                      {busyAction === `request:${flag.key}`
                        ? adminText(locale, "common.submitting")
                        : flag.enabled
                          ? adminText(locale, "featureFlags.requestDisable")
                          : adminText(locale, "featureFlags.requestEnable")}
                    </button>
                  </div>
                </section>
              ))}
            </div>
          )}

          <section style={{ display: "grid", gap: "0.8rem" }}>
            <h2 style={{ margin: 0 }}>{adminText(locale, "featureFlags.changeRequestsTitle")}</h2>
            {requests.length === 0 ? (
              <EmptyState copy={requestStatus === "all" ? adminText(locale, "featureFlags.noRequests") : adminText(locale, "featureFlags.noRequestsForFilter")} />
            ) : (
              <div style={{ display: "grid", gap: "0.8rem" }}>
                {requests.map((request) => (
                  <section className="app-inline-surface" key={request.id} style={{ display: "grid", gap: "0.7rem" }}>
                    <div style={{ display: "flex", justifyContent: "space-between", gap: "1rem", alignItems: "start" }}>
                      <div style={{ display: "grid", gap: "0.45rem" }}>
                        <div style={{ display: "flex", gap: "0.5rem", flexWrap: "wrap", alignItems: "center" }}>
                          <strong>{request.flag_key}</strong>
                          <span className="app-inline-surface">{featureFlagStatusLabel(locale, request.status)}</span>
                        </div>
                        <span style={{ color: "hsl(var(--muted-foreground))" }}>{request.reason}</span>
                        <div style={{ display: "flex", gap: "0.5rem", flexWrap: "wrap", fontSize: "0.82rem", color: "hsl(var(--muted-foreground))" }}>
                          <span>{adminText(locale, "featureFlags.requestedBy")}{request.requested_by}</span>
                          <span>{adminText(locale, "common.created")}: {formatTimestamp(request.created_at, locale)}</span>
                          {request.reviewed_by ? <span>{adminText(locale, "common.reviewedBy")}{request.reviewed_by}</span> : null}
                        </div>
                      </div>
                      <span style={{ fontSize: "0.78rem", color: "hsl(var(--muted-foreground))" }}>#{request.id}</span>
                    </div>
                    <div style={{ display: "flex", gap: "0.5rem", flexWrap: "wrap", fontSize: "0.82rem" }}>
                      <span className="app-inline-surface">{adminText(locale, "common.current")}: {request.current_enabled ? adminText(locale, "common.on") : adminText(locale, "common.off")}</span>
                      <span className="app-inline-surface">{adminText(locale, "featureFlags.requested")}{request.requested_enabled ? adminText(locale, "common.on") : adminText(locale, "common.off")}</span>
                    </div>
                    {request.review_note ? <div className="app-inline-surface">{adminText(locale, "featureFlags.reviewNote")}{request.review_note}</div> : null}
                    {request.status === "pending" ? (
                      <div style={{ display: "flex", gap: "0.6rem", flexWrap: "wrap" }}>
                        <input
                          className="app-input"
                          onChange={(event) =>
                            setReviewNotes((currentNotes) => ({
                              ...currentNotes,
                              [request.id]: event.target.value,
                            }))
                          }
                          placeholder={adminText(locale, "featureFlags.optionalReviewNote")}
                          style={{ flex: 1 }}
                          type="text"
                          value={reviewNotes[request.id] ?? ""}
                        />
                        <button
                          className="app-button-secondary"
                          disabled={busyAction === `approve:${request.id}`}
                          type="button"
                          onClick={() => void handleReview(request, true)}
                        >
                          {busyAction === `approve:${request.id}` ? adminText(locale, "common.processing") : adminText(locale, "featureFlags.approveExecute")}
                        </button>
                        <button
                          className="app-button-ghost"
                          disabled={busyAction === `reject:${request.id}`}
                          type="button"
                          onClick={() => void handleReview(request, false)}
                        >
                          {busyAction === `reject:${request.id}` ? adminText(locale, "common.processing") : adminText(locale, "featureFlags.reject")}
                        </button>
                      </div>
                    ) : null}
                  </section>
                ))}
              </div>
            )}
          </section>
        </>
      )}
    </section>
  );
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
        title={adminMessage(locale, "admin.auditLogs.sectionTitle")}
        subtitle={adminMessage(locale, "admin.auditLogs.sectionSubtitle")}
      />
      {error ? <ErrorState message={formatAdminError(locale, error)} /> : null}
      <section className="app-inline-surface" style={{ display: "grid", gap: "0.8rem" }}>
        <div style={{ display: "grid", gap: "0.8rem", gridTemplateColumns: "1.2fr repeat(4, minmax(10rem, 1fr)) 8rem" }}>
          <div>
            <label className="app-form-label" htmlFor="admin-audit-query">
              {adminMessage(locale, "admin.searchLabel")}
            </label>
            <input className="app-input" id="admin-audit-query" onChange={(event) => { setQuery(event.target.value); setPage(1); }} placeholder={adminMessage(locale, "admin.searchPlaceholder")} type="text" value={query} />
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
              {adminMessage(locale, "admin.filter.windowLabel")}
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
              {adminMessage(locale, "admin.filter.pageSizeLabel")}
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
