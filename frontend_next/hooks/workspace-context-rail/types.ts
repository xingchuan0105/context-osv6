export type WorkspaceContextRailProps = {
  workspaceId: string;
  selectedSourceIds: string[];
  onSelectedSourceIdsChange(ids: string[]): void;
  focusedSourceId?: string | null;
};
