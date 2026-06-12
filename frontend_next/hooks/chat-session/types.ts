import type { WorkspaceChatMode } from "../../lib/workspace/ui-store";
import type {
  AnswerBlock,
  Citation,
  DegradeTraceItem,
  ProgressSourcePreview,
  ToolResult,
  WorkspaceChatStreamEvent,
} from "../../lib/workspace/stream";

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
};

export type ProgressEntry = {
  id: string;
  phase: string;
  title: string;
  detail: string | null;
  counts: Record<string, number>;
  sourcesPreview: ProgressSourcePreview[];
  timestamp: string | null;
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
  };
  error: string | null;
  send: (query: string) => void;
  stop: () => void;
  toggleProgressCollapsed: () => void;
};

export type PendingDoneEvent = Extract<WorkspaceChatStreamEvent, { kind: "done" }>;
