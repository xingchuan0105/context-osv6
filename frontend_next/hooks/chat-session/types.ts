import type { WorkspaceChatMode } from "../../lib/workspace/ui-store";
import type { ChatEvent, ChatResponse } from "../../lib/contracts";
import type {
  AnswerBlock,
  Citation,
  DegradeTraceItem,
  ProgressSourcePreview,
  ToolResult,
} from "../../lib/workspace/stream";

/** Frozen process card for a finished assistant turn (survives refresh via local cache). */
export type UiProgressSnapshot = {
  mode: WorkspaceChatMode;
  activities: ProgressEntry[];
  startedAtMs: number | null;
  endedAtMs: number | null;
  collapsed: boolean;
};

export type UiChatMessage = {
  id: string;
  role: "user" | "assistant";
  mode: WorkspaceChatMode | null;
  content: string;
  answerBlocks: AnswerBlock[];
  citations: Citation[];
  degradeTrace: DegradeTraceItem[];
  guarded: boolean;
  messageId: number | null;
  pending?: boolean;
  sessionId: string | null;
  toolResults: ToolResult[];
  /** Per-turn retrieval/process card (attached on stream done; restored after refresh). */
  progress?: UiProgressSnapshot | null;
};

export type ProgressEntry = {
  id: string;
  phase: string;
  title: string;
  detail: string | null;
  counts: Record<string, number>;
  sourcesPreview: ProgressSourcePreview[];
  timestamp: string | null;
  /** Client wall-clock when this step was added (for Grok-style elapsed). */
  startedAtMs?: number;
};

export type UseChatSessionOptions = {
  token: string;
  workspaceId: string;
  sessionId: string | null;
  selectedSourceIds: string[];
  effectiveChatMode: WorkspaceChatMode;
  locale: "zh-CN" | "en";
  onSessionChange?: (sessionId: string | null) => void;
  onSessionActivity?: () => void;
};

export type UseChatSessionResult = {
  messages: UiChatMessage[];
  isStreaming: boolean;
  progress: {
    activities: ProgressEntry[];
    mode: WorkspaceChatMode | null;
    collapsed: boolean;
    /** Stream start time (ms) for live elapsed in the process header. */
    startedAtMs: number | null;
    /** When set, process is finalized (timer frozen, collapsed summary). */
    endedAtMs: number | null;
  };
  error: string | null;
  send: (query: string) => void;
  stop: () => void;
  toggleProgressCollapsed: () => void;
};

export type PendingDoneEvent = Extract<ChatEvent, { event: "done" }> & {
  payload: ChatResponse;
};
