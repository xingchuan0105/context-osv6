"use client";

import {
  type CSSProperties,
  useCallback,
  useEffect,
  useLayoutEffect,
  useMemo,
  useRef,
  useState,
} from "react";
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
import {
  getWorkspaceUiState,
  useWorkspaceUiState,
  workspaceUiStore,
} from "../../lib/workspace/ui-store";
import { useChatSession } from "../../hooks/use-chat-session";
import type { UiChatMessage } from "../../hooks/chat-session/types";
import { ChatComposer } from "../workspace/chat-composer";
import { ChatMessageList } from "../workspace/chat-message-list";
import { SessionFileTray } from "./session-file-tray";
import styles from "../workspace/workspace-chat.module.css";

type ChatCanvasProps = {
  workspaceId: string | null;
  sessionId: string | null;
  selectedSourceIds: string[];
  onSessionActivity?: () => void;
  onSessionChange?: (sessionId: string | null) => void;
  onFocusSource?: (sourceId: string | null) => void;
  onSelectCitation?: (request: WorkspaceCitationRequest) => void;
  onOpenWebSources?: (request: WorkspaceWebSourcesRequest) => void;
  registerComposerInsert?: (handler: ((text: string) => boolean) | null) => void;
  /**
   * Shared KB visitor surface: source_type=share, default RAG locked (no chat mode, no search).
   */
  shareToken?: string | null;
  turnstileToken?: string | null;
  /** Local transcript seed for share sessions (not loaded from PG). */
  initialMessages?: import("../../hooks/chat-session/types").UiChatMessage[] | null;
  onTranscriptChange?: (
    messages: import("../../hooks/chat-session/types").UiChatMessage[],
  ) => void;
  /** Fixed capabilities; omit to use normal per-workspace toggles. */
  lockedCapabilities?: WorkspaceCapability[];
  /** Capability controls available on this surface; personal chat exposes search in W1. */
  availableCapabilities?: WorkspaceCapability[];
  /** Explicitly clears an uncommitted/new conversation without remounting the live stream. */
  resetEpoch?: number;
  /** Lets the owning shell disable navigation while a turn is in flight. */
  onStreamingChange?: (isStreaming: boolean) => void;
  /** Open the right rail (guide users to select sources when rag chip is inert). */
  onRequestGuideSources?: () => void;
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

export function ChatCanvas({
  workspaceId,
  sessionId,
  selectedSourceIds,
  onSessionActivity,
  onSessionChange,
  onFocusSource: _onFocusSource,
  onSelectCitation,
  onOpenWebSources,
  registerComposerInsert,
  shareToken = null,
  turnstileToken = null,
  initialMessages = null,
  onTranscriptChange,
  lockedCapabilities,
  availableCapabilities,
  resetEpoch = 0,
  onStreamingChange,
  onRequestGuideSources,
}: ChatCanvasProps) {
  const auth = useAuth();
  const { locale } = useUiPreferences();
  const [draft, setDraft] = useState("");
  const [composerClearance, setComposerClearance] = useState<number | null>(null);
  const [sessionFilesBlocked, setSessionFilesBlocked] = useState(false);
  const [sessionReadyFiles, setSessionReadyFiles] = useState(0);
  const isShareMode = Boolean(shareToken?.trim());
  const isPersonalConversation = workspaceId === null;
  const uiScopeId = workspaceId ?? `chat:${sessionId ?? "new"}`;
  const previousUiScopeRef = useRef(uiScopeId);
  const fixedCaps = useMemo<WorkspaceCapability[] | null>(() => {
    if (lockedCapabilities && lockedCapabilities.length > 0) {
      return [...lockedCapabilities];
    }
    if (isShareMode) {
      return ["rag"];
    }
    return null;
  }, [isShareMode, lockedCapabilities]);
  const supportsRag = fixedCaps
    ? fixedCaps.includes("rag")
    : !availableCapabilities || availableCapabilities.includes("rag");
  const storeCapabilities = useWorkspaceUiState(uiScopeId, (state) => state.capabilities);
  const capabilities = useMemo(
    () =>
      fixedCaps ??
      (availableCapabilities
        ? storeCapabilities.filter((capability) => availableCapabilities.includes(capability))
        : storeCapabilities),
    [availableCapabilities, fixedCaps, storeCapabilities],
  );
  const textareaRef = useRef<HTMLTextAreaElement | null>(null);
  const pendingCursorRef = useRef<number | null>(null);

  useEffect(() => {
    const previousScopeId = previousUiScopeRef.current;
    previousUiScopeRef.current = uiScopeId;

    if (!isPersonalConversation) {
      return;
    }

    if (uiScopeId === "chat:new") {
      workspaceUiStore.getState().resetWorkspace(uiScopeId);
      return;
    }

    if (previousScopeId !== "chat:new") {
      return;
    }

    const previous = getWorkspaceUiState(previousScopeId);
    const current = getWorkspaceUiState(uiScopeId);
    if (!current.capabilitiesManual && current.capabilities.length === 0) {
      workspaceUiStore
        .getState()
        .setCapabilities(uiScopeId, previous.capabilities, {
          manual: previous.capabilitiesManual,
        });
    }
    workspaceUiStore.getState().resetWorkspace(previousScopeId);
  }, [isPersonalConversation, resetEpoch, uiScopeId]);

  const restorePersonalCapabilities = useCallback(
    (messages: UiChatMessage[]) => {
      if (!isPersonalConversation) {
        return;
      }
      const current = getWorkspaceUiState(uiScopeId);
      if (current.capabilitiesManual) {
        return;
      }
      const lastAssistant = [...messages]
        .reverse()
        .find((message) => message.role === "assistant" && !message.pending);
      const restored = (lastAssistant?.capabilities ?? []).filter(
        (capability): capability is WorkspaceCapability => capability === "search",
      );
      workspaceUiStore
        .getState()
        .setCapabilities(uiScopeId, restored, { manual: false });
    },
    [isPersonalConversation, uiScopeId],
  );

  // Source selection auto-attaches knowledge retrieval (2026-08-30 foolproofing):
  // whenever this workspace is first observed with selected sources (user just
  // picked one, or the pane renders / switches in with a persisted selection)
  // and the user has not touched the chips manually, rag is on so the next
  // question is already grounded.
  const previousSelectionRef = useRef<{ scopeId: string; count: number } | null>(null);
  useEffect(() => {
    if (fixedCaps || isShareMode) {
      return;
    }
    const previous = previousSelectionRef.current;
    previousSelectionRef.current = { scopeId: uiScopeId, count: selectedSourceIds.length };
    if (selectedSourceIds.length === 0) {
      return;
    }
    if (previous !== null && previous.scopeId === uiScopeId && previous.count > 0) {
      // Selection changed while sources stay selected — chip state already settled.
      return;
    }
    const current = getWorkspaceUiState(uiScopeId);
    if (current.capabilitiesManual || current.capabilities.includes("rag")) {
      return;
    }
    workspaceUiStore
      .getState()
      .setCapabilities(uiScopeId, [...current.capabilities, "rag"], { manual: false });
  }, [selectedSourceIds, uiScopeId, fixedCaps, isShareMode]);

  // RAG requires an explicit source selection: strip it when the selection
  // becomes empty (product rule 2026-07-18 — no implicit whole-workspace scope).
  // Shared KB uses whole-workspace RAG by default — never strip.
  // Personal conversations legitimize RAG via ready session files: strip when
  // the last ready file goes away (review round-3 P1 — otherwise the next turn
  // would send an empty scope and fail with docscope_required), but only when
  // the chip was auto-attached rather than user-selected.
  useEffect(() => {
    if (fixedCaps || isShareMode) {
      return;
    }
    if (isPersonalConversation) {
      const current = getWorkspaceUiState(uiScopeId);
      if (sessionReadyFiles === 0 && capabilities.includes("rag") && !current.capabilitiesManual) {
        workspaceUiStore
          .getState()
          .setCapabilities(
            uiScopeId,
            capabilities.filter((cap) => cap !== "rag"),
            { manual: false },
          );
      }
      return;
    }
    if (selectedSourceIds.length > 0 || !capabilities.includes("rag")) {
      return;
    }
    workspaceUiStore.getState().setCapabilities(
      uiScopeId,
      capabilities.filter((cap) => cap !== "rag"),
      { manual: false },
    );
  }, [
    selectedSourceIds,
    capabilities,
    uiScopeId,
    fixedCaps,
    isShareMode,
    isPersonalConversation,
    sessionReadyFiles,
  ]);

  // Session files ready → retrieval joins by default (server derives the doc
  // scope from bindings; this only flips the capability chip like the
  // workspace source-selection foolproofing above).
  const previousReadyRef = useRef<{ scopeId: string; count: number } | null>(null);
  useEffect(() => {
    if (fixedCaps || isShareMode || !isPersonalConversation) {
      return;
    }
    const previous = previousReadyRef.current;
    previousReadyRef.current = { scopeId: uiScopeId, count: sessionReadyFiles };
    // Attach on the 0→ready transition of this session's files (and when a
    // ready-file session is first observed); skip no-op re-renders.
    if (
      sessionReadyFiles === 0 ||
      (previous !== null &&
        previous.scopeId === uiScopeId &&
        previous.count === sessionReadyFiles)
    ) {
      return;
    }
    const current = getWorkspaceUiState(uiScopeId);
    if (current.capabilitiesManual || current.capabilities.includes("rag")) {
      return;
    }
    workspaceUiStore
      .getState()
      .setCapabilities(uiScopeId, [...current.capabilities, "rag"], { manual: false });
  }, [sessionReadyFiles, uiScopeId, fixedCaps, isShareMode, isPersonalConversation]);

  const handleCapabilitiesChange = useCallback(
    (next: WorkspaceCapability[]) => {
      if (fixedCaps) {
        return;
      }
      workspaceUiStore.getState().setCapabilities(uiScopeId, next, { manual: true });
    },
    [uiScopeId, fixedCaps],
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
    onHistoryHydrated: restorePersonalCapabilities,
    shareToken,
    turnstileToken,
    initialMessages,
    onTranscriptChange,
  });

  useEffect(() => {
    onStreamingChange?.(chatSession.isStreaming);
  }, [chatSession.isStreaming, onStreamingChange]);

  useEffect(
    () => () => {
      onStreamingChange?.(false);
    },
    [onStreamingChange],
  );

  useEffect(() => {
    setDraft("");
  }, [resetEpoch, sessionId, uiScopeId]);

  // Keep local share transcripts in sync after stream settles.
  useEffect(() => {
    if (!isShareMode || chatSession.isStreaming) {
      return;
    }
    onTranscriptChange?.(chatSession.messages);
  }, [isShareMode, chatSession.isStreaming, chatSession.messages, onTranscriptChange]);

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
        {formatUiMessage(
          locale,
          isPersonalConversation ? "chat.heroSubtitle" : "workspaceChatHeroSubtitle",
        )}
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
    if (sessionFilesBlocked) {
      return;
    }
    chatSession.send(draft);
    setDraft("");
  }, [chatSession, draft, sessionFilesBlocked]);

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
      aria-label={formatUiMessage(
        locale,
        isPersonalConversation ? "chat.regionLabel" : "workspaceChatRegionLabel",
      )}
      data-testid="chat-canvas"
      data-active-mode={activeModeCode}
    >
      {/* W5 #18: mode title/chip header removed — composer capability toggles remain. */}
      {chatSession.error && (
        <p className={styles.error} role="alert">
          {chatSession.error}
        </p>
      )}

      {/* Empty thread: hero composer fills the pane — skip the empty transcript
          so its flex share doesn't push the hero down. */}
      {!showComposerHero && (
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
      )}

      {isPersonalConversation && !isShareMode && (
        <SessionFileTray
          disabled={chatSession.isStreaming}
          onBlockedChange={setSessionFilesBlocked}
          onReadyCountChange={setSessionReadyFiles}
          onSessionChange={(id) => onSessionChange?.(id)}
          sessionId={sessionId}
          token={auth.token || ""}
        />
      )}

      <ChatComposer
        draft={draft}
        onDraftChange={setDraft}
        isStreaming={chatSession.isStreaming}
        disabled={
          chatSession.isHydrating ||
          chatSession.historyLoadFailed ||
          sessionFilesBlocked
        }
        capabilities={capabilities}
        locale={locale}
        composerId={uiScopeId}
        composerLabel={formatUiMessage(
          locale,
          isPersonalConversation ? "chat.composerLabel" : "workspaceChatComposerLabel",
        )}
        ragDisabled={supportsRag && !isShareMode && selectedSourceIds.length === 0}
        selectedSourceCount={selectedSourceIds.length}
        onRequestGuideSources={onRequestGuideSources}
        onSubmit={handleSend}
        onStop={chatSession.stop}
        onCapabilitiesChange={handleCapabilitiesChange}
        textareaRef={textareaRef}
        onHeightChange={setComposerClearance}
        hero={composerHero}
        availableCapabilities={fixedCaps ?? availableCapabilities}
        lockCapabilities={Boolean(fixedCaps)}
      />
    </section>
  );
}
