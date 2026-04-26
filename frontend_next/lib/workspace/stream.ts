import { ApiError, buildApiUrl } from "../auth/client";

export type ChatTurnInput = {
  role: string;
  content: string;
};

export type AnswerBlock =
  | {
      type: "text";
      text: string;
      citations: string[];
    }
  | {
      type: "image";
      chunk_id: string;
    };

export type CitationSourceLocator = {
  url?: string | null;
  citation_index?: number | null;
};

export type Citation = {
  citation_id: number;
  doc_id: string;
  chunk_id?: string | null;
  page?: number | null;
  doc_name: string;
  preview?: string | null;
  content?: string | null;
  score: number;
  layer?: string | null;
  chunk_type?: string | null;
  asset_id?: string | null;
  caption?: string | null;
  image_url?: string | null;
  source_locator?: CitationSourceLocator | null;
};

export type SourceRef = {
  id: string;
  title: string;
  snippet?: string | null;
  doc_id?: string | null;
  page?: number | null;
};

export type ProgressSourcePreview = {
  id: string;
  label: string;
  href?: string | null;
};

export type TraceInfo = {
  mode: string;
};

export type DegradeTraceItem = {
  stage: string;
  reason: string;
  impact: string;
};

export type ChatMessage = {
  id: number;
  session_id: string;
  role: string;
  content: string;
  answer_blocks: AnswerBlock[];
  agent_id?: string | null;
  agent_name?: string | null;
  agent_icon?: string | null;
  citations: Citation[];
  created_at: string;
};

export type ChatRequest = {
  query: string;
  notebook_id?: string | null;
  session_id?: string | null;
  agent_type?: string;
  source_type?: string | null;
  source_token?: string | null;
  doc_scope?: string[];
  messages?: ChatTurnInput[];
  stream?: boolean;
};

export type ChatResponse = {
  answer: string;
  answer_blocks: AnswerBlock[];
  session_id: string;
  agent_type: string;
  sources: SourceRef[];
  citations: Citation[];
  trace: TraceInfo;
  degrade_trace: DegradeTraceItem[];
  planner_output?: unknown | null;
  mode_debug?: unknown | null;
  message_id?: number | null;
  guard_report?: unknown | null;
};

export type ChatDonePayload = {
  request_id: string;
  session_id: string;
  message_id: number;
  response: ChatResponse;
};

export type ChatEvent =
  | {
      event: "start";
      request_id: string;
      session_id: string;
    }
  | {
      event: "activity";
      request_id: string;
      phase: string;
      title: string;
      detail?: string | null;
      counts?: Record<string, number>;
      sources_preview?: ProgressSourcePreview[];
      timestamp?: string | null;
    }
  | {
      event: "answer_start";
      request_id: string;
      session_id: string;
      message_id: number;
      agent_type: string;
    }
  | {
      event: "trace";
      request_id: string;
      stage: string;
      status: string;
      detail?: unknown | null;
    }
  | {
      event: "token";
      request_id: string;
      message_id: number;
      content: string;
    }
  | {
      event: "citations";
      request_id: string;
      message_id: number;
      citations: Citation[];
    }
  | {
      event: "done";
      request_id: string;
      session_id: string;
      message_id: number;
      payload: ChatResponse;
    }
  | {
      event: "error";
      request_id: string;
      code: string;
      message: string;
    };

export type WorkspaceChatStreamEvent =
  | {
      kind: "start";
      request_id: string;
      session_id: string;
    }
  | {
      kind: "activity";
      request_id: string;
      phase: string;
      title: string;
      detail?: string | null;
      counts: Record<string, number>;
      sources_preview: ProgressSourcePreview[];
      timestamp?: string | null;
    }
  | {
      kind: "answer_start";
      request_id: string;
      session_id: string;
      message_id: number;
      agent_type: string;
    }
  | {
      kind: "trace";
      request_id: string;
      stage: string;
      status: string;
      detail?: unknown | null;
    }
  | {
      kind: "token";
      request_id: string;
      message_id: number;
      content: string;
    }
  | {
      kind: "citations";
      request_id: string;
      message_id: number;
      citations: Citation[];
    }
  | {
      kind: "done";
      request_id: string;
      session_id: string;
      message_id: number;
      payload: ChatResponse;
    }
  | {
      kind: "error";
      request_id: string;
      code: string;
      message: string;
    };

async function decodeError(response: Response) {
  const raw = await response.text();

  if (!raw.trim()) {
    return new ApiError(response.status, null, `Request failed with status ${response.status}`);
  }

  try {
    const parsed = JSON.parse(raw) as { error?: string | null; message?: string };
    return new ApiError(response.status, parsed.error ?? null, parsed.message ?? raw);
  } catch {
    return new ApiError(response.status, null, raw);
  }
}

function decodeChatEvent(eventName: string, dataText: string): WorkspaceChatStreamEvent | null {
  if (!eventName || !dataText.trim()) {
    return null;
  }

  let parsed: unknown;

  try {
    parsed = JSON.parse(dataText);
  } catch {
    return null;
  }

  if (typeof parsed !== "object" || parsed === null) {
    return null;
  }

  const raw = parsed as Record<string, unknown>;

  if (typeof raw.event === "string" && raw.event !== eventName) {
    return null;
  }

  switch (eventName) {
    case "start":
      return {
        kind: "start",
        request_id: String(raw.request_id ?? ""),
        session_id: String(raw.session_id ?? ""),
      };
    case "activity":
      return {
        kind: "activity",
        request_id: String(raw.request_id ?? ""),
        phase: String(raw.phase ?? ""),
        title: String(raw.title ?? ""),
        detail: raw.detail == null ? null : String(raw.detail),
        counts:
          typeof raw.counts === "object" && raw.counts !== null
            ? Object.fromEntries(
                Object.entries(raw.counts as Record<string, unknown>).map(([key, value]) => [
                  key,
                  Number(value ?? 0),
                ]),
              )
            : {},
        sources_preview: Array.isArray(raw.sources_preview)
          ? (raw.sources_preview as ProgressSourcePreview[])
          : [],
        timestamp: raw.timestamp == null ? null : String(raw.timestamp),
      };
    case "answer_start":
      return {
        kind: "answer_start",
        request_id: String(raw.request_id ?? ""),
        session_id: String(raw.session_id ?? ""),
        message_id: Number(raw.message_id ?? 0),
        agent_type: String(raw.agent_type ?? ""),
      };
    case "trace":
      return {
        kind: "trace",
        request_id: String(raw.request_id ?? ""),
        stage: String(raw.stage ?? ""),
        status: String(raw.status ?? ""),
        detail: raw.detail ?? null,
      };
    case "token":
      return {
        kind: "token",
        request_id: String(raw.request_id ?? ""),
        message_id: Number(raw.message_id ?? 0),
        content: String(raw.content ?? ""),
      };
    case "citations":
      return {
        kind: "citations",
        request_id: String(raw.request_id ?? ""),
        message_id: Number(raw.message_id ?? 0),
        citations: Array.isArray(raw.citations) ? (raw.citations as Citation[]) : [],
      };
    case "done":
      return {
        kind: "done",
        request_id: String(raw.request_id ?? ""),
        session_id: String(raw.session_id ?? ""),
        message_id: Number(raw.message_id ?? 0),
        payload: (raw.payload ?? {}) as ChatResponse,
      };
    case "error":
      return {
        kind: "error",
        request_id: String(raw.request_id ?? ""),
        code: String(raw.code ?? ""),
        message: String(raw.message ?? ""),
      };
    default:
      return null;
  }
}

function splitLine(buffer: string) {
  const newlineIndex = buffer.indexOf("\n");

  if (newlineIndex === -1) {
    return { line: null, rest: buffer };
  }

  return {
    line: buffer.slice(0, newlineIndex),
    rest: buffer.slice(newlineIndex + 1),
  };
}

async function dispatchEvent(
  eventName: string,
  dataLines: string[],
  onEvent: (event: WorkspaceChatStreamEvent) => void | Promise<void>,
) {
  const event = decodeChatEvent(eventName, dataLines.join("\n"));

  if (event) {
    await onEvent(event);
  }
}

export async function parseWorkspaceChatEventStream(
  stream: ReadableStream<Uint8Array> | null,
  onEvent: (event: WorkspaceChatStreamEvent) => void | Promise<void>,
): Promise<void> {
  if (!stream) {
    return;
  }

  const reader = stream.getReader();
  const decoder = new TextDecoder();
  let buffer = "";
  let eventName = "";
  let dataLines: string[] = [];

  const flush = async () => {
    if (eventName || dataLines.length > 0) {
      await dispatchEvent(eventName, dataLines, onEvent);
      eventName = "";
      dataLines = [];
    }
  };

  try {
    while (true) {
      const { done, value } = await reader.read();

      if (done) {
        buffer += decoder.decode();
        break;
      }

      buffer += decoder.decode(value, { stream: true });

      while (buffer.length > 0) {
        const { line, rest } = splitLine(buffer);

        if (line === null) {
          buffer = rest;
          break;
        }

        buffer = rest;

        const normalizedLine = line.endsWith("\r") ? line.slice(0, -1) : line;

        if (normalizedLine.length === 0) {
          await flush();
          continue;
        }

        if (normalizedLine.startsWith(":")) {
          continue;
        }

        const separatorIndex = normalizedLine.indexOf(":");
        const field =
          separatorIndex === -1 ? normalizedLine : normalizedLine.slice(0, separatorIndex);
        let value = separatorIndex === -1 ? "" : normalizedLine.slice(separatorIndex + 1);

        if (value.startsWith(" ")) {
          value = value.slice(1);
        }

        if (field === "event") {
          eventName = value;
          continue;
        }

        if (field === "data") {
          dataLines.push(value);
        }
      }
    }
  } finally {
    reader.releaseLock();
  }

  await flush();
}

export async function streamWorkspaceChat(
  token: string,
  request: ChatRequest,
  onEvent: (event: WorkspaceChatStreamEvent) => void | Promise<void>,
): Promise<void> {
  const response = await fetch(buildApiUrl("/api/v1/chat"), {
    method: "POST",
    cache: "no-store",
    headers: {
      Accept: "text/event-stream",
      "Content-Type": "application/json",
      Authorization: `Bearer ${token}`,
    },
    body: JSON.stringify({
      ...request,
      stream: true,
    }),
  });

  if (!response.ok) {
    throw await decodeError(response);
  }

  await parseWorkspaceChatEventStream(response.body, onEvent);
}
