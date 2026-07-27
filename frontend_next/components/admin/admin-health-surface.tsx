"use client";

import { useAuth } from "../../lib/auth/context";
import { useUiPreferences } from "../../lib/ui-preferences";
import {
  adminText,
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

import styles from "./admin-health-surface.module.css";

export function AdminHealthSurface() {
  const { token, user } = useAuth();
  const actorId = user?.id;
  const { locale } = useUiPreferences();
  const healthQuery = useAdminHealthQuery(actorId, token);
  const health = healthQuery.data ?? null;
  const healthy = health ? ["ok", "healthy", "ready"].includes(health.status) : false;
  const loading = Boolean(token) && healthQuery.isPending;

  return (
    <section className={styles.container}>
      <AdminPageHeading
        title={adminText(locale, "admin.health.sectionTitle")}
        subtitle={adminText(locale, "admin.health.sectionSubtitle")}
      />
      {healthQuery.error ? <ErrorState message={formatAdminError(locale, healthQuery.error)} /> : null}
      {loading ? (
        <LoadingState copy={adminText(locale, "common.loading")} />
      ) : !health ? (
        <EmptyState copy={adminText(locale, "common.emptyData")} />
      ) : (
        <>
          <div className={styles.metricsGrid}>
            <AdminMetricCard label={adminText(locale, "common.status")} tone={healthy ? "success" : "danger"} value={healthStatusLabel(locale, health.status)} />
            <AdminMetricCard label={adminText(locale, "common.service")} value={health.service} />
            <AdminMetricCard label={adminText(locale, "common.version")} tone="warning" value={health.version} />
          </div>
          <section className={`app-inline-surface ${styles.panel}`}>
            <div className={`app-inline-row ${styles.inlineRowFlat}`}>
              <span>{adminText(locale, "common.serviceStatus")}</span>
              <strong>{healthStatusLabel(locale, health.status)}</strong>
            </div>
            <div className={`app-inline-row ${styles.inlineRowFlat}`}>
              <span>{adminText(locale, "common.service")}</span>
              <strong>{health.service}</strong>
            </div>
            <div className={`app-inline-row ${styles.inlineRowFlat}`}>
              <span>{adminText(locale, "common.version")}</span>
              <strong>{health.version}</strong>
            </div>
          </section>
        </>
      )}
    </section>
  );
}
