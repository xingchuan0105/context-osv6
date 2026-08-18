"use client";

import Link from "next/link";
import { useEffect, useState } from "react";

import { useAppWorkspaceId } from "../../hooks/use-app-workspace-id";

import { formatUiMessage } from "../../lib/i18n/messages";
import { useUiPreferences } from "../../lib/ui-preferences";
import { AppTopBar } from "../app-top-bar";
import { NavRail } from "../ui/nav-rail";
import { WorkspaceApiAccessSurface } from "../api-access/workspace-api-access-surface";
import { ShareActivityPanel } from "./parts/share-activity-panel";
import { ShareControlBar } from "./parts/share-control-bar";
import { ShareConversionBanner } from "./parts/share-conversion-banner";
import { ShareInsightsPanel } from "./parts/share-insights-panel";
import { ShareInvitePanel } from "./parts/share-invite-panel";
import { ShareOwnerProfileCard } from "./parts/share-owner-profile-card";
import { SectionHeader } from "./parts/share-center-ui";
import {
  IconApi,
  IconInvite,
  IconOwnerProfile,
  IconShareControls,
  IconTraffic,
} from "./parts/share-nav-icons";
import { useShareCenter } from "./parts/use-share-center";
import styles from "./workspace-share-surface.module.css";

type WorkspaceShareCenterSurfaceProps = {
  workspaceId: string;
};

type ShareCenterSection = "controls" | "invite" | "api" | "traffic" | "profile";

const SHARE_CENTER_SECTIONS: ShareCenterSection[] = [
  "controls",
  "invite",
  "api",
  "traffic",
  "profile",
];

function initialSectionFromHash(): ShareCenterSection {
  if (typeof window === "undefined") {
    return "controls";
  }
  const hash = window.location.hash.replace("#", "");
  return SHARE_CENTER_SECTIONS.includes(hash as ShareCenterSection)
    ? (hash as ShareCenterSection)
    : "controls";
}

export function WorkspaceShareCenterSurface({
  workspaceId: workspaceIdProp,
}: WorkspaceShareCenterSurfaceProps) {
  const workspaceId = useAppWorkspaceId(workspaceIdProp);
  const { locale } = useUiPreferences();
  const center = useShareCenter(workspaceId);
  const { actionError, actionMessage, settingsQuery, quotaSummary } = center;
  const [section, setSection] = useState<ShareCenterSection>("controls");
  const quotaForced =
    typeof actionError === "string" &&
    (actionError.includes("share_workspace_quota") ||
      actionError.includes("可分享") ||
      actionError.toLowerCase().includes("shareable") ||
      actionError.toLowerCase().includes("quota"));

  // 深链：/share#api 等直接选中对应分享方法区块（含页内 hash 变化）。
  useEffect(() => {
    function syncFromHash() {
      setSection(initialSectionFromHash());
    }
    syncFromHash();
    window.addEventListener("hashchange", syncFromHash);
    return () => window.removeEventListener("hashchange", syncFromHash);
  }, []);

  return (
    <>
      <AppTopBar locale={locale} />
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
            {center.quotaLabel ? (
              <p className={styles.pageSubtitle} data-testid="share-owner-cost-hint">
                {center.quotaLabel}
              </p>
            ) : null}
          </div>
        </header>

        <ShareConversionBanner
          locale={locale}
          quota={quotaSummary}
          forced={quotaForced}
        />

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

        {/* Grok 式：左导航（分享方法/追踪/主页）+ 右内容 */}
        <div className={styles.railGrid}>
          <NavRail
            activeId={section}
            ariaLabel={formatUiMessage(locale, "shareCenter.navAriaLabel")}
            items={[
              {
                id: "controls",
                label: formatUiMessage(locale, "shareCenter.controlBarTitle"),
                icon: <IconShareControls />,
              },
              {
                id: "invite",
                label: formatUiMessage(locale, "shareCenter.inviteSectionTitle"),
                icon: <IconInvite />,
              },
              {
                id: "api",
                label: formatUiMessage(locale, "apiAccess.title"),
                icon: <IconApi />,
              },
              {
                id: "traffic",
                label: formatUiMessage(locale, "shareCenter.navTraffic"),
                icon: <IconTraffic />,
              },
              {
                id: "profile",
                label: formatUiMessage(locale, "shareCenter.ownerProfileTitle"),
                icon: <IconOwnerProfile />,
              },
            ]}
            testId="share-center-nav-rail"
            onSelect={(id) => setSection(id as ShareCenterSection)}
          />
          <div className={styles.railContent}>
            {section === "controls" ? (
              <section className={`app-surface-card ${styles.controlCard}`}>
                <ShareControlBar center={center} />
              </section>
            ) : null}
            {section === "invite" ? <ShareInvitePanel center={center} /> : null}
            {section === "api" ? (
              <section id="api" data-testid="share-api-section">
                <SectionHeader
                  title={formatUiMessage(locale, "apiAccess.title")}
                  subtitle={formatUiMessage(locale, "shareCenter.apiMethodHint")}
                />
                <WorkspaceApiAccessSurface workspaceId={workspaceId} />
              </section>
            ) : null}
            {section === "traffic" ? (
              <>
                <ShareInsightsPanel center={center} />
                <ShareActivityPanel center={center} />
              </>
            ) : null}
            {section === "profile" ? <ShareOwnerProfileCard /> : null}
          </div>
        </div>
      </div>
      </main>
    </>
  );
}
