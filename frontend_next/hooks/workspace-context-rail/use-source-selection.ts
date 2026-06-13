"use client";

import { useCallback, useEffect, useRef } from "react";

import {
  isWorkspaceSourceDocscopeEligible,
} from "../../lib/workspace/model";
import { getWorkspaceUiState, workspaceUiStore } from "../../lib/workspace/ui-store";
import { arraysEqual } from "../../components/workspace/parts/workspace-right-rail-helpers";

export function useWorkspaceSourceSelection({
  workspaceId,
  sources,
  selectedSourceIds,
  onSelectedSourceIdsChange,
  pendingUploadedModeSwitchRef,
}: {
  workspaceId: string;
  sources: Array<{ id: string; status: string }>;
  selectedSourceIds: string[];
  onSelectedSourceIdsChange(ids: string[]): void;
  pendingUploadedModeSwitchRef: React.MutableRefObject<boolean>;
}) {
  const selectionInitializedRef = useRef(false);
  const previousSourceStatusesRef = useRef(new Map<string, string>());

  useEffect(() => {
    selectionInitializedRef.current = false;
    previousSourceStatusesRef.current = new Map();
    pendingUploadedModeSwitchRef.current = false;
  }, [pendingUploadedModeSwitchRef, workspaceId]);

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

        if (workspaceUi.chatModePreference === "manual" && workspaceUi.chatMode !== "chat") {
          pendingUploadedModeSwitchRef.current = false;
        } else {
          workspaceUiStore.getState().setChatMode(workspaceId, "rag", "auto");
          pendingUploadedModeSwitchRef.current = false;
        }
      }
    }

    previousSourceStatusesRef.current = currentStatuses;
  }, [onSelectedSourceIdsChange, pendingUploadedModeSwitchRef, selectedSourceIds, sources, workspaceId]);

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

  return {
    handleSelectAll,
    handleSelectedSourceToggle,
  };
}
