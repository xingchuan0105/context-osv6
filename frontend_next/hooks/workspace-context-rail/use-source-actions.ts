"use client";

import { useQueryClient } from "@tanstack/react-query";
import { useCallback, useEffect, useRef, useState } from "react";

import { formatUiMessage } from "../../lib/i18n/messages";
import { queryKeys } from "../../lib/query/keys";
import {
  addWorkspaceSourceUrl,
  completeWorkspaceDocumentUpload,
  createWorkspaceDocumentUpload,
  uploadWorkspaceDocumentFile,
} from "../../lib/workspace/client";
import {
  useDeleteWorkspaceSourceMutation,
  useReindexWorkspaceSourceMutation,
  useWorkspaceSourcesQuery,
} from "../../lib/workspace/right-rail-queries";
import {
  isWorkspaceSourceDocscopeEligible,
  isWorkspaceSourceTerminal,
} from "../../lib/workspace/model";
import { getWorkspaceUiState, workspaceUiStore } from "../../lib/workspace/ui-store";

export function useWorkspaceSourceActions({
  token,
  workspaceId,
  locale,
  selectedSourceIds,
  onSelectedSourceIdsChange,
  markSourcesTransientProcessing,
  pendingUploadedModeSwitchRef,
  setViewerSourceIdRef,
}: {
  token: string | null;
  workspaceId: string;
  locale: "zh-CN" | "en";
  selectedSourceIds: string[];
  onSelectedSourceIdsChange(ids: string[]): void;
  markSourcesTransientProcessing(sourceIds: string[]): void;
  pendingUploadedModeSwitchRef: React.MutableRefObject<boolean>;
  setViewerSourceIdRef: React.MutableRefObject<React.Dispatch<React.SetStateAction<string | null>>>;
}) {
  const queryClient = useQueryClient();

  const [urlSource, setUrlSource] = useState("");
  const [sourceUploadPending, setSourceUploadPending] = useState(false);
  const [sourceActionError, setSourceActionError] = useState("");

  const sourcesQuery = useWorkspaceSourcesQuery(token, workspaceId);
  const deleteSourceMutation = useDeleteWorkspaceSourceMutation(token, workspaceId);
  const reindexSourceMutation = useReindexWorkspaceSourceMutation(token, workspaceId);

  const sources = sourcesQuery.data ?? [];
  const sourcesLoading = sourcesQuery.isPending;
  const sourcesPolling =
    Boolean(sources.length > 0) &&
    sourcesQuery.isFetching &&
    !sourcesQuery.isPending &&
    sources.some((source) => !isWorkspaceSourceTerminal(source.status));
  const sourcesError =
    sourceActionError || (sourcesQuery.isError ? formatUiMessage(locale, "workspaceRightRail.sourcesError") : "");

  const resetSourceActions = useCallback(() => {
    setUrlSource("");
    setSourceUploadPending(false);
    setSourceActionError("");
    pendingUploadedModeSwitchRef.current = false;
  }, [pendingUploadedModeSwitchRef]);

  useEffect(() => {
    resetSourceActions();
  }, [resetSourceActions, workspaceId]);

  const handleAddUrlSource = useCallback(async () => {
    if (!token || !workspaceId) {
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
        await addWorkspaceSourceUrl(token, workspaceId, url);
      }

      await queryClient.invalidateQueries({ queryKey: queryKeys.workspace.sources(workspaceId) });
      setUrlSource("");
      setSourceActionError("");
      return true;
    } catch {
      setSourceActionError(formatUiMessage(locale, "workspaceRightRail.sourcesError"));
      return false;
    }
  }, [locale, queryClient, token, urlSource, workspaceId]);

  const handleUploadFiles = useCallback(
    async (files: File[]) => {
      if (!token || !workspaceId || files.length === 0) {
        return;
      }

      setSourceUploadPending(true);

      try {
        const uploadedSourceIds: string[] = [];

        for (const file of files) {
          const upload = await createWorkspaceDocumentUpload(token, workspaceId, {
            filename: file.name,
            file_size: file.size,
            mime_type: file.type || "application/octet-stream",
          });

          await uploadWorkspaceDocumentFile(upload.upload_url, file);
          await completeWorkspaceDocumentUpload(token, upload.document_id);
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
            workspaceUiStore.getState().setChatMode(workspaceId, "chat", "manual");
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
    [
      locale,
      markSourcesTransientProcessing,
      onSelectedSourceIdsChange,
      pendingUploadedModeSwitchRef,
      selectedSourceIds,
      sourcesQuery,
      token,
      workspaceId,
    ],
  );

  const handleDeleteSource = useCallback(
    async (sourceId: string) => {
      if (!token) {
        return;
      }

      try {
        await deleteSourceMutation.mutateAsync(sourceId);
        onSelectedSourceIdsChange(selectedSourceIds.filter((id) => id !== sourceId));
        setViewerSourceIdRef.current((current) => (current === sourceId ? null : current));
        setSourceActionError("");
      } catch {
        setSourceActionError(formatUiMessage(locale, "workspaceRightRail.sourcesError"));
      }
    },
    [deleteSourceMutation, locale, onSelectedSourceIdsChange, selectedSourceIds, setViewerSourceIdRef, token],
  );

  const handleReindexSource = useCallback(
    async (sourceId: string) => {
      if (!token) {
        return;
      }

      try {
        await reindexSourceMutation.mutateAsync(sourceId);
        setSourceActionError("");
      } catch {
        setSourceActionError(formatUiMessage(locale, "workspaceRightRail.sourcesError"));
      }
    },
    [locale, reindexSourceMutation, token],
  );

  return {
    handleAddUrlSource,
    handleDeleteSource,
    handleReindexSource,
    handleUploadFiles,
    resetSourceActions,
    setUrlSource,
    sourceUploadPending,
    sources,
    sourcesError,
    sourcesLoading,
    sourcesPolling,
    urlSource,
  };
}
