"use client";

import { WorkspaceContextRail } from "./parts/workspace-context-rail";

type WorkspaceRightRailProps = {
  workspaceId: string;
  selectedSourceIds: string[];
  onSelectedSourceIdsChange(ids: string[]): void;
  focusedSourceId?: string | null;
  openSourceId?: string | null;
  onOpenSourceConsumed?: () => void;
};

export function WorkspaceRightRail({
  workspaceId,
  selectedSourceIds,
  onSelectedSourceIdsChange,
  focusedSourceId = null,
  openSourceId = null,
  onOpenSourceConsumed,
}: WorkspaceRightRailProps) {
  return (
    <WorkspaceContextRail
      focusedSourceId={focusedSourceId}
      openSourceId={openSourceId}
      onOpenSourceConsumed={onOpenSourceConsumed}
      onSelectedSourceIdsChange={onSelectedSourceIdsChange}
      selectedSourceIds={selectedSourceIds}
      workspaceId={workspaceId}
    />
  );
}
