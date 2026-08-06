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
  loadStoredCapabilities,
  storeCapabilities,
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
  return parts.join(" · ");
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
  const [capabilities, setCapabilities] = useState<WorkspaceCapability[]>(() =>
    loadStoredCapabilities(workspaceId),
  );
  const textareaRef = useRef<HTMLTextAreaElement | null>(null);
  const pendingCursorRef = useRef<number | null>(null);

  // Restore per-workspace capability toggles after refresh / workspace switch.
  useEffect(() => {
    setCapabilities(loadStoredCapabilities(workspaceId));
  }, [workspaceId]);

  // RAG requires an explicit source selection: strip it when the selection
  // becomes empty (product rule 2026-07-18 — no implicit whole-workspace scope).
  useEffect(() => {
    if (selectedSourceIds.length > 0 || !capabilities.includes("rag")) {
      return;
    }
    const next = capabilities.filter((cap) => cap !== "rag");
    setCapabilities(next);
    storeCapabilities(workspaceId, next);
  }, [selectedSourceIds, capabilities, workspaceId]);

  const handleCapabilitiesChange = useCallback(
    (next: WorkspaceCapability[]) => {
      setCapabilities(next);
      storeCapabilities(workspaceId, next);
    },
    [workspaceId],
  );

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

  // Empty thread: lift the composer into a centered hero layout (Grok-style).
  const showComposerHero = chatSession.messages.length === 0 && !chatSession.progress.mode;
  const composerHero = showComposerHero ? (
    <div className={styles.heroBlock} data-testid="workspace-chat-empty">
      <h1 className={styles.heroTitle}>
        {formatUiMessage(locale, "workspaceChatHeroTitle")}
      </h1>
      <p className={styles.heroSubtitle}>
        {formatUiMessage(locale, "workspaceChatHeroSubtitle")}
      </p>
      <p className={styles.heroModeHint}>
        {formatUiMessage(locale, "workspaceEmptyStateModeHint", {
          mode: activeModeLabel,
        })}
      </p>
    </div>
  ) : null;

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
      data-testid="workspace-chat-pane"
      data-active-mode={activeModeCode}
    >
      {/* W5 #18: mode title/chip header removed — composer capability toggles remain. */}
      {chatSession.error && (
        <p className={styles.error} role="alert">
          {chatSession.error}
        </p>
      )}

      <ChatMessageList
        key={sessionId ?? "new-thread"}
        messages={chatSession.messages}
        progress={chatSession.progress}
        isStreaming={chatSession.isStreaming}
        locale={locale}
        onSelectCitation={onSelectCitation ?? (() => {})}
        onOpenWebSources={onOpenWebSources ?? (() => {})}
        onCopyMessage={handleCopyMessage}
        onEditMessage={handleEditMessage}
        onSubmitFeedback={handleSubmitFeedback}
        onToggleProgressCollapsed={chatSession.toggleProgressCollapsed}
      />

      <ChatComposer
        draft={draft}
        onDraftChange={setDraft}
        isStreaming={chatSession.isStreaming}
        capabilities={capabilities}
        locale={locale}
        workspaceId={workspaceId}
        ragDisabled={selectedSourceIds.length === 0}
        onSubmit={handleSend}
        onStop={chatSession.stop}
        onCapabilitiesChange={handleCapabilitiesChange}
        textareaRef={textareaRef}
        onHeightChange={setComposerClearance}
        hero={composerHero}
      />
    </section>
  );
}
