"use client";

import type { WorkspaceWebSourcesRequest } from "../../lib/workspace/model";
import { WebSourcesTakeover } from "./parts/workspace-right-rail-web-sources";
import { WorkspaceContextRail } from "./parts/workspace-context-rail";

type WorkspaceRightRailProps = {
  workspaceId: string;
  selectedSourceIds: string[];
  onSelectedSourceIdsChange(ids: string[]): void;
  focusedSourceId?: string | null;
  activeWebSources?: WorkspaceWebSourcesRequest | null;
  onCloseWebSources?: () => void;
};

export function WorkspaceRightRail({
  workspaceId,
  selectedSourceIds,
  onSelectedSourceIdsChange,
  focusedSourceId = null,
  activeWebSources = null,
  onCloseWebSources,
}: WorkspaceRightRailProps) {
  if (activeWebSources && activeWebSources.sources.length > 0) {
    return (
      <WebSourcesTakeover
        activeWebSources={activeWebSources}
        onCloseWebSources={onCloseWebSources}
      />
    );
  }

  return (
    <WorkspaceContextRail
      focusedSourceId={focusedSourceId}
      onSelectedSourceIdsChange={onSelectedSourceIdsChange}
      selectedSourceIds={selectedSourceIds}
      workspaceId={workspaceId}
    />
  );
}
