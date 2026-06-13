"use client";

import styles from "../workspace-right-rail.module.css";
import { WorkspaceNotesPane } from "../workspace-notes-pane";
import { WorkspaceSourceViewer } from "../workspace-source-viewer";
import { WorkspaceSourcesPane } from "../workspace-sources-pane";
import {
  type WorkspaceContextRailProps,
  useWorkspaceContextRail,
} from "../../../hooks/use-workspace-context-rail";

export function WorkspaceContextRail(props: WorkspaceContextRailProps) {
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

  return (
    <div className={`${styles.rail}${noteEditorOpen ? ` ${styles.railTakeover}` : ""}`}>
      {noteEditorOpen ? (
        <div className={styles.takeoverSection}>
          <WorkspaceNotesPane
            activeNoteId={activeNoteId}
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
      ) : (
        <div className={styles.contextRail}>
          <div className={`${styles.contextSection} ${styles.contextSectionTop}`}>
            {sourceViewerOpen ? (
              <WorkspaceSourceViewer
                activePreviewIndex={null}
                citation={null}
                error={viewerError}
                hasMore={viewerHasMore}
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
            ) : (
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
            )}
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
      )}
    </div>
  );
}
