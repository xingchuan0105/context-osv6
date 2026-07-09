"use client";

import { useAuth } from "../../lib/auth/context";
import { useUiPreferences } from "../../lib/ui-preferences";
import {
  adminMessage,
  formatAdminError,
  healthStatusLabel,
} from "./admin-i18n";
import { useAdminHealthQuery } from "./admin-queries";
import {
  AdminMetricCard,
  AdminPageHeading,
  EmptyState,
  ErrorState,
  LoadingState,
} from "./admin-shared-ui";

export function AdminHealthSurface() {
  const { token, user } = useAuth();
  const actorId = user?.id;
  const { locale } = useUiPreferences();
  const healthQuery = useAdminHealthQuery(actorId, token);
  const health = healthQuery.data ?? null;
  const healthy = health ? ["ok", "healthy", "ready"].includes(health.status) : false;
  const loading = Boolean(token) && healthQuery.isPending;

  return (
    <section style={{ display: "grid", gap: "1rem" }}>
      <AdminPageHeading
        title={adminMessage(locale, "admin.health.sectionTitle")}
        subtitle={adminMessage(locale, "admin.health.sectionSubtitle")}
      />
      {healthQuery.error ? <ErrorState message={formatAdminError(locale, healthQuery.error)} /> : null}
      {loading ? (
        <LoadingState copy={adminMessage(locale, "common.loading")} />
      ) : !health ? (
        <EmptyState copy={adminMessage(locale, "common.emptyData")} />
      ) : (
        <>
          <div style={{ display: "grid", gap: "0.8rem", gridTemplateColumns: "repeat(auto-fit, minmax(12rem, 1fr))" }}>
            <AdminMetricCard label={adminMessage(locale, "common.status")} tone={healthy ? "success" : "danger"} value={healthStatusLabel(locale, health.status)} />
            <AdminMetricCard label={adminMessage(locale, "common.service")} value={health.service} />
            <AdminMetricCard label={adminMessage(locale, "common.version")} tone="warning" value={health.version} />
          </div>
          <section className="app-inline-surface" style={{ display: "grid", gap: "0.7rem" }}>
            <div className="app-inline-row" style={{ marginBottom: 0 }}>
              <span>{adminMessage(locale, "common.serviceStatus")}</span>
              <strong>{healthStatusLabel(locale, health.status)}</strong>
            </div>
            <div className="app-inline-row" style={{ marginBottom: 0 }}>
              <span>{adminMessage(locale, "common.service")}</span>
              <strong>{health.service}</strong>
            </div>
            <div className="app-inline-row" style={{ marginBottom: 0 }}>
              <span>{adminMessage(locale, "common.version")}</span>
              <strong>{health.version}</strong>
            </div>
          </section>
        </>
      )}
    </section>
  );
}
