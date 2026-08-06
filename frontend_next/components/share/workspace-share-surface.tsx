"use client";

import Link from "next/link";

import { formatUiMessage } from "../../lib/i18n/messages";
import { useUiPreferences } from "../../lib/ui-preferences";
import { ShareActivityPanel } from "./parts/share-activity-panel";
import { ShareControlBar } from "./parts/share-control-bar";
import { ShareInsightsPanel } from "./parts/share-insights-panel";
import { ShareInvitePanel } from "./parts/share-invite-panel";
import { useShareCenter } from "./parts/use-share-center";
import styles from "./workspace-share-surface.module.css";

type WorkspaceShareCenterSurfaceProps = {
  workspaceId: string;
};

export function WorkspaceShareCenterSurface({
  workspaceId,
}: WorkspaceShareCenterSurfaceProps) {
  const { locale } = useUiPreferences();
  const center = useShareCenter(workspaceId);
  const { actionError, actionMessage, settingsQuery } = center;

  return (
    <main className="app-page-shell">
      <div
        className={`app-page-center ${styles.pageCenter}`}
      >
        <header className={styles.header}>
          <Link className="app-link app-link-muted" href={`/dashboard/${workspaceId}`}>
            {formatUiMessage(locale, "shareCenter.backToWorkspace")}
          </Link>
          <div className={styles.headerGrid}>
            <div>
              <h1 className={`app-page-title ${styles.pageTitle}`}>
                {formatUiMessage(locale, "shareCenter.pageTitle")}
              </h1>
              <p
                className={`app-page-subtitle ${styles.pageSubtitle}`}
              >
                {formatUiMessage(locale, "shareCenter.pageSubtitle")}
              </p>
            </div>
            <p className={styles.pageSubtitle} data-testid="share-owner-cost-hint">
              {formatUiMessage(locale, "shareCenter.pageSubtitle")}
              {center.quotaLabel ? ` · ${center.quotaLabel}` : ""}
            </p>
          </div>
        </header>

        {actionError ? (
          <p className="app-notice-banner">{actionError}</p>
        ) : null}

        {actionMessage ? (
          <p className={`app-inline-surface ${styles.actionMessage}`}>
            {actionMessage}
          </p>
        ) : null}

        {settingsQuery.isLoading && !settingsQuery.data ? (
          <section className="app-surface-card">
            <div aria-hidden="true" className={styles.skeletonStack}>
              <div className={`dashboard-skeleton-line ${styles.skeletonShort}`} />
              <div className={`dashboard-skeleton-line ${styles.skeletonLong}`} />
              <div className={`dashboard-skeleton-line ${styles.skeletonMid}`} />
            </div>
          </section>
        ) : null}

        {settingsQuery.error && !settingsQuery.data ? (
          <section className="app-surface-card">
            <p className="app-notice-banner">
              {settingsQuery.error instanceof Error
                ? settingsQuery.error.message
                : formatUiMessage(locale, "shareCenter.settingsLoadError")}
            </p>
          </section>
        ) : null}

        {/* k-structure: invite → people → access/link (control bar) → insights */}
        <ShareInvitePanel center={center} />
        <section className={`app-surface-card ${styles.controlCard}`}>
          <ShareControlBar center={center} />
        </section>
        <ShareInsightsPanel center={center} />
        <ShareActivityPanel center={center} />
      </div>
    </main>
  );
}
