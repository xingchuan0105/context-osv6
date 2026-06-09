"use client";

import { type CSSProperties, useCallback, useRef, useState } from "react";
import { useAuth } from "../../lib/auth/context";
import { formatUiMessage } from "../../lib/i18n/messages";
import { useUiPreferences } from "../../lib/ui-preferences";
import {
  submitWorkspaceMessageFeedback,
} from "../../lib/workspace/client";
import type {
  WorkspaceCitationRequest,
  WorkspaceWebSourcesRequest,
} from "../../lib/workspace/model";
import {
  resolveWorkspaceChatMode,
  useWorkspaceUi,
  type WorkspaceChatMode,
} from "../../lib/workspace/ui-store";
import { useChatSession } from "../../hooks/use-chat-session";
import { ChatComposer } from "./chat-composer";
import { ChatMessageList } from "./chat-message-list";
import styles from "./workspace-chat.module.css";

type WorkspaceChatPaneProps = {
  workspaceId: string;
  sessionId: string | null;
  selectedSourceIds: string[];
  onSessionActivity?: () => void;
  onSessionChange?: (sessionId: string | null) => void;
  onFocusSource?: (sourceId: string | null) => void;
  onSelectCitation?: (request: WorkspaceCitationRequest) => void;
  onOpenWebSources?: (request: WorkspaceWebSourcesRequest) => void;
};

function getModeLabel(locale: "zh-CN" | "en", mode: WorkspaceChatMode) {
  switch (mode) {
    case "rag":
      return formatUiMessage(locale, "workspaceChatModeRag");
    case "search":
      return formatUiMessage(locale, "workspaceChatModeSearch");
    case "chat":
    default:
      return formatUiMessage(locale, "workspaceChatModeChat");
  }
}

function getModeCode(mode: WorkspaceChatMode) {
  switch (mode) {
    case "rag":
      return "RAG";
    case "search":
      return "web_search";
    case "chat":
    default:
      return "chat";
  }
}

export function WorkspaceChatPane({
  workspaceId,
  sessionId,
  selectedSourceIds,
  onSessionActivity,
  onSessionChange,
  onFocusSource: _onFocusSource,
  onSelectCitation,
  onOpenWebSources,
}: WorkspaceChatPaneProps) {
  const auth = useAuth();
  const { locale } = useUiPreferences();
  const workspaceUi = useWorkspaceUi(workspaceId);
  const { setChatMode } = workspaceUi;
  const [draft, setDraft] = useState("");
  const [composerClearance, setComposerClearance] = useState<number | null>(null);
  const textareaRef = useRef<HTMLTextAreaElement | null>(null);

  const effectiveChatMode = resolveWorkspaceChatMode(workspaceUi, selectedSourceIds.length > 0);
  const activeModeLabel = getModeLabel(locale, effectiveChatMode);
  const activeModeCode = getModeCode(effectiveChatMode);

  const chatSession = useChatSession({
    token: auth.token || "",
    workspaceId,
    sessionId,
    selectedSourceIds,
    effectiveChatMode,
    locale,
    onSessionChange,
    onSessionActivity,
  });

  const shellStyle: CSSProperties | undefined =
    composerClearance !== null
      ? { "--workspace-chat-bottom-clearance": `${composerClearance}px` } as CSSProperties
      : undefined;

  const handleCopyMessage = useCallback((content: string) => {
    if (typeof navigator === "undefined" || !navigator.clipboard) {
      return;
    }
    void navigator.clipboard.writeText(content);
  }, []);

  const handleEditMessage = useCallback((content: string) => {
    setDraft(content);
    textareaRef.current?.focus();
  }, []);

  const handleSubmitFeedback = useCallback(
    async (messageId: string, rating: "up" | "down") => {
      const message = chatSession.messages.find((m) => m.id === messageId);
      if (!auth.token || !message?.sessionId || message.messageId === null) {
        return;
      }
      try {
        await submitWorkspaceMessageFeedback(auth.token, {
          session_id: message.sessionId,
          message_id: message.messageId,
          rating,
        });
      } catch {
        // Silently fail — feedback is best-effort
      }
    },
    [auth.token, chatSession.messages],
  );

  const handleSend = useCallback(() => {
    chatSession.send(draft);
    setDraft("");
  }, [chatSession, draft]);

  return (
    <section
      className={styles.shell}
      style={shellStyle}
      aria-label={formatUiMessage(locale, "workspaceChatRegionLabel")}
    >
      <header className={styles.header}>
        <div className={styles.titleBlock}>
          <h2 className={styles.title}>{activeModeLabel}</h2>
        </div>
        <span className={styles.modeChip}>{activeModeCode}</span>
      </header>

      <ChatMessageList
        messages={chatSession.messages}
        progress={chatSession.progress}
        isStreaming={chatSession.isStreaming}
        locale={locale}
        onToggleProgressCollapsed={chatSession.toggleProgressCollapsed}
        onSelectCitation={onSelectCitation ?? (() => {})}
        onOpenWebSources={onOpenWebSources ?? (() => {})}
        onCopyMessage={handleCopyMessage}
        onEditMessage={handleEditMessage}
        onSubmitFeedback={handleSubmitFeedback}
      />

      <ChatComposer
        draft={draft}
        onDraftChange={setDraft}
        isStreaming={chatSession.isStreaming}
        effectiveChatMode={effectiveChatMode}
        locale={locale}
        workspaceId={workspaceId}
        onSubmit={handleSend}
        onStop={chatSession.stop}
        onModeChange={setChatMode}
        textareaRef={textareaRef}
        onHeightChange={setComposerClearance}
      />
    </section>
  );
}
