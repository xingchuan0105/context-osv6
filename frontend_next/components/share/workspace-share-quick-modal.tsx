"use client";

import { AppModal } from "../ui/app-modal";
import { formatUiMessage } from "../../lib/i18n/messages";
import { useUiPreferences } from "../../lib/ui-preferences";
import { ShareControlBar } from "./parts/share-control-bar";
import { ShareInvitePanel } from "./parts/share-invite-panel";
import { useShareCenter } from "./parts/use-share-center";

type WorkspaceShareQuickModalProps = {
  open: boolean;
  workspaceId: string;
  onClose: () => void;
};

function WorkspaceShareQuickModalBody({
  workspaceId,
  onClose,
}: {
  workspaceId: string;
  onClose: () => void;
}) {
  const { locale } = useUiPreferences();
  const center = useShareCenter(workspaceId);
  const { actionError, actionMessage, settingsQuery } = center;

  return (
    <AppModal
      open
      size="lg"
      title={formatUiMessage(locale, "shareQuickModal.title")}
      closeLabel={formatUiMessage(locale, "appModal.close")}
      fullPageHref={`/dashboard/${workspaceId}/share`}
      fullPageLabel={formatUiMessage(locale, "shareQuickModal.openFullPage")}
      testId="workspace-share-quick-modal"
      onClose={onClose}
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
      <div style={{ display: "grid", gap: "1rem" }}>
        <section className="app-surface-card" style={{ padding: "1rem" }}>
          <ShareControlBar center={center} />
        </section>
        <ShareInvitePanel center={center} />
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
