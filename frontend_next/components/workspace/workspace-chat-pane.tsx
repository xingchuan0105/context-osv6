"use client";

import { type CSSProperties, useCallback, useEffect, useLayoutEffect, useRef, useState } from "react";
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
  deriveAgentTypeLabel,
  type WorkspaceCapability,
} from "../../lib/workspace/capabilities";
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
  registerComposerInsert?: (handler: ((text: string) => boolean) | null) => void;
};

function getCapabilitiesSummaryLabel(
  locale: "zh-CN" | "en",
  capabilities: WorkspaceCapability[],
) {
  if (capabilities.length === 0) {
    return formatUiMessage(locale, "workspaceChatModeChat");
  }
  const parts = capabilities.map((cap) =>
    cap === "rag"
      ? formatUiMessage(locale, "workspaceChatCapRag")
      : formatUiMessage(locale, "workspaceChatCapSearch"),
  );
  return parts.join(locale === "zh-CN" ? " · " : " · ");
}

function getCapabilitiesCode(capabilities: WorkspaceCapability[]) {
  return deriveAgentTypeLabel(capabilities);
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
  registerComposerInsert,
}: WorkspaceChatPaneProps) {
  const auth = useAuth();
  const { locale } = useUiPreferences();
  const [draft, setDraft] = useState("");
  const [composerClearance, setComposerClearance] = useState<number | null>(null);
  const [capabilities, setCapabilities] = useState<WorkspaceCapability[]>([]);
  const textareaRef = useRef<HTMLTextAreaElement | null>(null);
  const pendingCursorRef = useRef<number | null>(null);
  const lastEstablishedSessionRef = useRef<string | null>(null);

  // Session-only capabilities: reset on new thread or switch between sessions.
  // Do not wipe when backend assigns session_id on first send (null → id).
  useEffect(() => {
    if (sessionId === null) {
      setCapabilities([]);
      lastEstablishedSessionRef.current = null;
      return;
    }
    if (
      lastEstablishedSessionRef.current != null &&
      lastEstablishedSessionRef.current !== sessionId
    ) {
      setCapabilities([]);
    }
    lastEstablishedSessionRef.current = sessionId;
  }, [sessionId]);

  const activeModeLabel = getCapabilitiesSummaryLabel(locale, capabilities);
  const activeModeCode = getCapabilitiesCode(capabilities);

  const chatSession = useChatSession({
    token: auth.token || "",
    workspaceId,
    sessionId,
    selectedSourceIds,
    capabilities,
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

  const insertIntoComposer = useCallback(
    (text: string): boolean => {
      if (chatSession.isStreaming) {
        return false;
      }

      setDraft((currentDraft) => {
        const textarea = textareaRef.current;
        const start = textarea?.selectionStart ?? currentDraft.length;
        const end = textarea?.selectionEnd ?? currentDraft.length;
        const nextDraft = `${currentDraft.slice(0, start)}${text}${currentDraft.slice(end)}`;
        pendingCursorRef.current = start + text.length;
        return nextDraft;
      });

      return true;
    },
    [chatSession.isStreaming],
  );

  useLayoutEffect(() => {
    if (pendingCursorRef.current === null) {
      return;
    }

    const textarea = textareaRef.current;
    if (!textarea) {
      pendingCursorRef.current = null;
      return;
    }

    const nextCursor = pendingCursorRef.current;
    pendingCursorRef.current = null;
    textarea.setSelectionRange(nextCursor, nextCursor);
    textarea.focus();
  }, [draft]);

  useEffect(() => {
    if (!registerComposerInsert) {
      return;
    }

    registerComposerInsert(insertIntoComposer);
    return () => registerComposerInsert(null);
  }, [insertIntoComposer, registerComposerInsert]);

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

      {chatSession.error && (
        <p className={styles.error} role="alert">
          {chatSession.error}
        </p>
      )}

      <ChatMessageList
        messages={chatSession.messages}
        progress={chatSession.progress}
        isStreaming={chatSession.isStreaming}
        locale={locale}
        activeModeLabel={activeModeLabel}
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
        capabilities={capabilities}
        locale={locale}
        workspaceId={workspaceId}
        onSubmit={handleSend}
        onStop={chatSession.stop}
        onCapabilitiesChange={setCapabilities}
        textareaRef={textareaRef}
        onHeightChange={setComposerClearance}
      />
    </section>
  );
}
