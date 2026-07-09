"use client";

import { useMemo } from "react";
import { createJSONStorage, persist, type PersistStorage } from "zustand/middleware";
import { useStore } from "zustand/react";
import { createStore } from "zustand/vanilla";

import type { WorkspaceCitationRequest } from "./model";

export type WorkspaceChatMode = "rag" | "search" | "chat" | "write";
type WorkspaceChatModeInput = WorkspaceChatMode | "general";
export type WorkspaceChatModePreference = "auto" | "manual";

export type WorkspaceUiState = {
  historyRailOpen: boolean;
  historyRailWidth: number;
  rightRailOpen: boolean;
  rightRailWidth: number;
  rightRailSplitRatio: number;
  selectedSourceIds: string[];
  focusedSourceId: string | null;
  activeCitation: WorkspaceCitationRequest | null;
  chatMode: WorkspaceChatMode;
  chatModePreference: WorkspaceChatModePreference;
};

type WorkspaceUiData = {
  workspaces: Record<string, WorkspaceUiState>;
};

type WorkspaceUiStore = WorkspaceUiData & {
  resetWorkspace: (workspaceId: string) => void;
  setHistoryRailOpen: (workspaceId: string, open: boolean) => void;
  setHistoryRailWidth: (workspaceId: string, width: number) => void;
  toggleHistoryRail: (workspaceId: string) => void;
  setRightRailOpen: (workspaceId: string, open: boolean) => void;
  setRightRailWidth: (workspaceId: string, width: number) => void;
  toggleRightRail: (workspaceId: string) => void;
  setRightRailSplitRatio: (workspaceId: string, ratio: number) => void;
  setSelectedSourceIds: (workspaceId: string, sourceIds: readonly string[]) => void;
  toggleSelectedSourceId: (workspaceId: string, sourceId: string) => void;
  setFocusedSourceId: (workspaceId: string, sourceId: string | null) => void;
  setActiveCitation: (workspaceId: string, citation: WorkspaceCitationRequest | null) => void;
  setChatMode: (
    workspaceId: string,
    mode: WorkspaceChatModeInput,
    preference?: WorkspaceChatModePreference,
  ) => void;
};

type WorkspaceUiStoreOptions = {
  name?: string;
  storage?: PersistStorage<WorkspaceUiData>;
};

export const WORKSPACE_UI_STORAGE_KEY = "context-os.workspace-ui.v1";

const HISTORY_RAIL_MIN_WIDTH = 236;
const HISTORY_RAIL_MAX_WIDTH = 320;
const RIGHT_RAIL_MIN_WIDTH = 304;
const RIGHT_RAIL_MAX_WIDTH = 392;
const LEGACY_HISTORY_RAIL_DEFAULT_WIDTH = 264;
const LEGACY_RIGHT_RAIL_DEFAULT_WIDTH = 336;

export const DEFAULT_WORKSPACE_UI_STATE: WorkspaceUiState = {
  historyRailOpen: true,
  historyRailWidth: HISTORY_RAIL_MAX_WIDTH,
  rightRailOpen: true,
  rightRailWidth: RIGHT_RAIL_MAX_WIDTH,
  rightRailSplitRatio: 0.5,
  selectedSourceIds: [],
  focusedSourceId: null,
  activeCitation: null,
  chatMode: "chat",
  chatModePreference: "auto",
};

function normalizeSourceIds(sourceIds: readonly string[]) {
  return Array.from(new Set(sourceIds.map((sourceId) => sourceId.trim()).filter(Boolean)));
}

function clampRightRailSplitRatio(ratio: number) {
  if (!Number.isFinite(ratio)) {
    return DEFAULT_WORKSPACE_UI_STATE.rightRailSplitRatio;
  }

  return Math.min(0.8, Math.max(0.2, ratio));
}

function clampHistoryRailWidth(width: number) {
  if (!Number.isFinite(width)) {
    return DEFAULT_WORKSPACE_UI_STATE.historyRailWidth;
  }

  return Math.min(HISTORY_RAIL_MAX_WIDTH, Math.max(HISTORY_RAIL_MIN_WIDTH, Math.round(width)));
}

function clampRightRailWidth(width: number) {
  if (!Number.isFinite(width)) {
    return DEFAULT_WORKSPACE_UI_STATE.rightRailWidth;
  }

  return Math.min(RIGHT_RAIL_MAX_WIDTH, Math.max(RIGHT_RAIL_MIN_WIDTH, Math.round(width)));
}

function normalizeChatMode(mode: string | null | undefined): WorkspaceChatMode {
  if (mode === "general" || mode === "chat") {
    return "chat";
  }

  if (mode === "rag" || mode === "search" || mode === "write") {
    return mode;
  }

  return DEFAULT_WORKSPACE_UI_STATE.chatMode;
}

function normalizeChatModePreference(preference: WorkspaceChatModePreference | undefined) {
  if (preference === "manual") {
    return preference;
  }

  return DEFAULT_WORKSPACE_UI_STATE.chatModePreference;
}

export function getDefaultWorkspaceChatMode(hasContentSources: boolean) {
  return hasContentSources ? "rag" : "chat";
}

export function resolveWorkspaceChatMode(
  state: Pick<WorkspaceUiState, "chatMode" | "chatModePreference">,
  hasContentSources: boolean,
) {
  if (normalizeChatModePreference(state.chatModePreference) === "manual") {
    return normalizeChatMode(state.chatMode);
  }

  return getDefaultWorkspaceChatMode(hasContentSources);
}

const normalizedWorkspaceUiStateCache = new WeakMap<WorkspaceUiState, WorkspaceUiState>();

function readWorkspaceUiState(workspaces: Record<string, WorkspaceUiState>, workspaceId: string) {
  const current = workspaces[workspaceId];

  if (!current) {
    return DEFAULT_WORKSPACE_UI_STATE;
  }

  if (
    current.chatMode === normalizeChatMode(current.chatMode) &&
    current.chatModePreference === normalizeChatModePreference(current.chatModePreference) &&
    typeof current.historyRailWidth === "number" &&
    typeof current.rightRailWidth === "number" &&
    current.historyRailWidth !== LEGACY_HISTORY_RAIL_DEFAULT_WIDTH &&
    current.rightRailWidth !== LEGACY_RIGHT_RAIL_DEFAULT_WIDTH
  ) {
    return current;
  }

  const cached = normalizedWorkspaceUiStateCache.get(current);
  if (cached) {
    return cached;
  }

  const normalized = {
    ...DEFAULT_WORKSPACE_UI_STATE,
    ...current,
    chatMode: normalizeChatMode(current.chatMode),
    chatModePreference: normalizeChatModePreference(current.chatModePreference),
    historyRailWidth:
      current.historyRailWidth === LEGACY_HISTORY_RAIL_DEFAULT_WIDTH || current.historyRailWidth == null
        ? DEFAULT_WORKSPACE_UI_STATE.historyRailWidth
        : clampHistoryRailWidth(current.historyRailWidth),
    rightRailWidth:
      current.rightRailWidth === LEGACY_RIGHT_RAIL_DEFAULT_WIDTH || current.rightRailWidth == null
        ? DEFAULT_WORKSPACE_UI_STATE.rightRailWidth
        : clampRightRailWidth(current.rightRailWidth),
  };

  normalizedWorkspaceUiStateCache.set(current, normalized);

  return normalized;
}

function updateWorkspaceUiState(
  workspaces: Record<string, WorkspaceUiState>,
  workspaceId: string,
  update: (current: WorkspaceUiState) => WorkspaceUiState,
) {
  if (!workspaceId) {
    return workspaces;
  }

  return {
    ...workspaces,
    [workspaceId]: update(readWorkspaceUiState(workspaces, workspaceId)),
  };
}

function createWorkspaceUiStorage() {
  if (typeof window === "undefined") {
    return undefined;
  }

  return createJSONStorage<WorkspaceUiData>(() => window.localStorage);
}

export function createWorkspaceUiStore({
  name = WORKSPACE_UI_STORAGE_KEY,
  storage = createWorkspaceUiStorage(),
}: WorkspaceUiStoreOptions = {}) {
  return createStore<WorkspaceUiStore>()(
    persist(
      (set) => ({
        workspaces: {},
        resetWorkspace: (workspaceId) =>
          set((state) => {
            if (!workspaceId || !state.workspaces[workspaceId]) {
              return state;
            }

            const nextWorkspaces = { ...state.workspaces };
            delete nextWorkspaces[workspaceId];

            return { workspaces: nextWorkspaces };
          }),
        setHistoryRailOpen: (workspaceId, open) =>
          set((state) => ({
            workspaces: updateWorkspaceUiState(state.workspaces, workspaceId, (current) => ({
              ...current,
              historyRailOpen: open,
            })),
          })),
        setHistoryRailWidth: (workspaceId, width) =>
          set((state) => ({
            workspaces: updateWorkspaceUiState(state.workspaces, workspaceId, (current) => ({
              ...current,
              historyRailWidth: clampHistoryRailWidth(width),
            })),
          })),
        toggleHistoryRail: (workspaceId) =>
          set((state) => ({
            workspaces: updateWorkspaceUiState(state.workspaces, workspaceId, (current) => ({
              ...current,
              historyRailOpen: !current.historyRailOpen,
            })),
          })),
        setRightRailOpen: (workspaceId, open) =>
          set((state) => ({
            workspaces: updateWorkspaceUiState(state.workspaces, workspaceId, (current) => ({
              ...current,
              rightRailOpen: open,
            })),
          })),
        setRightRailWidth: (workspaceId, width) =>
          set((state) => ({
            workspaces: updateWorkspaceUiState(state.workspaces, workspaceId, (current) => ({
              ...current,
              rightRailWidth: clampRightRailWidth(width),
            })),
          })),
        toggleRightRail: (workspaceId) =>
          set((state) => ({
            workspaces: updateWorkspaceUiState(state.workspaces, workspaceId, (current) => ({
              ...current,
              rightRailOpen: !current.rightRailOpen,
            })),
          })),
        setRightRailSplitRatio: (workspaceId, ratio) =>
          set((state) => ({
            workspaces: updateWorkspaceUiState(state.workspaces, workspaceId, (current) => ({
              ...current,
              rightRailSplitRatio: clampRightRailSplitRatio(ratio),
            })),
          })),
        setSelectedSourceIds: (workspaceId, sourceIds) =>
          set((state) => ({
            workspaces: updateWorkspaceUiState(state.workspaces, workspaceId, (current) => ({
              ...current,
              selectedSourceIds: normalizeSourceIds(sourceIds),
            })),
          })),
        toggleSelectedSourceId: (workspaceId, sourceId) =>
          set((state) => ({
            workspaces: updateWorkspaceUiState(state.workspaces, workspaceId, (current) => {
              const normalizedSourceId = sourceId.trim();

              if (!normalizedSourceId) {
                return current;
              }

              return {
                ...current,
                selectedSourceIds: current.selectedSourceIds.includes(normalizedSourceId)
                  ? current.selectedSourceIds.filter((currentSourceId) => currentSourceId !== normalizedSourceId)
                  : [...current.selectedSourceIds, normalizedSourceId],
              };
            }),
          })),
        setFocusedSourceId: (workspaceId, sourceId) =>
          set((state) => ({
            workspaces: updateWorkspaceUiState(state.workspaces, workspaceId, (current) => ({
              ...current,
              focusedSourceId: sourceId,
            })),
          })),
        setActiveCitation: (workspaceId, citation) =>
          set((state) => ({
            workspaces: updateWorkspaceUiState(state.workspaces, workspaceId, (current) => ({
              ...current,
              activeCitation: citation,
            })),
          })),
        setChatMode: (workspaceId, mode, preference = "manual") =>
          set((state) => ({
            workspaces: updateWorkspaceUiState(state.workspaces, workspaceId, (current) => ({
              ...current,
              chatMode: normalizeChatMode(mode),
              chatModePreference: normalizeChatModePreference(preference),
            })),
          })),
      }),
      {
        name,
        storage,
        partialize: (state) => ({ workspaces: state.workspaces }),
      },
    ),
  );
}

export const workspaceUiStore = createWorkspaceUiStore();

export function getWorkspaceUiState(workspaceId: string) {
  return readWorkspaceUiState(workspaceUiStore.getState().workspaces, workspaceId);
}

export function useWorkspaceUiState<T>(
  workspaceId: string,
  selector: (state: WorkspaceUiState) => T,
) {
  return useStore(workspaceUiStore, (state) => selector(readWorkspaceUiState(state.workspaces, workspaceId)));
}

export function useWorkspaceUi(workspaceId: string) {
  const state = useWorkspaceUiState(workspaceId, (current) => current);

  return useMemo(
    () => ({
      ...state,
      reset: () => workspaceUiStore.getState().resetWorkspace(workspaceId),
      setHistoryRailOpen: (open: boolean) => workspaceUiStore.getState().setHistoryRailOpen(workspaceId, open),
      setHistoryRailWidth: (width: number) => workspaceUiStore.getState().setHistoryRailWidth(workspaceId, width),
      toggleHistoryRail: () => workspaceUiStore.getState().toggleHistoryRail(workspaceId),
      setRightRailOpen: (open: boolean) => workspaceUiStore.getState().setRightRailOpen(workspaceId, open),
      setRightRailWidth: (width: number) => workspaceUiStore.getState().setRightRailWidth(workspaceId, width),
      toggleRightRail: () => workspaceUiStore.getState().toggleRightRail(workspaceId),
      setRightRailSplitRatio: (ratio: number) =>
        workspaceUiStore.getState().setRightRailSplitRatio(workspaceId, ratio),
      setSelectedSourceIds: (sourceIds: readonly string[]) =>
        workspaceUiStore.getState().setSelectedSourceIds(workspaceId, sourceIds),
      toggleSelectedSourceId: (sourceId: string) =>
        workspaceUiStore.getState().toggleSelectedSourceId(workspaceId, sourceId),
      setFocusedSourceId: (sourceId: string | null) =>
        workspaceUiStore.getState().setFocusedSourceId(workspaceId, sourceId),
      setActiveCitation: (citation: WorkspaceCitationRequest | null) =>
        workspaceUiStore.getState().setActiveCitation(workspaceId, citation),
      setChatMode: (mode: WorkspaceChatModeInput, preference?: WorkspaceChatModePreference) =>
        workspaceUiStore.getState().setChatMode(workspaceId, mode, preference),
    }),
    [state, workspaceId],
  );
}
