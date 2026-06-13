"use client";

import Link from "next/link";

import { formatUiMessage } from "../../lib/i18n/messages";
import { useUiPreferences } from "../../lib/ui-preferences";
import { ShareActivityPanel } from "./parts/share-activity-panel";
import { ShareControlBar } from "./parts/share-control-bar";
import { ShareInsightsPanel } from "./parts/share-insights-panel";
import { ShareInvitePanel } from "./parts/share-invite-panel";
import { useShareCenter } from "./parts/use-share-center";

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
        className="app-page-center"
        style={{ display: "grid", gap: "0.85rem", maxWidth: "54rem", width: "100%" }}
      >
        <header style={{ display: "grid", gap: "0.65rem" }}>
          <Link className="app-link app-link-muted" href={`/dashboard/${workspaceId}`}>
            {formatUiMessage(locale, "shareCenter.backToWorkspace")}
          </Link>
          <div
            style={{
              alignItems: "start",
              display: "grid",
              gap: "1rem",
              gridTemplateColumns: "minmax(0, 1fr)",
            }}
          >
            <div>
              <h1 className="app-page-title" style={{ fontSize: "clamp(2.15rem, 5vw, 2.8rem)" }}>
                {formatUiMessage(locale, "shareCenter.pageTitle")}
              </h1>
              <p
                className="app-page-subtitle"
                style={{ fontSize: "1rem", lineHeight: 1.55, marginTop: "0.25rem" }}
              >
                {formatUiMessage(locale, "shareCenter.pageSubtitle")}
              </p>
            </div>
            <section
              className="app-surface-card"
              style={{
                display: "grid",
                gap: "0.85rem",
                padding: "0.9rem 0.95rem 0.95rem",
              }}
            >
              <ShareControlBar center={center} />
            </section>
          </div>
        </header>

        {actionError ? (
          <p className="app-notice-banner">{actionError}</p>
        ) : null}

        {actionMessage ? (
          <p className="app-inline-surface" style={{ margin: 0 }}>
            {actionMessage}
          </p>
        ) : null}

        {settingsQuery.isLoading && !settingsQuery.data ? (
          <section className="app-surface-card">
            <p style={{ margin: 0 }}>
              {formatUiMessage(locale, "shareCenter.loading")}
            </p>
          </section>
        ) : null}

        {settingsQuery.error && !settingsQuery.data ? (
          <section className="app-surface-card">
            <p className="app-notice-banner" style={{ margin: 0 }}>
              {settingsQuery.error instanceof Error
                ? settingsQuery.error.message
                : formatUiMessage(locale, "shareCenter.settingsLoadError")}
            </p>
          </section>
        ) : null}

        <ShareInvitePanel center={center} />
        <ShareInsightsPanel center={center} />
        <ShareActivityPanel center={center} />
      </div>
    </main>
  );
}
