"use client";

import { useQueryClient } from "@tanstack/react-query";
import { useCallback, useEffect, useMemo, useRef, useState } from "react";

import { useAuth } from "../../lib/auth/context";
import { formatUiMessage } from "../../lib/i18n/messages";
import { queryKeys } from "../../lib/query/keys";
import { useUiPreferences } from "../../lib/ui-preferences";
import {
  addWorkspaceSourceUrl,
  completeWorkspaceDocumentUpload,
  createWorkspaceDocumentUpload,
  uploadWorkspaceDocumentFile,
} from "../../lib/workspace/client";
import {
  fetchWorkspaceSourceRawContent,
  useCreateWorkspaceNoteMutation,
  useDeleteWorkspaceNoteMutation,
  useDeleteWorkspaceSourceMutation,
  usePromoteWorkspaceNoteMutation,
  useReindexWorkspaceSourceMutation,
  useUpdateWorkspaceNoteMutation,
  useWorkspaceNotesQuery,
  useWorkspaceSourceViewerQuery,
  useWorkspaceSourcesQuery,
} from "../../lib/workspace/right-rail-queries";
import {
  isWorkspaceSourceDocscopeEligible,
  isWorkspaceSourceTerminal,
  WorkspaceNoteSyncState,
  type WorkspaceNote,
  type WorkspaceWebSourcesRequest,
} from "../../lib/workspace/model";
import { getWorkspaceUiState, workspaceUiStore } from "../../lib/workspace/ui-store";
import styles from "./workspace-right-rail.module.css";
import { WorkspaceNotesPane } from "./workspace-notes-pane";
import { WorkspaceSourceViewer } from "./workspace-source-viewer";
import { WorkspaceSourcesPane } from "./workspace-sources-pane";

const TRANSIENT_SOURCE_PROCESSING_MS = 1800;

type WorkspaceRightRailProps = {
  workspaceId: string;
  selectedSourceIds: string[];
  onSelectedSourceIdsChange(ids: string[]): void;
  focusedSourceId?: string | null;
  activeWebSources?: WorkspaceWebSourcesRequest | null;
  onCloseWebSources?: () => void;
};

type WorkspaceContextRailProps = Omit<
  WorkspaceRightRailProps,
  "activeWebSources" | "onCloseWebSources"
>;

function arraysEqual(left: string[], right: string[]) {
  if (left.length !== right.length) {
    return false;
  }

  return left.every((value, index) => value === right[index]);
}

function getNoteMutationErrorMessage(
  locale: "zh-CN" | "en",
  action: "save" | "promote",
  error: unknown,
) {
  if (action === "promote" && error instanceof Error) {
    const message = error.message.trim();

    if (/cannot promote an empty note/i.test(message)) {
      return formatUiMessage(locale, "workspaceRightRail.promoteNoteEmptyError");
    }
  }

  return formatUiMessage(
    locale,
    action === "save" ? "workspaceRightRail.saveNoteError" : "workspaceRightRail.promoteNoteError",
  );
}

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

function WebSourcesTakeover({
  activeWebSources,
  onCloseWebSources,
}: {
  activeWebSources: WorkspaceWebSourcesRequest;
  onCloseWebSources?: () => void;
}) {
  const { locale } = useUiPreferences();

  return (
    <div className={`${styles.rail} ${styles.railTakeover}`}>
      <div className={styles.takeoverSection}>
        <div className={styles.webSourcesHeader}>
          <span className={styles.webSourcesCount}>
            {locale === "zh-CN"
              ? `${activeWebSources.sources.length} 个来源`
              : `${activeWebSources.sources.length} source${activeWebSources.sources.length > 1 ? "s" : ""}`}
          </span>
          <button
            aria-label={locale === "zh-CN" ? "关闭来源" : "Close sources"}
            className={styles.webSourcesClose}
            onClick={onCloseWebSources}
            type="button"
          >
            <svg
              aria-hidden="true"
              fill="none"
              height="20"
              stroke="currentColor"
              viewBox="0 0 24 24"
              width="20"
            >
              <path
                d="M18 6 6 18M6 6l12 12"
                strokeLinecap="round"
                strokeLinejoin="round"
                strokeWidth="1.8"
              />
            </svg>
          </button>
        </div>

        <div className={styles.webSourcesList}>
          {activeWebSources.sources.map((source, index) => (
            <div className={styles.webSourceCard} key={`${source.url}-${index}`}>
              <div className={styles.webSourceTitle}>
                <a
                  className={styles.webSourceLink}
                  href={source.url}
                  rel="noreferrer"
                  target="_blank"
                >
                  {source.title || source.url}
                </a>
              </div>
              <div className={styles.webSourceUrl}>{source.url}</div>
              {source.snippet ? (
                <div className={styles.webSourceSnippet}>{source.snippet}</div>
              ) : null}
            </div>
          ))}
        </div>
      </div>
    </div>
  );
}

function WorkspaceContextRail({
  workspaceId,
  selectedSourceIds,
  onSelectedSourceIdsChange,
  focusedSourceId = null,
}: WorkspaceContextRailProps) {
  const auth = useAuth();
  const queryClient = useQueryClient();
  const { locale } = useUiPreferences();
  const selectionInitializedRef = useRef(false);
  const previousSourceStatusesRef = useRef(new Map<string, string>());
  const noteSyncTimerRef = useRef<number | null>(null);
  const noteSyncRevisionRef = useRef(0);
  const pendingUploadedModeSwitchRef = useRef(false);

  const [urlSource, setUrlSource] = useState("");
  const [activeNoteId, setActiveNoteId] = useState<string | null>(null);
  const [draftTitle, setDraftTitle] = useState("");
  const [draftContent, setDraftContent] = useState("");
  const [noteSyncState, setNoteSyncState] = useState(WorkspaceNoteSyncState.Idle);
  const [viewerSourceId, setViewerSourceId] = useState<string | null>(null);
  const [viewerRawContent, setViewerRawContent] = useState("");
  const [viewerRawSummary, setViewerRawSummary] = useState("");
  const [viewerRawLoading, setViewerRawLoading] = useState(false);
  const [viewerError, setViewerError] = useState("");
  const [sourceUploadPending, setSourceUploadPending] = useState(false);
  const [transientProcessingSourceIds, setTransientProcessingSourceIds] = useState<string[]>([]);
  const [sourceActionError, setSourceActionError] = useState("");
  const [noteActionError, setNoteActionError] = useState("");
  const transientProcessingTimersRef = useRef<number[]>([]);

  const sourcesQuery = useWorkspaceSourcesQuery(auth.token, workspaceId);
  const notesQuery = useWorkspaceNotesQuery(auth.token, workspaceId);
  const deleteSourceMutation = useDeleteWorkspaceSourceMutation(auth.token, workspaceId);
  const reindexSourceMutation = useReindexWorkspaceSourceMutation(auth.token, workspaceId);
  const createNoteMutation = useCreateWorkspaceNoteMutation(auth.token, workspaceId);
  const updateNoteMutation = useUpdateWorkspaceNoteMutation(auth.token, workspaceId);
  const deleteNoteMutation = useDeleteWorkspaceNoteMutation(auth.token, workspaceId);
  const promoteNoteMutation = usePromoteWorkspaceNoteMutation(auth.token, workspaceId);

  const sources = sourcesQuery.data ?? [];
  const notes = notesQuery.data ?? [];
  const sourceViewerQuery = useWorkspaceSourceViewerQuery(
    auth.token,
    workspaceId,
    viewerSourceId,
    null,
  );

  const viewerPreview = useMemo(
    () => sourceViewerQuery.data?.pages.flatMap((page) => page.items) ?? [],
    [sourceViewerQuery.data],
  );
  const viewerSummary = useMemo(() => {
    const previewSummary = sourceViewerQuery.data?.pages.find((page) => page.summary)?.summary ?? "";

    return previewSummary || viewerRawSummary;
  }, [sourceViewerQuery.data, viewerRawSummary]);
  const viewerSource = viewerSourceId ? sources.find((source) => source.id === viewerSourceId) ?? null : null;
  const activeNote = activeNoteId ? notes.find((note) => note.id === activeNoteId) ?? null : null;
  const sourcesLoading = sourcesQuery.isPending;
  const sourcesPolling =
    Boolean(sources.length > 0) &&
    sourcesQuery.isFetching &&
    !sourcesQuery.isPending &&
    sources.some((source) => !isWorkspaceSourceTerminal(source.status));
  const notesLoading = notesQuery.isPending;
  const sourcesError =
    sourceActionError || (sourcesQuery.isError ? formatUiMessage(locale, "workspaceRightRail.sourcesError") : "");
  const notesError =
    noteActionError || (notesQuery.isError ? formatUiMessage(locale, "workspaceRightRail.notesError") : "");
  const viewerLoading = Boolean(viewerSourceId) && (sourceViewerQuery.isPending || viewerRawLoading);
  const viewerLoadingMore = sourceViewerQuery.isFetchingNextPage;
  const viewerHasMore = Boolean(sourceViewerQuery.hasNextPage && !viewerRawContent);

  const clearNoteSyncTimer = useCallback(() => {
    if (noteSyncTimerRef.current !== null) {
      window.clearTimeout(noteSyncTimerRef.current);
      noteSyncTimerRef.current = null;
    }
  }, []);

  const markSourcesTransientProcessing = useCallback((sourceIds: string[]) => {
    if (sourceIds.length === 0) {
      return;
    }

    setTransientProcessingSourceIds((current) => Array.from(new Set([...current, ...sourceIds])));

    const timer = window.setTimeout(() => {
      setTransientProcessingSourceIds((current) => current.filter((id) => !sourceIds.includes(id)));
      transientProcessingTimersRef.current = transientProcessingTimersRef.current.filter((item) => item !== timer);
    }, TRANSIENT_SOURCE_PROCESSING_MS);

    transientProcessingTimersRef.current.push(timer);
  }, []);

  useEffect(() => {
    return () => {
      transientProcessingTimersRef.current.forEach((timer) => window.clearTimeout(timer));
      transientProcessingTimersRef.current = [];
    };
  }, []);

  useEffect(() => {
    selectionInitializedRef.current = false;
    previousSourceStatusesRef.current = new Map();
    clearNoteSyncTimer();
    setUrlSource("");
    setActiveNoteId(null);
    setDraftTitle("");
    setDraftContent("");
    setNoteSyncState(WorkspaceNoteSyncState.Idle);
    setViewerSourceId(null);
    setViewerRawContent("");
    setViewerRawSummary("");
    setViewerRawLoading(false);
    setViewerError("");
    setSourceUploadPending(false);
    setSourceActionError("");
    setNoteActionError("");
    noteSyncRevisionRef.current = 0;
    pendingUploadedModeSwitchRef.current = false;
  }, [clearNoteSyncTimer, workspaceId]);

  useEffect(() => {
    if (sources.length === 0) {
      return;
    }

    const currentStatuses = new Map(sources.map((source) => [source.id, source.status]));
    const eligibleIds = sources
      .filter((source) => isWorkspaceSourceDocscopeEligible(source.status))
      .map((source) => source.id);

    let nextSelected = selectedSourceIds.filter((id) => currentStatuses.has(id));

    if (!selectionInitializedRef.current) {
      selectionInitializedRef.current = true;

      if (nextSelected.length === 0) {
        nextSelected = eligibleIds;
      }
    } else {
      for (const source of sources) {
        const previousStatus = previousSourceStatusesRef.current.get(source.id);

        if (
          isWorkspaceSourceDocscopeEligible(source.status) &&
          !isWorkspaceSourceDocscopeEligible(previousStatus ?? "") &&
          !nextSelected.includes(source.id)
        ) {
          nextSelected.push(source.id);
        }
      }
    }

    if (!arraysEqual(nextSelected, selectedSourceIds)) {
      onSelectedSourceIdsChange(nextSelected);
    }

    if (pendingUploadedModeSwitchRef.current) {
      const hasEligibleSelectedSource = nextSelected.some((id) =>
        isWorkspaceSourceDocscopeEligible(currentStatuses.get(id) ?? ""),
      );

      if (hasEligibleSelectedSource) {
        const workspaceUi = getWorkspaceUiState(workspaceId);

        if (workspaceUi.chatModePreference === "manual" && workspaceUi.chatMode !== "general") {
          pendingUploadedModeSwitchRef.current = false;
        } else {
          workspaceUiStore.getState().setChatMode(workspaceId, "rag", "auto");
          pendingUploadedModeSwitchRef.current = false;
        }
      }
    }

    previousSourceStatusesRef.current = currentStatuses;
  }, [onSelectedSourceIdsChange, selectedSourceIds, sources, workspaceId]);

  useEffect(() => {
    if (!activeNoteId) {
      setDraftTitle("");
      setDraftContent("");
      setNoteSyncState(WorkspaceNoteSyncState.Idle);
      return;
    }

    if (!activeNote) {
      setActiveNoteId(null);
      setDraftTitle("");
      setDraftContent("");
      setNoteSyncState(WorkspaceNoteSyncState.Idle);
      return;
    }

    setDraftTitle(activeNote.title);
    setDraftContent(activeNote.content);

    if (!noteSyncTimerRef.current) {
      setNoteSyncState(WorkspaceNoteSyncState.Synced);
    }
  }, [activeNote, activeNoteId]);

  useEffect(() => {
    if (!activeNoteId || !activeNote) {
      return;
    }

    const isClean = activeNote.title === draftTitle && activeNote.content === draftContent;
    clearNoteSyncTimer();

    if (isClean) {
      setNoteSyncState(WorkspaceNoteSyncState.Synced);
      return;
    }

    if (!auth.token || !workspaceId) {
      setNoteSyncState(WorkspaceNoteSyncState.Error);
      return;
    }

    setNoteSyncState(WorkspaceNoteSyncState.Syncing);
    const revision = noteSyncRevisionRef.current + 1;
    noteSyncRevisionRef.current = revision;

    noteSyncTimerRef.current = window.setTimeout(async () => {
      noteSyncTimerRef.current = null;

      try {
        await updateNoteMutation.mutateAsync({
          noteId: activeNoteId,
          requestBody: {
            title: draftTitle,
            content: draftContent,
          },
        });

        if (noteSyncRevisionRef.current === revision) {
          setNoteSyncState(WorkspaceNoteSyncState.Synced);
          setNoteActionError("");
        }
      } catch {
        if (noteSyncRevisionRef.current === revision) {
          setNoteSyncState(WorkspaceNoteSyncState.Error);
          setNoteActionError(formatUiMessage(locale, "workspaceRightRail.notesError"));
        }
      }
    }, 700);

    return clearNoteSyncTimer;
  }, [
    activeNote,
    activeNoteId,
    auth.token,
    clearNoteSyncTimer,
    draftContent,
    draftTitle,
    locale,
    updateNoteMutation,
    workspaceId,
  ]);

  useEffect(() => {
    return () => {
      clearNoteSyncTimer();
    };
  }, [clearNoteSyncTimer]);

  useEffect(() => {
    if (!viewerSourceId) {
      setViewerRawContent("");
      setViewerRawSummary("");
      setViewerRawLoading(false);
      setViewerError("");
      return;
    }

    setViewerRawContent("");
    setViewerRawSummary("");
    setViewerRawLoading(false);
    setViewerError("");
  }, [
    viewerSourceId,
  ]);

  useEffect(() => {
    if (!viewerSourceId || !auth.token || !sourceViewerQuery.isError) {
      return;
    }

    if (viewerRawContent || viewerRawLoading || viewerError) {
      return;
    }

    let cancelled = false;
    setViewerRawLoading(true);

    void fetchWorkspaceSourceRawContent(queryClient, auth.token, workspaceId, viewerSourceId)
      .then((response) => {
        if (cancelled) {
          return;
        }

        setViewerRawContent(response.content);
        setViewerRawSummary(response.summary ?? "");
        setViewerError("");
      })
      .catch(() => {
        if (!cancelled) {
          setViewerError(formatUiMessage(locale, "workspaceRightRail.viewerError"));
        }
      })
      .finally(() => {
        if (!cancelled) {
          setViewerRawLoading(false);
        }
      });

    return () => {
      cancelled = true;
    };
  }, [
    auth.token,
    locale,
    queryClient,
    sourceViewerQuery.isError,
    viewerError,
    viewerRawContent,
    viewerRawLoading,
    viewerSourceId,
    workspaceId,
  ]);

  useEffect(() => {
    if (viewerSourceId && !sourcesQuery.isPending && !sources.some((source) => source.id === viewerSourceId)) {
      setViewerSourceId(null);
    }
  }, [sources, sourcesQuery.isPending, viewerSourceId]);

  const handleSelectAll = useCallback(() => {
    const nextSelected = selectedSourceIds.length === sources.length ? [] : sources.map((source) => source.id);
    onSelectedSourceIdsChange(nextSelected);
  }, [onSelectedSourceIdsChange, selectedSourceIds.length, sources]);

  const handleSelectedSourceToggle = useCallback(
    (sourceId: string) => {
      const nextSelected = selectedSourceIds.includes(sourceId)
        ? selectedSourceIds.filter((id) => id !== sourceId)
        : [...selectedSourceIds, sourceId];
      onSelectedSourceIdsChange(nextSelected);
    },
    [onSelectedSourceIdsChange, selectedSourceIds],
  );

  const handleAddUrlSource = useCallback(async () => {
    if (!auth.token || !workspaceId) {
      return false;
    }

    const urls = urlSource
      .split(/\r?\n/)
      .map((value) => value.trim())
      .filter(Boolean);

    if (urls.length === 0) {
      return false;
    }

    try {
      for (const url of urls) {
        await addWorkspaceSourceUrl(auth.token, workspaceId, url);
      }

      await queryClient.invalidateQueries({ queryKey: queryKeys.workspace.sources(workspaceId) });
      setUrlSource("");
      setSourceActionError("");
      return true;
    } catch {
      setSourceActionError(formatUiMessage(locale, "workspaceRightRail.sourcesError"));
      return false;
    }
  }, [auth.token, locale, queryClient, urlSource, workspaceId]);

  const handleUploadFiles = useCallback(
    async (files: File[]) => {
      if (!auth.token || !workspaceId || files.length === 0) {
        return;
      }

      setSourceUploadPending(true);

      try {
        const uploadedSourceIds: string[] = [];

        for (const file of files) {
          const upload = await createWorkspaceDocumentUpload(auth.token, workspaceId, {
            filename: file.name,
            file_size: file.size,
            mime_type: file.type || "application/octet-stream",
          });

          await uploadWorkspaceDocumentFile(upload.upload_url, file);
          await completeWorkspaceDocumentUpload(auth.token, upload.document_id);
          uploadedSourceIds.push(upload.document_id);
        }

        const refreshedSources = (await sourcesQuery.refetch()).data ?? [];
        const uploadedSourceIdSet = new Set(uploadedSourceIds);
        const uploadedVisibleSourceIds = refreshedSources
          .filter((source) => uploadedSourceIdSet.has(source.id))
          .map((source) => source.id);

        if (uploadedVisibleSourceIds.length > 0) {
          markSourcesTransientProcessing(uploadedVisibleSourceIds);

          const nextSelected = Array.from(new Set([...selectedSourceIds, ...uploadedVisibleSourceIds]));
          const refreshedStatuses = new Map(refreshedSources.map((source) => [source.id, source.status]));
          const hasEligibleSelectedSource = nextSelected.some((id) =>
            isWorkspaceSourceDocscopeEligible(refreshedStatuses.get(id) ?? ""),
          );
          const workspaceUi = getWorkspaceUiState(workspaceId);

          onSelectedSourceIdsChange(nextSelected);

          if (hasEligibleSelectedSource) {
            workspaceUiStore.getState().setChatMode(workspaceId, "rag", "auto");
            pendingUploadedModeSwitchRef.current = false;
          } else if (workspaceUi.chatModePreference === "auto") {
            workspaceUiStore.getState().setChatMode(workspaceId, "general", "manual");
            pendingUploadedModeSwitchRef.current = true;
          } else {
            pendingUploadedModeSwitchRef.current = false;
          }
        }

        setSourceActionError("");
      } catch (error) {
        setSourceActionError(formatUiMessage(locale, "workspaceRightRail.sourcesError"));
        throw error;
      } finally {
        setSourceUploadPending(false);
      }
    },
    [auth.token, locale, markSourcesTransientProcessing, onSelectedSourceIdsChange, selectedSourceIds, sourcesQuery, workspaceId],
  );

  const handleDeleteSource = useCallback(
    async (sourceId: string) => {
      if (!auth.token) {
        return;
      }

      try {
        await deleteSourceMutation.mutateAsync(sourceId);
        onSelectedSourceIdsChange(selectedSourceIds.filter((id) => id !== sourceId));
        setViewerSourceId((current) => (current === sourceId ? null : current));
        setSourceActionError("");
      } catch {
        setSourceActionError(formatUiMessage(locale, "workspaceRightRail.sourcesError"));
      }
    },
    [auth.token, deleteSourceMutation, locale, onSelectedSourceIdsChange, selectedSourceIds],
  );

  const handleReindexSource = useCallback(
    async (sourceId: string) => {
      if (!auth.token) {
        return;
      }

      try {
        await reindexSourceMutation.mutateAsync(sourceId);
        setSourceActionError("");
      } catch {
        setSourceActionError(formatUiMessage(locale, "workspaceRightRail.sourcesError"));
      }
    },
    [auth.token, locale, reindexSourceMutation],
  );

  const handleOpenSource = useCallback((sourceId: string) => {
    setViewerSourceId((current) => (current === sourceId ? null : sourceId));
  }, []);

  const handleCreateNote = useCallback(async () => {
    if (!auth.token || !workspaceId) {
      return;
    }

    try {
      const response = await createNoteMutation.mutateAsync({
        title: null,
        content: null,
      });

      setActiveNoteId(response.note.id);
      setDraftTitle(response.note.title);
      setDraftContent(response.note.content);
      setNoteSyncState(WorkspaceNoteSyncState.Synced);
      setNoteActionError("");
    } catch {
      setNoteActionError(formatUiMessage(locale, "workspaceRightRail.notesError"));
    }
  }, [auth.token, createNoteMutation, locale, workspaceId]);

  const handleDeleteActiveNote = useCallback(async () => {
    if (!auth.token || !workspaceId || !activeNoteId) {
      return;
    }

    try {
      await deleteNoteMutation.mutateAsync(activeNoteId);
      setActiveNoteId(null);
      setDraftTitle("");
      setDraftContent("");
      setNoteSyncState(WorkspaceNoteSyncState.Idle);
      noteSyncRevisionRef.current += 1;
      clearNoteSyncTimer();
      setNoteActionError("");
    } catch {
      setNoteActionError(formatUiMessage(locale, "workspaceRightRail.notesError"));
    }
  }, [activeNoteId, auth.token, clearNoteSyncTimer, deleteNoteMutation, locale, workspaceId]);

  const handleDeleteNote = useCallback(
    async (noteId: string) => {
      if (!auth.token || !workspaceId) {
        return;
      }

      try {
        await deleteNoteMutation.mutateAsync(noteId);

        if (activeNoteId === noteId) {
          setActiveNoteId(null);
          setDraftTitle("");
          setDraftContent("");
          setNoteSyncState(WorkspaceNoteSyncState.Idle);
          clearNoteSyncTimer();
          noteSyncRevisionRef.current += 1;
        }

        setNoteActionError("");
      } catch {
        setNoteActionError(formatUiMessage(locale, "workspaceRightRail.notesError"));
      }
    },
    [activeNoteId, auth.token, clearNoteSyncTimer, deleteNoteMutation, locale, workspaceId],
  );

  const handlePromoteActiveNote = useCallback(async () => {
    if (!auth.token || !workspaceId || !activeNoteId) {
      return;
    }

    clearNoteSyncTimer();
    noteSyncRevisionRef.current += 1;
    setNoteSyncState(WorkspaceNoteSyncState.Syncing);

    try {
      const needsSave = !activeNote || activeNote.title !== draftTitle || activeNote.content !== draftContent;

      if (needsSave) {
        await updateNoteMutation.mutateAsync({
          noteId: activeNoteId,
          requestBody: {
            title: draftTitle,
            content: draftContent,
          },
        });
      }

      await promoteNoteMutation.mutateAsync(activeNoteId);
      setNoteSyncState(WorkspaceNoteSyncState.Synced);
      setNoteActionError("");
      setActiveNoteId(null);
    } catch (error) {
      setNoteSyncState(WorkspaceNoteSyncState.Error);
      setNoteActionError(getNoteMutationErrorMessage(locale, "promote", error));
    }
  }, [
    activeNote,
    activeNoteId,
    auth.token,
    clearNoteSyncTimer,
    draftContent,
    draftTitle,
    locale,
    promoteNoteMutation,
    updateNoteMutation,
    workspaceId,
  ]);

  const handlePromoteNote = useCallback(
    async (noteId: string) => {
      if (!auth.token || !workspaceId) {
        return;
      }

      try {
        await promoteNoteMutation.mutateAsync(noteId);
        setNoteActionError("");
      } catch (error) {
        setNoteActionError(getNoteMutationErrorMessage(locale, "promote", error));
      }
    },
    [auth.token, locale, promoteNoteMutation, workspaceId],
  );

  const handleSaveActiveNote = useCallback(async () => {
    if (!auth.token || !workspaceId || !activeNoteId) {
      return;
    }

    clearNoteSyncTimer();
    noteSyncRevisionRef.current += 1;

    if (activeNote && activeNote.title === draftTitle && activeNote.content === draftContent) {
      setNoteSyncState(WorkspaceNoteSyncState.Synced);
      setNoteActionError("");
      setActiveNoteId(null);
      return;
    }

    setNoteSyncState(WorkspaceNoteSyncState.Syncing);

    try {
      await updateNoteMutation.mutateAsync({
        noteId: activeNoteId,
        requestBody: {
          title: draftTitle,
          content: draftContent,
        },
      });
      setNoteSyncState(WorkspaceNoteSyncState.Synced);
      setNoteActionError("");
      setActiveNoteId(null);
    } catch (error) {
      setNoteSyncState(WorkspaceNoteSyncState.Error);
      setNoteActionError(getNoteMutationErrorMessage(locale, "save", error));
    }
  }, [
    activeNote,
    activeNoteId,
    auth.token,
    clearNoteSyncTimer,
    draftContent,
    draftTitle,
    locale,
    updateNoteMutation,
    workspaceId,
  ]);

  const noteEditorOpen = Boolean(activeNoteId);
  const sourceViewerOpen = Boolean(viewerSourceId) && !noteEditorOpen;

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
