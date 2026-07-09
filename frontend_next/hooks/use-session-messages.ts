import { useCallback, useRef } from "react";

import {
  listWorkspaceSessionMessages,
  type WorkspaceChatMessage,
} from "../lib/workspace/client";

/**
 * Deduplicated session-message fetcher.
 *
 * `WorkspaceHistoryPane` derives both session titles and search documents from
 * the same per-session message list. Without dedup, a session that lacks a
 * title AND is open in search triggers two concurrent
 * `listWorkspaceSessionMessages` requests. This hook collapses concurrent
 * requests for the same session into a single network call via an in-flight
 * promise map. Once a request settles the entry is cleared so legitimate
 * refetches (e.g. on `updated_at` change) still work.
 */
export function useSessionMessages(token: string | null) {
  const inFlightRef = useRef<Map<string, Promise<WorkspaceChatMessage[]>>>(new Map());

  return useCallback(
    async function fetchSessionMessages(sessionId: string): Promise<WorkspaceChatMessage[]> {
      const existing = inFlightRef.current.get(sessionId);
      if (existing) {
        return existing;
      }

      if (!token) {
        return [];
      }

      const promise = listWorkspaceSessionMessages(token, sessionId)
        .then((response) => response.messages)
        .finally(() => {
          inFlightRef.current.delete(sessionId);
        });

      inFlightRef.current.set(sessionId, promise);
      return promise;
    },
    [token],
  );
}
