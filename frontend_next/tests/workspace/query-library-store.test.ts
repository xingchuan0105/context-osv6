import { describe, expect, it } from "vitest";

import {
  createQueryLibraryStore,
  getQueryLibraryItems,
  queryLibraryStore,
  QUERY_LIBRARY_STORAGE_KEY,
} from "../../lib/workspace/query-library/store";

async function waitForQueryLibraryHydration(
  store: ReturnType<typeof createQueryLibraryStore>,
) {
  if (store.persist.hasHydrated()) {
    return;
  }

  await new Promise<void>((resolve) => {
    const unsubscribe = store.persist.onFinishHydration(() => {
      unsubscribe();
      resolve();
    });
  });
}

describe("queryLibraryStore", () => {
  it("keeps workspace query lists isolated by workspace id", () => {
    const store = createQueryLibraryStore({ name: "query-library-test-isolation" });

    store.getState().capture("ws-1", "Summarize this PDF");
    store.getState().capture("ws-2", "Rewrite in formal tone");

    expect(store.getState().workspaces["ws-1"]).toHaveLength(1);
    expect(store.getState().workspaces["ws-1"]?.[0]?.text).toBe("Summarize this PDF");
    expect(store.getState().workspaces["ws-2"]).toHaveLength(1);
    expect(store.getState().workspaces["ws-2"]?.[0]?.text).toBe("Rewrite in formal tone");
  });

  it("persists query library state into localStorage", () => {
    window.localStorage.removeItem(QUERY_LIBRARY_STORAGE_KEY);

    const store = createQueryLibraryStore();
    store.getState().capture("ws-1", "Summarize this PDF");

    const persisted = window.localStorage.getItem(QUERY_LIBRARY_STORAGE_KEY);

    expect(persisted).not.toBeNull();
    expect(JSON.parse(persisted!)).toMatchObject({
      state: {
        workspaces: {
          "ws-1": [{ text: "Summarize this PDF" }],
        },
      },
    });
  });

  it("removes, touches, and clears workspace items", () => {
    const store = createQueryLibraryStore({ name: "query-library-test-actions" });

    store.getState().capture("ws-1", "Summarize this PDF");
    store.getState().capture("ws-1", "Rewrite in formal tone");
    const firstId = store.getState().workspaces["ws-1"]?.[0]?.id;

    expect(firstId).toBeTruthy();
    store.getState().touch("ws-1", firstId!);
    expect(store.getState().workspaces["ws-1"]?.[0]?.useCount).toBe(2);

    store.getState().remove("ws-1", firstId!);
    expect(store.getState().workspaces["ws-1"]).toHaveLength(1);
    expect(store.getState().workspaces["ws-1"]?.[0]?.text).toBe("Summarize this PDF");

    store.getState().clear("ws-1");
    expect(store.getState().workspaces["ws-1"]).toEqual([]);
  });

  it("rehydrates persisted state when store is recreated", async () => {
    const name = "query-library-test-rehydrate";
    window.localStorage.removeItem(name);

    const store1 = createQueryLibraryStore({ name });
    store1.getState().capture("ws-1", "Summarize this PDF");
    await waitForQueryLibraryHydration(store1);

    const store2 = createQueryLibraryStore({ name });
    await waitForQueryLibraryHydration(store2);

    expect(store2.getState().workspaces["ws-1"]?.[0]?.text).toBe("Summarize this PDF");
  });

  it("filters invalid items when reading workspace items", () => {
    window.localStorage.removeItem(QUERY_LIBRARY_STORAGE_KEY);
    queryLibraryStore.setState({
      workspaces: {
        "ws-1": [
          {
            id: "valid",
            text: "Valid query",
            createdAt: 1,
            lastUsedAt: 2,
            useCount: 1,
          },
          {
            id: "",
            text: "bad",
            createdAt: 1,
            lastUsedAt: 2,
            useCount: 1,
          },
        ],
      },
    });

    expect(getQueryLibraryItems("ws-1")).toHaveLength(1);
    expect(getQueryLibraryItems("ws-1")[0]?.text).toBe("Valid query");
  });

  it("filters invalid persisted entries on read", () => {
    const store = createQueryLibraryStore({ name: "query-library-test-normalize" });

    store.setState({
      workspaces: {
        "ws-1": [
          {
            id: "valid",
            text: "Valid query",
            createdAt: 1,
            lastUsedAt: 2,
            useCount: 1,
          },
          {
            id: "",
            text: "bad",
            createdAt: 1,
            lastUsedAt: 2,
            useCount: 1,
          },
          {
            id: "empty",
            text: "   ",
            createdAt: 1,
            lastUsedAt: 2,
            useCount: 1,
          },
        ],
      },
    });

    store.getState().capture("ws-1", "Another query");

    expect(store.getState().workspaces["ws-1"]).toHaveLength(2);
    expect(store.getState().workspaces["ws-1"]?.map((item) => item.text)).toEqual([
      "Another query",
      "Valid query",
    ]);
  });
});
