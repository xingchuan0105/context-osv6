import React from "react";
import { render } from "@testing-library/react";

import { WorkspaceRightRail } from "../../../components/workspace/workspace-right-rail";
import { QueryProvider } from "../../../lib/query/provider";
import { getWorkspaceUiState } from "../../../lib/workspace/ui-store";

export function buildSource(overrides: Partial<{ id: string; file_name: string; status: string; title: string }> = {}) {
  return {
    id: overrides.id ?? "src-1",
    workspace_id: "ws-1",
    workspace_name: "Workspace 1",
    title: overrides.title ?? "Source",
    file_name: overrides.file_name ?? "source.pdf",
    status: overrides.status ?? "processing",
  };
}

export function buildNote(
  overrides: Partial<{
    id: string;
    title: string;
    content: string;
    preview: string;
    updated_at: string;
    promoted_document_id: string | null;
    promoted_at: string | null;
  }> = {},
) {
  return {
    id: overrides.id ?? "note-1",
    workspace_id: "ws-1",
    title: overrides.title ?? "Note",
    content: overrides.content ?? "Body",
    preview: overrides.preview ?? "Body",
    created_at: "2026-04-17T00:00:00Z",
    updated_at: overrides.updated_at ?? "2026-04-18T00:00:00Z",
    promoted_document_id: overrides.promoted_document_id ?? null,
    promoted_at: overrides.promoted_at ?? null,
  };
}

export function RightRailHarness({
  activeWebSources = null,
  focusedSourceId,
  onCloseWebSources,
  selectedSourceIds: initialSelectedSourceIds = [],
}: {
  activeWebSources?: Parameters<typeof WorkspaceRightRail>[0]["activeWebSources"];
  focusedSourceId?: string | null;
  onCloseWebSources?: () => void;
  selectedSourceIds?: string[];
}) {
  const [selectedSourceIds, setSelectedSourceIds] = React.useState(initialSelectedSourceIds);

  return (
    <WorkspaceRightRail
      activeWebSources={activeWebSources}
      focusedSourceId={focusedSourceId}
      onCloseWebSources={onCloseWebSources}
      onSelectedSourceIdsChange={setSelectedSourceIds}
      selectedSourceIds={selectedSourceIds}
      workspaceId="ws-1"
    />
  );
}

export function renderRightRailHarness(props: Parameters<typeof RightRailHarness>[0] = {}) {
  return render(
    <QueryProvider>
      <RightRailHarness {...props} />
    </QueryProvider>,
  );
}

export { getWorkspaceUiState };
