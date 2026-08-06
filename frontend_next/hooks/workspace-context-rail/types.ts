export type WorkspaceContextRailProps = {
  workspaceId: string;
  selectedSourceIds: string[];
  onSelectedSourceIdsChange(ids: string[]): void;
  focusedSourceId?: string | null;
  /** One-shot: open source viewer modal for this id, then clear via onOpenSourceConsumed. */
  openSourceId?: string | null;
  onOpenSourceConsumed?: () => void;
};
