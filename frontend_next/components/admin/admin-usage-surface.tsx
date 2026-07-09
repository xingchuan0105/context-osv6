"use client";

import { useState } from "react";

import { useAuth } from "../../lib/auth/context";
import { useUiPreferences } from "../../lib/ui-preferences";
import {
  adminText,
  formatAdminError,
} from "./admin-i18n";
import {
  ADMIN_ALL_ORGS_VALUE,
  getCombinedAdminQueryError,
  useAdminOrganizationsQuery as useOrganizationsQuery,
  useAdminUsageScopeQuery,
} from "./admin-queries";
import {
  AdminMetricCard,
  AdminPageHeading,
  EmptyState,
  ErrorState,
  LoadingState,
} from "./admin-shared-ui";
import {
  formatCompactNumber,
  formatCountLabel,
  USAGE_PERIOD_OPTIONS,
} from "./admin-utils";

export function AdminUsageSurface() {
  const { token, user } = useAuth();
  const actorId = user?.id;
  const { locale } = useUiPreferences();
  const organizationsQuery = useOrganizationsQuery(actorId, token);
  const [selectedOrgId, setSelectedOrgId] = useState<string | null>(null);
  const [selectedPeriod, setSelectedPeriod] = useState<(typeof USAGE_PERIOD_OPTIONS)[number]>("30d");
  const organizations = organizationsQuery.data ?? [];
  const effectiveSelectedOrgId = selectedOrgId ?? ADMIN_ALL_ORGS_VALUE;
  const usageScopeQuery = useAdminUsageScopeQuery(
    actorId,
    token,
    organizations,
    effectiveSelectedOrgId,
    selectedPeriod,
  );
  const usage = usageScopeQuery.data?.usage ?? null;
  const error = organizationsQuery.error ?? usageScopeQuery.error ?? null;
  const warning = usageScopeQuery.data?.failedOrgNames.length
    ? `${adminText(locale, "admin.loadError")} ${usageScopeQuery.data.failedOrgNames.join(", ")}`
    : "";
  const selectedOrg = organizations.find((organization) => organization.id === effectiveSelectedOrgId) ?? null;
  const scopeLabel =
    effectiveSelectedOrgId === ADMIN_ALL_ORGS_VALUE
      ? adminText(locale, "usage.aggregateScope")
      : selectedOrg?.name ?? adminText(locale, "users.noOrganizationSelected");
  const usageLoading = Boolean(token) && (organizationsQuery.isPending || usageScopeQuery.isPending);

  return (
    <section style={{ display: "grid", gap: "1rem" }}>
      <AdminPageHeading
        title={adminText(locale, "admin.nav.usage")}
        subtitle={adminText(locale, "usage.subtitle")}
      />
      {error ? <ErrorState message={formatAdminError(locale, error)} /> : null}
      {warning ? <ErrorState message={warning} /> : null}

      <section className="app-inline-surface" style={{ display: "grid", gap: "0.8rem" }}>
        <div style={{ display: "grid", gap: "0.8rem", gridTemplateColumns: "minmax(16rem, 18rem) minmax(0, 1fr)" }}>
          <div>
            <label className="app-form-label" htmlFor="admin-usage-scope">
              {adminText(locale, "common.scope")}
            </label>
            <select
              className="app-input"
              disabled={organizationsQuery.isPending || organizations.length === 0}
              id="admin-usage-scope"
              onChange={(event) => setSelectedOrgId(event.target.value)}
              value={effectiveSelectedOrgId}
            >
              <option value={ADMIN_ALL_ORGS_VALUE}>{adminText(locale, "usage.aggregateScope")}</option>
              {organizations.map((organization) => (
                <option key={organization.id} value={organization.id}>
                  {organization.name}
                </option>
              ))}
            </select>
          </div>
          <div>
            <label className="app-form-label">{adminText(locale, "admin.filter.windowLabel")}</label>
            <div className="app-button-row">
              {USAGE_PERIOD_OPTIONS.map((period) => (
                <button
                  className={selectedPeriod === period ? "app-button-primary" : "app-button-secondary"}
                  key={period}
                  type="button"
                  onClick={() => setSelectedPeriod(period)}
                >
                  {period}
                </button>
              ))}
            </div>
          </div>
        </div>
        <div style={{ display: "flex", gap: "1rem", flexWrap: "wrap", fontSize: "0.82rem", color: "hsl(var(--muted-foreground))" }}>
          <span>{adminText(locale, "common.currentView")}{scopeLabel}</span>
          <span>{adminText(locale, "common.timeWindow")}{selectedPeriod}</span>
          {effectiveSelectedOrgId === ADMIN_ALL_ORGS_VALUE && organizations.length > 0 ? (
            <span>{formatCountLabel(locale, organizations.length, "organizationsInAggregate")}</span>
          ) : null}
        </div>
      </section>

      {usageLoading ? (
        <LoadingState copy={adminText(locale, "usage.loading")} />
      ) : !usage ? (
        <EmptyState copy={adminText(locale, "usage.noData")} />
      ) : (
        <>
          <div style={{ display: "grid", gap: "0.8rem", gridTemplateColumns: "repeat(auto-fit, minmax(12rem, 1fr))" }}>
            <AdminMetricCard label={adminText(locale, "admin.metrics.totalRequests")} tone="primary" value={formatCompactNumber(usage.total_requests)} />
            <AdminMetricCard label={adminText(locale, "common.totalTokens")} tone="success" value={formatCompactNumber(usage.total_tokens)} />
            <AdminMetricCard label={adminText(locale, "admin.metrics.totalDocuments")} tone="warning" value={formatCompactNumber(usage.total_documents)} />
          </div>
          <section className="app-inline-surface" style={{ display: "grid", gap: "0.7rem" }}>
            <h2 style={{ margin: 0 }}>{adminText(locale, "common.platformStatistics")}</h2>
            <div className="app-inline-row" style={{ marginBottom: 0 }}>
              <span>{adminText(locale, "admin.metrics.totalRequests")}</span>
              <strong>{usage.total_requests}</strong>
            </div>
            <div className="app-inline-row" style={{ marginBottom: 0 }}>
              <span>{adminText(locale, "common.totalTokensProcessed")}</span>
              <strong>{usage.total_tokens}</strong>
            </div>
            <div className="app-inline-row" style={{ marginBottom: 0 }}>
              <span>{adminText(locale, "common.totalIndexedDocuments")}</span>
              <strong>{usage.total_documents}</strong>
            </div>
          </section>
        </>
      )}
    </section>
  );
}
