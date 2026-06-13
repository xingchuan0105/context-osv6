"use client";

import { useCallback } from "react";

import { formatUiMessage } from "../../lib/i18n/messages";
import { getNoteMutationErrorMessage } from "../../components/workspace/parts/workspace-right-rail-helpers";
import { WorkspaceNoteSyncState } from "../../lib/workspace/model";

type DeleteNoteMutation = {
  mutateAsync(noteId: string): Promise<unknown>;
};

type PromoteNoteMutation = {
  mutateAsync(noteId: string): Promise<unknown>;
};

export function useWorkspaceNoteListActions({
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
}: {
  activeNoteId: string | null;
  clearNoteSyncTimer: () => void;
  locale: "zh-CN" | "en";
  noteSyncRevisionRef: React.MutableRefObject<number>;
  deleteNoteMutation: DeleteNoteMutation;
  promoteNoteMutation: PromoteNoteMutation;
  setActiveNoteId: React.Dispatch<React.SetStateAction<string | null>>;
  setDraftContent: React.Dispatch<React.SetStateAction<string>>;
  setDraftTitle: React.Dispatch<React.SetStateAction<string>>;
  setNoteActionError: React.Dispatch<React.SetStateAction<string>>;
  setNoteSyncState: React.Dispatch<React.SetStateAction<WorkspaceNoteSyncState>>;
  token: string | null;
  workspaceId: string;
}) {
  const handleDeleteNote = useCallback(
    async (noteId: string) => {
      if (!token || !workspaceId) {
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
    [
      activeNoteId,
      clearNoteSyncTimer,
      deleteNoteMutation,
      locale,
      noteSyncRevisionRef,
      setActiveNoteId,
      setDraftContent,
      setDraftTitle,
      setNoteActionError,
      setNoteSyncState,
      token,
      workspaceId,
    ],
  );

  const handlePromoteNote = useCallback(
    async (noteId: string) => {
      if (!token || !workspaceId) {
        return;
      }

      try {
        await promoteNoteMutation.mutateAsync(noteId);
        setNoteActionError("");
      } catch (error) {
        setNoteActionError(getNoteMutationErrorMessage(locale, "promote", error));
      }
    },
    [locale, promoteNoteMutation, setNoteActionError, token, workspaceId],
  );

  return {
    handleDeleteNote,
    handlePromoteNote,
  };
}
