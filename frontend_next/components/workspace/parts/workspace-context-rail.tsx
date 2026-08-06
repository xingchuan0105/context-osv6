"use client";

import { formatUiMessage } from "../../../lib/i18n/messages";
import { useUiPreferences } from "../../../lib/ui-preferences";
import {
  type WorkspaceContextRailProps,
  useWorkspaceContextRail,
} from "../../../hooks/use-workspace-context-rail";
import { AppModal } from "../../ui/app-modal";
import styles from "../workspace-right-rail.module.css";
import { WorkspaceNotesPane } from "../workspace-notes-pane";
import { WorkspaceSourceViewer } from "../workspace-source-viewer";
import { WorkspaceSourcesPane } from "../workspace-sources-pane";

/**
 * Right rail: source list + notes list stay in place.
 * W5 #17: source viewer and note editor open as centered AppModal overlays.
 */
export function WorkspaceContextRail(props: WorkspaceContextRailProps) {
  const { locale } = useUiPreferences();
  const {
    activeNoteId,
    draftContent,
    draftTitle,
    focusedSourceId,
    handleAddUrlSource,
    handleCreateNote,
    handleDeleteActiveNote,
    handleDeleteNote,
    handleDeleteSource,
    handleOpenSource,
    handlePromoteActiveNote,
    handlePromoteNote,
    handleReindexSource,
    handleSaveActiveNote,
    handleSelectAll,
    handleSelectedSourceToggle,
    handleUploadFiles,
    noteEditorOpen,
    notes,
    notesError,
    notesLoading,
    noteSyncState,
    selectedSourceIds,
    setActiveNoteId,
    setDraftContent,
    setDraftTitle,
    setUrlSource,
    setViewerSourceId,
    sourceViewerOpen,
    sourceViewerQuery,
    sources,
    sourcesError,
    sourcesLoading,
    sourcesPolling,
    transientProcessingSourceIds,
    sourceUploadPending,
    urlSource,
    viewerError,
    viewerHasMore,
    viewerLoading,
    viewerLoadingMore,
    viewerPreview,
    viewerRawContent,
    viewerSource,
    viewerSourceId,
    viewerSummary,
  } = useWorkspaceContextRail(props);

  const noteModalTitle =
    draftTitle?.trim() || formatUiMessage(locale, "workspaceRightRail.untitledNote");

  return (
    <div className={styles.rail}>
      <div className={styles.contextRail}>
        <div className={`${styles.contextSection} ${styles.contextSectionTop}`}>
          <WorkspaceSourcesPane
            activeViewerSourceId={viewerSourceId}
            error={sourcesError}
            focusedSourceId={focusedSourceId}
            loading={sourcesLoading}
            onAddUrlSource={handleAddUrlSource}
            onDeleteSource={handleDeleteSource}
            onOpenSource={handleOpenSource}
            onReindexSource={handleReindexSource}
            onSelectAll={handleSelectAll}
            onSelectedSourceToggle={handleSelectedSourceToggle}
            onUploadFiles={handleUploadFiles}
            onUrlSourceChange={setUrlSource}
            polling={sourcesPolling}
            selectedSourceIds={selectedSourceIds}
            sources={sources}
            transientProcessingSourceIds={transientProcessingSourceIds}
            uploading={sourceUploadPending}
            urlSource={urlSource}
          />
        </div>

        <div className={`${styles.contextSection} ${styles.contextSectionBottom}`}>
          <WorkspaceNotesPane
            activeNoteId={null}
            draftContent={draftContent}
            draftTitle={draftTitle}
            error={notesError}
            loading={notesLoading}
            notes={notes}
            onClearActiveNote={() => setActiveNoteId(null)}
            onCreateNote={handleCreateNote}
            onDeleteActiveNote={handleDeleteActiveNote}
            onDeleteNote={handleDeleteNote}
            onDraftContentChange={setDraftContent}
            onDraftTitleChange={setDraftTitle}
            onPromoteActiveNote={handlePromoteActiveNote}
            onPromoteNote={handlePromoteNote}
            onSaveActiveNote={handleSaveActiveNote}
            onSelectNote={setActiveNoteId}
            syncState={noteSyncState}
          />
        </div>
      </div>

      <AppModal
        open={sourceViewerOpen}
        size="lg"
        title={
          viewerSource?.file_name ??
          formatUiMessage(locale, "workspaceRightRail.viewerSectionTitle")
        }
        closeLabel={formatUiMessage(locale, "workspaceRightRail.closeViewerAction")}
        testId="workspace-source-viewer-modal"
        onClose={() => setViewerSourceId(null)}
      >
        <WorkspaceSourceViewer
          activePreviewIndex={null}
          citation={null}
          error={viewerError}
          hasMore={viewerHasMore}
          hideChrome
          loading={viewerLoading}
          loadingMore={viewerLoadingMore}
          parsedPreview={viewerPreview}
          rawContent={viewerRawContent}
          source={viewerSource}
          summary={viewerSummary}
          onClose={() => setViewerSourceId(null)}
          onLoadMore={() => {
            if (sourceViewerQuery.hasNextPage && !sourceViewerQuery.isFetchingNextPage) {
              void sourceViewerQuery.fetchNextPage();
            }
          }}
        />
      </AppModal>

      <AppModal
        open={noteEditorOpen}
        size="lg"
        title={noteModalTitle}
        closeLabel={formatUiMessage(locale, "workspaceRightRail.closeViewerAction")}
        testId="workspace-note-editor-modal"
        onClose={() => setActiveNoteId(null)}
      >
        <WorkspaceNotesPane
          activeNoteId={activeNoteId}
          draftContent={draftContent}
          draftTitle={draftTitle}
          error={notesError}
          hideChrome
          loading={notesLoading}
          notes={notes}
          onClearActiveNote={() => setActiveNoteId(null)}
          onCreateNote={handleCreateNote}
          onDeleteActiveNote={handleDeleteActiveNote}
          onDeleteNote={handleDeleteNote}
          onDraftContentChange={setDraftContent}
          onDraftTitleChange={setDraftTitle}
          onPromoteActiveNote={handlePromoteActiveNote}
          onPromoteNote={handlePromoteNote}
          onSaveActiveNote={handleSaveActiveNote}
          onSelectNote={setActiveNoteId}
          syncState={noteSyncState}
        />
      </AppModal>
    </div>
  );
}
