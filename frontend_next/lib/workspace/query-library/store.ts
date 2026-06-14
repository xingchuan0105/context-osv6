"use client";

import { useMemo } from "react";
import { createJSONStorage, persist, type PersistStorage } from "zustand/middleware";
import { useStore } from "zustand/react";
import { createStore } from "zustand/vanilla";

import {
  captureQueryLibraryItems,
  removeQueryLibraryItem,
  touchQueryLibraryItem,
} from "./logic";
import type { QueryLibraryItem } from "./types";

type QueryLibraryData = {
  workspaces: Record<string, QueryLibraryItem[]>;
};

type QueryLibraryStore = QueryLibraryData & {
  capture: (workspaceId: string, raw: string) => void;
  remove: (workspaceId: string, id: string) => void;
  touch: (workspaceId: string, id: string) => void;
  clear: (workspaceId: string) => void;
};

type QueryLibraryStoreOptions = {
  name?: string;
  storage?: PersistStorage<QueryLibraryData>;
};

export const QUERY_LIBRARY_STORAGE_KEY = "context-os.query-library.v1";

function readWorkspaceItems(workspaces: Record<string, QueryLibraryItem[]>, workspaceId: string) {
  if (!workspaceId) {
    return [];
  }

  const items = workspaces[workspaceId] ?? [];
  return items.filter(
    (item) =>
      typeof item.id === "string" &&
      item.id.length > 0 &&
      typeof item.text === "string" &&
      item.text.trim().length > 0 &&
      typeof item.createdAt === "number" &&
      typeof item.lastUsedAt === "number" &&
      typeof item.useCount === "number",
  );
}

function updateWorkspaceItems(
  workspaces: Record<string, QueryLibraryItem[]>,
  workspaceId: string,
  updater: (items: QueryLibraryItem[]) => QueryLibraryItem[],
) {
  if (!workspaceId) {
    return workspaces;
  }

  return {
    ...workspaces,
    [workspaceId]: updater(readWorkspaceItems(workspaces, workspaceId)),
  };
}

function createQueryLibraryStorage() {
  if (typeof window === "undefined") {
    return undefined;
  }

  return createJSONStorage<QueryLibraryData>(() => window.localStorage);
}

export function createQueryLibraryStore({
  name = QUERY_LIBRARY_STORAGE_KEY,
  storage = createQueryLibraryStorage(),
}: QueryLibraryStoreOptions = {}) {
  return createStore<QueryLibraryStore>()(
    persist(
      (set) => ({
        workspaces: {},
        capture: (workspaceId, raw) =>
          set((state) => ({
            workspaces: updateWorkspaceItems(state.workspaces, workspaceId, (items) =>
              captureQueryLibraryItems(items, raw, Date.now()),
            ),
          })),
        remove: (workspaceId, id) =>
          set((state) => ({
            workspaces: updateWorkspaceItems(state.workspaces, workspaceId, (items) =>
              removeQueryLibraryItem(items, id),
            ),
          })),
        touch: (workspaceId, id) =>
          set((state) => ({
            workspaces: updateWorkspaceItems(state.workspaces, workspaceId, (items) =>
              touchQueryLibraryItem(items, id, Date.now()),
            ),
          })),
        clear: (workspaceId) =>
          set((state) => ({
            workspaces: updateWorkspaceItems(state.workspaces, workspaceId, () => []),
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

export const queryLibraryStore = createQueryLibraryStore();

const EMPTY_QUERY_LIBRARY_ITEMS: QueryLibraryItem[] = [];

export function getQueryLibraryItems(workspaceId: string) {
  return readWorkspaceItems(queryLibraryStore.getState().workspaces, workspaceId);
}

export function useQueryLibraryItems(workspaceId: string) {
  const rawItems = useStore(queryLibraryStore, (state) => {
    const items = state.workspaces[workspaceId];
    if (!items || items.length === 0) {
      return EMPTY_QUERY_LIBRARY_ITEMS;
    }

    return items;
  });

  return useMemo(() => {
    if (rawItems === EMPTY_QUERY_LIBRARY_ITEMS) {
      return EMPTY_QUERY_LIBRARY_ITEMS;
    }

    const filtered = readWorkspaceItems(queryLibraryStore.getState().workspaces, workspaceId);
    if (filtered.length === rawItems.length) {
      return rawItems;
    }

    return filtered;
  }, [rawItems, workspaceId]);
}

export function useQueryLibrary(workspaceId: string) {
  const items = useQueryLibraryItems(workspaceId);

  return useMemo(
    () => ({
      items,
      capture: (raw: string) => queryLibraryStore.getState().capture(workspaceId, raw),
      remove: (id: string) => queryLibraryStore.getState().remove(workspaceId, id),
      touch: (id: string) => queryLibraryStore.getState().touch(workspaceId, id),
      clear: () => queryLibraryStore.getState().clear(workspaceId),
    }),
    [items, workspaceId],
  );
}
