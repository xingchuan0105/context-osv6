import { ApiError, buildApiUrl } from "../auth/client";
import type {
  AnswerBlock,
  ChatActivitySourcePreview,
  ChatDonePayload,
  ChatRequest,
  ChatResponse,
  Citation,
  DegradeTraceItem,
  ToolResult,
} from "../contracts";

export type {
  AnswerBlock,
  ChatDonePayload,
  ChatRequest,
  ChatResponse,
  ChatTurnInput,
  Citation,
  DegradeTraceItem,
  GuardReport,
  ModeDebug,
  PlannerOutput,
  SourceRef,
  ToolResult,
  TraceInfo,
} from "../contracts";

export { ToolStatus } from "../contracts";

/** Frontend alias for generated `ChatActivitySourcePreview`. */
export type ProgressSourcePreview = ChatActivitySourcePreview;

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
      kind: "reasoning_summary_delta";
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

function parseCitation(item: unknown): Citation | null {
  if (!item || typeof item !== "object") {
    return null;
  }

  const c = item as Record<string, unknown>;
  const docId = String(c.doc_id ?? "");

  if (!docId) {
    return null;
  }

  return {
    citation_id: Number(c.citation_id ?? 0),
    doc_id: docId,
    chunk_id: c.chunk_id == null ? undefined : String(c.chunk_id),
    page: c.page == null ? undefined : Number(c.page),
    doc_name: String(c.doc_name ?? ""),
    preview: c.preview == null ? undefined : String(c.preview),
    content: c.content == null ? undefined : String(c.content),
    score: Number(c.score ?? 0),
    layer: c.layer == null ? undefined : String(c.layer),
    chunk_type: c.chunk_type == null ? undefined : String(c.chunk_type),
    asset_id: c.asset_id == null ? undefined : String(c.asset_id),
    caption: c.caption == null ? undefined : String(c.caption),
    image_url: c.image_url == null ? undefined : String(c.image_url),
    parser_backend: c.parser_backend == null ? undefined : String(c.parser_backend),
    source_locator: c.source_locator ?? undefined,
    parse_run_id: c.parse_run_id == null ? undefined : String(c.parse_run_id),
  };
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
          ? raw.sources_preview.map((item: unknown) => {
              const src = item as Record<string, unknown>;
              return {
                id: String(src.id ?? ""),
                label: String(src.label ?? ""),
                href: src.href == null ? undefined : String(src.href),
              };
            })
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
    case "reasoning_summary_delta":
      return {
        kind: "reasoning_summary_delta",
        request_id: String(raw.request_id ?? ""),
        message_id: Number(raw.message_id ?? 0),
        content: String(raw.content ?? raw.summary ?? ""),
      };
    case "citations":
      return {
        kind: "citations",
        request_id: String(raw.request_id ?? ""),
        message_id: Number(raw.message_id ?? 0),
        citations: Array.isArray(raw.citations)
          ? raw.citations.map(parseCitation).filter((citation): citation is Citation => citation !== null)
          : [],
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
  options?: { signal?: AbortSignal },
): Promise<void> {
  const response = await fetch(buildApiUrl("/api/v1/chat"), {
    method: "POST",
    cache: "no-store",
    signal: options?.signal,
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
