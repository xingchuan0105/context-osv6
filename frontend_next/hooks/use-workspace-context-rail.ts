"use client";

import { useRef } from "react";

import { useAuth } from "../lib/auth/context";
import { useUiPreferences } from "../lib/ui-preferences";
import { useWorkspaceNoteEditor } from "./workspace-context-rail/use-note-editor";
import { useWorkspaceSourceActions } from "./workspace-context-rail/use-source-actions";
import { useWorkspaceSourceSelection } from "./workspace-context-rail/use-source-selection";
import { useWorkspaceSourceViewerExpansion } from "./workspace-context-rail/use-source-viewer-expansion";
import { useWorkspaceTransientSourceFilter } from "./workspace-context-rail/use-transient-source-filter";
import type { WorkspaceContextRailProps } from "./workspace-context-rail/types";

export type { WorkspaceContextRailProps };

export function useWorkspaceContextRail({
  workspaceId,
  selectedSourceIds,
  onSelectedSourceIdsChange,
  focusedSourceId = null,
}: WorkspaceContextRailProps) {
  const auth = useAuth();
  const { locale } = useUiPreferences();
  const pendingUploadedModeSwitchRef = useRef(false);
  const setViewerSourceIdRef = useRef<React.Dispatch<React.SetStateAction<string | null>>>(() => {});

  const transientFilter = useWorkspaceTransientSourceFilter();

  const sourceActions = useWorkspaceSourceActions({
    token: auth.token,
    workspaceId,
    locale,
    selectedSourceIds,
    onSelectedSourceIdsChange,
    markSourcesTransientProcessing: transientFilter.markSourcesTransientProcessing,
    pendingUploadedModeSwitchRef,
    setViewerSourceIdRef,
  });

  const viewerExpansion = useWorkspaceSourceViewerExpansion({
    token: auth.token,
    workspaceId,
    locale,
    sources: sourceActions.sources,
    sourcesPending: sourceActions.sourcesLoading,
  });

  setViewerSourceIdRef.current = viewerExpansion.setViewerSourceId;

  const selection = useWorkspaceSourceSelection({
    workspaceId,
    sources: sourceActions.sources,
    selectedSourceIds,
    onSelectedSourceIdsChange,
    pendingUploadedModeSwitchRef,
  });

  const noteEditor = useWorkspaceNoteEditor({
    token: auth.token,
    workspaceId,
    locale,
  });

  const sourceViewerOpen = Boolean(viewerExpansion.viewerSourceId) && !noteEditor.noteEditorOpen;

  return {
    activeNoteId: noteEditor.activeNoteId,
    draftContent: noteEditor.draftContent,
    draftTitle: noteEditor.draftTitle,
    focusedSourceId,
    handleAddUrlSource: sourceActions.handleAddUrlSource,
    handleCreateNote: noteEditor.handleCreateNote,
    handleDeleteActiveNote: noteEditor.handleDeleteActiveNote,
    handleDeleteNote: noteEditor.handleDeleteNote,
    handleDeleteSource: sourceActions.handleDeleteSource,
    handleOpenSource: viewerExpansion.handleOpenSource,
    handlePromoteActiveNote: noteEditor.handlePromoteActiveNote,
    handlePromoteNote: noteEditor.handlePromoteNote,
    handleReindexSource: sourceActions.handleReindexSource,
    handleSaveActiveNote: noteEditor.handleSaveActiveNote,
    handleSelectAll: selection.handleSelectAll,
    handleSelectedSourceToggle: selection.handleSelectedSourceToggle,
    handleUploadFiles: sourceActions.handleUploadFiles,
    noteEditorOpen: noteEditor.noteEditorOpen,
    notes: noteEditor.notes,
    notesError: noteEditor.notesError,
    notesLoading: noteEditor.notesLoading,
    noteSyncState: noteEditor.noteSyncState,
    selectedSourceIds,
    setActiveNoteId: noteEditor.setActiveNoteId,
    setDraftContent: noteEditor.setDraftContent,
    setDraftTitle: noteEditor.setDraftTitle,
    setUrlSource: sourceActions.setUrlSource,
    setViewerSourceId: viewerExpansion.setViewerSourceId,
    sourceViewerOpen,
    sourceViewerQuery: viewerExpansion.sourceViewerQuery,
    sources: sourceActions.sources,
    sourcesError: sourceActions.sourcesError,
    sourcesLoading: sourceActions.sourcesLoading,
    sourcesPolling: sourceActions.sourcesPolling,
    transientProcessingSourceIds: transientFilter.transientProcessingSourceIds,
    sourceUploadPending: sourceActions.sourceUploadPending,
    urlSource: sourceActions.urlSource,
    viewerError: viewerExpansion.viewerError,
    viewerHasMore: viewerExpansion.viewerHasMore,
    viewerLoading: viewerExpansion.viewerLoading,
    viewerLoadingMore: viewerExpansion.viewerLoadingMore,
    viewerPreview: viewerExpansion.viewerPreview,
    viewerRawContent: viewerExpansion.viewerRawContent,
    viewerSource: viewerExpansion.viewerSource,
    viewerSourceId: viewerExpansion.viewerSourceId,
    viewerSummary: viewerExpansion.viewerSummary,
  };
}
