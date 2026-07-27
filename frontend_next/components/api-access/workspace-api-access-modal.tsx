"use client";

import { AppModal } from "../ui/app-modal";
import { formatUiMessage } from "../../lib/i18n/messages";
import { useUiPreferences } from "../../lib/ui-preferences";
import { WorkspaceApiAccessSurface } from "./workspace-api-access-surface";

type WorkspaceApiAccessModalProps = {
  open: boolean;
  workspaceId: string;
  onClose: () => void;
};

export function WorkspaceApiAccessModal({
  open,
  workspaceId,
  onClose,
}: WorkspaceApiAccessModalProps) {
  const { locale } = useUiPreferences();

  if (!open) {
    return null;
  }

  return (
    <AppModal
      open
      size="lg"
      title={formatUiMessage(locale, "apiAccessModal.title")}
      closeLabel={formatUiMessage(locale, "appModal.close")}
      fullPageHref={`/dashboard/${workspaceId}/api-access`}
      fullPageLabel={formatUiMessage(locale, "apiAccessModal.openFullPage")}
      testId="workspace-api-access-modal"
      onClose={onClose}
    >
      <WorkspaceApiAccessSurface embedded workspaceId={workspaceId} />
    </AppModal>
  );
}
