"use client";

import { useState } from "react";

import type { AdminFeatureFlagChangeRequest, AdminFeatureFlagEntry } from "./shared";
import {
  AdminMetricCard,
  AdminPageHeading,
  EmptyState,
  ErrorState,
  LoadingState,
  adminText,
  featureFlagCategoryLabel,
  featureFlagSourceLabel,
  featureFlagStatusLabel,
  formatAdminError,
  formatTimestamp,
  useAdminFeatureFlagRequestsQuery,
  useAdminFeatureFlagsQuery,
  useAuth,
  useRequestAdminFeatureFlagChangeMutation,
  useReviewAdminFeatureFlagChangeMutation,
  useUiPreferences,
} from "./shared";

import styles from "./feature-flags-surface.module.css";

/** Change-request status badge classes (light tinted bg + matching text). */
const STATUS_BADGE_CLASS: Record<string, string> = {
  pending: styles.statusPending,
  approved: styles.statusApproved,
  executed: styles.statusExecuted,
  rejected: styles.statusRejected,
};

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
    <section className={styles.container}>
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
          <div className={styles.metricsGrid}>
            <AdminMetricCard label={adminText(locale, "common.totalFlags")} value={flags.length.toString()} />
            <AdminMetricCard label={adminText(locale, "featureFlags.pendingRequests")} tone="warning" value={flags.filter((flag) => flag.has_pending_request).length.toString()} />
            <AdminMetricCard label={adminText(locale, "featureFlags.configBlockers")} tone="danger" value={flags.filter((flag) => flag.requires_config && !flag.config_ready).length.toString()} />
            <AdminMetricCard label={adminText(locale, "featureFlags.drift")} tone="success" value={flags.filter((flag) => flag.enabled !== flag.effective_enabled).length.toString()} />
          </div>

          <section className={`app-inline-surface ${styles.panel}`}>
            <div className={styles.filtersGrid}>
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
            <div className={styles.listStack}>
              {filteredFlags.map((flag) => (
                <section className={`app-inline-surface ${styles.card}`} key={flag.key}>
                  <div className={styles.headRow}>
                    <div className={styles.titleStack}>
                      <strong>{flag.key}</strong>
                      <span className={styles.mutedText}>{flag.description}</span>
                      <div className={styles.chipsRow}>
                        <span className="app-inline-surface">{featureFlagCategoryLabel(locale, flag.category)}</span>
                        <span className="app-inline-surface">{adminText(locale, "featureFlags.source")}{featureFlagSourceLabel(locale, flag.source)}</span>
                        <span className="app-inline-surface">{adminText(locale, "featureFlags.desired")}{flag.enabled ? adminText(locale, "common.on") : adminText(locale, "common.off")}</span>
                        <span className="app-inline-surface">{adminText(locale, "featureFlags.effective")}{flag.effective_enabled ? adminText(locale, "common.on") : adminText(locale, "common.off")}</span>
                        <span className="app-inline-surface">{adminText(locale, "common.config")}: {flag.config_ready ? adminText(locale, "common.ready") : adminText(locale, "common.missing")}</span>
                        {flag.has_pending_request ? <span className="app-inline-surface">{adminText(locale, "common.pendingRequest")}</span> : null}
                      </div>
                    </div>
                    <span className={styles.smallMuted}>
                      {flag.updated_at ? `${adminText(locale, "common.updated")} ${formatTimestamp(flag.updated_at, locale)}` : adminText(locale, "featureFlags.seeded")}
                    </span>
                  </div>
                  <div className={styles.actionRow}>
                    <input
                      className={`app-input ${styles.flexInput}`}
                      onChange={(event) =>
                        setRequestReasons((currentReasons) => ({
                          ...currentReasons,
                          [flag.key]: event.target.value,
                        }))
                      }
                      placeholder={adminText(locale, "featureFlags.reasonPlaceholder")}
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

          <section className={styles.listStack}>
            <h2 className={styles.heading}>{adminText(locale, "featureFlags.changeRequestsTitle")}</h2>
            {requests.length === 0 ? (
              <EmptyState copy={requestStatus === "all" ? adminText(locale, "featureFlags.noRequests") : adminText(locale, "featureFlags.noRequestsForFilter")} />
            ) : (
              <div className={styles.listStack}>
                {requests.map((request) => (
                  <section className={`app-inline-surface ${styles.card}`} key={request.id}>
                    <div className={styles.headRow}>
                      <div className={styles.titleStack}>
                        <div className={styles.titleRow}>
                          <strong>{request.flag_key}</strong>
                          <span className={`app-inline-surface ${STATUS_BADGE_CLASS[request.status] ?? ""}`}>
                            {featureFlagStatusLabel(locale, request.status)}
                          </span>
                        </div>
                        <span className={styles.mutedText}>{request.reason}</span>
                        <div className={styles.chipsRowMuted}>
                          <span>{adminText(locale, "featureFlags.requestedBy")}{request.requested_by}</span>
                          <span>{adminText(locale, "common.created")}: {formatTimestamp(request.created_at, locale)}</span>
                          {request.reviewed_by ? <span>{adminText(locale, "common.reviewedBy")}{request.reviewed_by}</span> : null}
                        </div>
                      </div>
                      <span className={styles.smallMuted}>#{request.id}</span>
                    </div>
                    <div className={styles.chipsRowSpaced}>
                      <span className="app-inline-surface">{adminText(locale, "common.current")}: {request.current_enabled ? adminText(locale, "common.on") : adminText(locale, "common.off")}</span>
                      <span className="app-inline-surface">{adminText(locale, "featureFlags.requested")}{request.requested_enabled ? adminText(locale, "common.on") : adminText(locale, "common.off")}</span>
                    </div>
                    {request.review_note ? <div className="app-inline-surface">{adminText(locale, "featureFlags.reviewNote")}{request.review_note}</div> : null}
                    {request.status === "pending" ? (
                      <div className={styles.actionRow}>
                        <input
                          className={`app-input ${styles.flexInput}`}
                          onChange={(event) =>
                            setReviewNotes((currentNotes) => ({
                              ...currentNotes,
                              [request.id]: event.target.value,
                            }))
                          }
                          placeholder={adminText(locale, "featureFlags.optionalReviewNote")}
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
