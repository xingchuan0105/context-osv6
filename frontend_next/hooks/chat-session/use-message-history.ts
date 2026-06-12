"use client";

import { useCallback, useState } from "react";
import { listWorkspaceSessionMessages } from "../../lib/workspace/client";
import { mapTranscriptMessage } from "./helpers";
import type { UiChatMessage } from "./types";

export function useMessageHistory(token: string, locale: "zh-CN" | "en") {
  const [messages, setMessages] = useState<UiChatMessage[]>([]);

  const loadSession = useCallback(
    async (sessionId: string) => {
      if (!token || !sessionId) {
        setMessages([]);
        return;
      }
      const response = await listWorkspaceSessionMessages(token, sessionId);
      setMessages(response.messages.map(mapTranscriptMessage));
    },
    [token, locale],
  );

  const reset = useCallback(() => {
    setMessages([]);
  }, []);

  return { messages, setMessages, loadSession, reset };
}

export type MessageHistory = ReturnType<typeof useMessageHistory>;
