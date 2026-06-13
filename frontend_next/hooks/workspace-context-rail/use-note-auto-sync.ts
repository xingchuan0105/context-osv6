"use client";

import { useEffect } from "react";

import { formatUiMessage } from "../../lib/i18n/messages";
import { WorkspaceNoteSyncState } from "../../lib/workspace/model";
import type { WorkspaceNote } from "../../lib/workspace/model";

type UpdateNoteMutation = {
  mutateAsync(input: {
    noteId: string;
    requestBody: {
      title: string | null;
      content: string | null;
    };
  }): Promise<unknown>;
};

export function useWorkspaceNoteAutoSync({
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
}: {
  activeNote: WorkspaceNote | null;
  activeNoteId: string | null;
  clearNoteSyncTimer: () => void;
  draftContent: string;
  draftTitle: string;
  locale: "zh-CN" | "en";
  noteSyncRevisionRef: React.MutableRefObject<number>;
  noteSyncTimerRef: React.MutableRefObject<number | null>;
  setNoteActionError: React.Dispatch<React.SetStateAction<string>>;
  setNoteSyncState: React.Dispatch<React.SetStateAction<WorkspaceNoteSyncState>>;
  token: string | null;
  updateNoteMutation: UpdateNoteMutation;
  workspaceId: string;
}) {
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

    if (!token || !workspaceId) {
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
  ]);
}
