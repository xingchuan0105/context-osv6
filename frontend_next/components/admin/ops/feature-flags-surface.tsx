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
        title={adminText(locale, "admin.featureFlags.sectionTitle")}
        subtitle={adminText(locale, "admin.featureFlags.sectionSubtitle")}
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
                  {adminText(locale, "admin.searchLabel")}
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
                  {adminText(locale, "admin.filter.statusLabel")}
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

