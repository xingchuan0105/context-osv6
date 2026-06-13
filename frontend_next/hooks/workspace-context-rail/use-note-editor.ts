"use client";

import { useCallback, useEffect, useRef, useState } from "react";

import { formatUiMessage } from "../../lib/i18n/messages";
import {
  useCreateWorkspaceNoteMutation,
  useDeleteWorkspaceNoteMutation,
  usePromoteWorkspaceNoteMutation,
  useUpdateWorkspaceNoteMutation,
  useWorkspaceNotesQuery,
} from "../../lib/workspace/right-rail-queries";
import { WorkspaceNoteSyncState } from "../../lib/workspace/model";
import { getNoteMutationErrorMessage } from "../../components/workspace/parts/workspace-right-rail-helpers";
import { useWorkspaceNoteAutoSync } from "./use-note-auto-sync";
import { useWorkspaceNoteListActions } from "./use-note-list-actions";

export function useWorkspaceNoteEditor({
  token,
  workspaceId,
  locale,
}: {
  token: string | null;
  workspaceId: string;
  locale: "zh-CN" | "en";
}) {
  const noteSyncTimerRef = useRef<number | null>(null);
  const noteSyncRevisionRef = useRef(0);

  const [activeNoteId, setActiveNoteId] = useState<string | null>(null);
  const [draftTitle, setDraftTitle] = useState("");
  const [draftContent, setDraftContent] = useState("");
  const [noteSyncState, setNoteSyncState] = useState(WorkspaceNoteSyncState.Idle);
  const [noteActionError, setNoteActionError] = useState("");

  const notesQuery = useWorkspaceNotesQuery(token, workspaceId);
  const createNoteMutation = useCreateWorkspaceNoteMutation(token, workspaceId);
  const updateNoteMutation = useUpdateWorkspaceNoteMutation(token, workspaceId);
  const deleteNoteMutation = useDeleteWorkspaceNoteMutation(token, workspaceId);
  const promoteNoteMutation = usePromoteWorkspaceNoteMutation(token, workspaceId);

  const notes = notesQuery.data ?? [];
  const activeNote = activeNoteId ? notes.find((note) => note.id === activeNoteId) ?? null : null;
  const notesLoading = notesQuery.isPending;
  const notesError =
    noteActionError || (notesQuery.isError ? formatUiMessage(locale, "workspaceRightRail.notesError") : "");
  const noteEditorOpen = Boolean(activeNoteId);

  const clearNoteSyncTimer = useCallback(() => {
    if (noteSyncTimerRef.current !== null) {
      window.clearTimeout(noteSyncTimerRef.current);
      noteSyncTimerRef.current = null;
    }
  }, []);

  const resetNoteEditor = useCallback(() => {
    clearNoteSyncTimer();
    setActiveNoteId(null);
    setDraftTitle("");
    setDraftContent("");
    setNoteSyncState(WorkspaceNoteSyncState.Idle);
    setNoteActionError("");
    noteSyncRevisionRef.current = 0;
  }, [clearNoteSyncTimer]);

  useEffect(() => {
    resetNoteEditor();
  }, [resetNoteEditor, workspaceId]);

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

  useWorkspaceNoteAutoSync({
    activeNote,
    activeNoteId,
    clearNoteSyncTimer,
    draftContent,
    draftTitle,
    locale,
    noteSyncRevisionRef,
    noteSyncTimerRef,
    setNoteActionError,
    setNoteSyncState,
    token,
    updateNoteMutation,
    workspaceId,
  });

  const listActions = useWorkspaceNoteListActions({
    activeNoteId,
    clearNoteSyncTimer,
    locale,
    noteSyncRevisionRef,
    deleteNoteMutation,
    promoteNoteMutation,
    setActiveNoteId,
    setDraftContent,
    setDraftTitle,
    setNoteActionError,
    setNoteSyncState,
    token,
    workspaceId,
  });

  useEffect(() => {
    return () => {
      clearNoteSyncTimer();
    };
  }, [clearNoteSyncTimer]);

  const handleCreateNote = useCallback(async () => {
    if (!token || !workspaceId) {
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
  }, [createNoteMutation, locale, token, workspaceId]);

  const handleDeleteActiveNote = useCallback(async () => {
    if (!token || !workspaceId || !activeNoteId) {
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
  }, [activeNoteId, clearNoteSyncTimer, deleteNoteMutation, locale, token, workspaceId]);

  const handlePromoteActiveNote = useCallback(async () => {
    if (!token || !workspaceId || !activeNoteId) {
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
    clearNoteSyncTimer,
    draftContent,
    draftTitle,
    locale,
    promoteNoteMutation,
    token,
    updateNoteMutation,
    workspaceId,
  ]);

  const handleSaveActiveNote = useCallback(async () => {
    if (!token || !workspaceId || !activeNoteId) {
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
    clearNoteSyncTimer,
    draftContent,
    draftTitle,
    locale,
    token,
    updateNoteMutation,
    workspaceId,
  ]);

  return {
    activeNoteId,
    draftContent,
    draftTitle,
    handleCreateNote,
    handleDeleteActiveNote,
    handleDeleteNote: listActions.handleDeleteNote,
    handlePromoteActiveNote,
    handlePromoteNote: listActions.handlePromoteNote,
    handleSaveActiveNote,
    noteEditorOpen,
    notes,
    notesError,
    notesLoading,
    noteSyncState,
    setActiveNoteId,
    setDraftContent,
    setDraftTitle,
  };
}
