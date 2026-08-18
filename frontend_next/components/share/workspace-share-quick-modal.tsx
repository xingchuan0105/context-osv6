"use client";

import { useState } from "react";

import { AppModal } from "../ui/app-modal";
import { NavRail } from "../ui/nav-rail";
import { WorkspaceApiAccessSurface } from "../api-access/workspace-api-access-surface";
import { formatUiMessage } from "../../lib/i18n/messages";
import { useUiPreferences } from "../../lib/ui-preferences";
import { SectionHeader } from "./parts/share-center-ui";
import { IconApi, IconInvite, IconShareControls } from "./parts/share-nav-icons";
import { ShareControlBar } from "./parts/share-control-bar";
import { ShareInvitePanel } from "./parts/share-invite-panel";
import { useShareCenter } from "./parts/use-share-center";
import { useDesktopPublishGate } from "./parts/use-desktop-publish-gate";

type WorkspaceShareQuickModalProps = {
  open: boolean;
  workspaceId: string;
  onClose: () => void;
};

type ShareQuickSection = "controls" | "invite" | "api";

function WorkspaceShareQuickModalBody({
  workspaceId,
  onClose,
}: {
  workspaceId: string;
  onClose: () => void;
}) {
  const { locale } = useUiPreferences();
  const publishGate = useDesktopPublishGate(workspaceId);
  const center = useShareCenter(publishGate.shareWorkspaceId, {
    queriesEnabled: publishGate.queriesEnabled,
  });
  const { actionError, actionMessage, settingsQuery } = center;
  const [section, setSection] = useState<ShareQuickSection>("controls");

  return (
    <AppModal
      open
      size="xl"
      bodyVariant="rail"
      title={formatUiMessage(locale, "shareQuickModal.title")}
      closeLabel={formatUiMessage(locale, "appModal.close")}
      fullPageHref={`/dashboard/${workspaceId}/share`}
      fullPageLabel={formatUiMessage(locale, "shareQuickModal.openFullPage")}
      testId="workspace-share-quick-modal"
      onClose={onClose}
    >
      {/* Grok 式设置弹窗：左导航（分享方法）+ 右设置内容 */}
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
        ]}
        testId="share-quick-nav-rail"
        onSelect={(id) => setSection(id as ShareQuickSection)}
      />
      <div
        style={{
          display: "grid",
          gap: "1rem",
          alignContent: "start",
          minWidth: 0,
          padding: "var(--space-lg)",
        }}
      >
        {actionError ? <p className="app-notice-banner">{actionError}</p> : null}
        {actionMessage ? <p className="app-inline-surface">{actionMessage}</p> : null}
        {settingsQuery.isLoading && !settingsQuery.data ? (
          <p className="app-page-subtitle">{formatUiMessage(locale, "shareQuickModal.loading")}</p>
        ) : null}
        {settingsQuery.error && !settingsQuery.data ? (
          <p className="app-notice-banner">
            {settingsQuery.error instanceof Error
              ? settingsQuery.error.message
              : formatUiMessage(locale, "shareCenter.settingsLoadError")}
          </p>
        ) : null}
        {section === "controls" ? (
          <section className="app-surface-card">
            <ShareControlBar center={center} publishGate={publishGate} />
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
      </div>
    </AppModal>
  );
}

export function WorkspaceShareQuickModal({
  open,
  workspaceId,
  onClose,
}: WorkspaceShareQuickModalProps) {
  if (!open) {
    return null;
  }
  return <WorkspaceShareQuickModalBody workspaceId={workspaceId} onClose={onClose} />;
}
