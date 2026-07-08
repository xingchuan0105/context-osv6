import { z } from "zod";
import { fetchResponse } from "../http/request";
import type {
  AnswerBlock,
  ChatActivitySourcePreview,
  ChatDonePayload,
  ChatEvent,
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

/** Stream events consumed by chat reducers (same shape as wire {@link ChatEvent}). */
export type WorkspaceChatStreamEvent = ChatEvent;

const CHAT_EVENT_NAMES = new Set<ChatEvent["event"]>([
  "start",
  "activity",
  "answer_start",
  "trace",
  "token",
  "reasoning_summary_delta",
  "citations",
  "done",
  "error",
]);

function parseSourceLocator(value: unknown): Record<string, unknown> | undefined {
  if (!value || typeof value !== "object" || Array.isArray(value)) {
    if (process.env.NODE_ENV !== "production" && value != null) {
      console.warn("parseCitation: invalid source_locator shape", value);
    }
    return undefined;
  }

  const raw = value as Record<string, unknown>;
  const parsed: Record<string, unknown> = {};

  if (typeof raw.url === "string" && raw.url.trim()) {
    parsed.url = raw.url.trim();
  }

  if (raw.page != null) {
    const page = Number(raw.page);
    if (!Number.isNaN(page)) {
      parsed.page = page;
    }
  }

  if (Object.keys(parsed).length === 0) {
    if (process.env.NODE_ENV !== "production") {
      console.warn("parseCitation: source_locator had no usable fields", value);
    }
    return undefined;
  }

  return parsed;
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
    source_locator: parseSourceLocator(c.source_locator),
    parse_run_id: c.parse_run_id == null ? undefined : String(c.parse_run_id),
  };
}

function parseSourcePreview(item: unknown): ProgressSourcePreview {
  const src = (item && typeof item === "object" ? item : {}) as Record<string, unknown>;

  return {
    id: String(src.id ?? ""),
    label: String(src.label ?? ""),
    href: src.href == null ? undefined : String(src.href),
  };
}

const strField = z.unknown().transform((v) => (v == null ? "" : String(v)));
const numField = z.unknown().transform((v) => {
  const n = Number(v);
  return Number.isNaN(n) ? 0 : n;
});
const optStrField = z.unknown().transform((v) => (v == null ? null : String(v)));

const CHAT_EVENT_SCHEMAS: Record<string, z.ZodType> = {
  start: z.object({
    event: z.literal("start"),
    request_id: strField,
    session_id: strField,
  }),
  activity: z.object({
    event: z.literal("activity"),
    request_id: strField,
    phase: strField,
    title: strField,
    detail: optStrField,
    counts: z
      .record(z.string(), z.unknown())
      .default({})
      .transform((obj) =>
        Object.fromEntries(
          Object.entries(obj).map(([key, value]) => [key, Number(value ?? 0)]),
        ),
      ),
    sources_preview: z
      .array(z.unknown())
      .default([])
      .transform((arr) => arr.map(parseSourcePreview)),
    timestamp: optStrField,
  }),
  answer_start: z.object({
    event: z.literal("answer_start"),
    request_id: strField,
    session_id: strField,
    message_id: numField,
    agent_type: strField,
  }),
  trace: z.object({
    event: z.literal("trace"),
    request_id: strField,
    stage: strField,
    status: strField,
    detail: z.unknown().transform((v) => (v ?? null)),
  }),
  token: z.object({
    event: z.literal("token"),
    request_id: strField,
    message_id: numField,
    content: strField,
  }),
  reasoning_summary_delta: z
    .object({
      event: z.literal("reasoning_summary_delta"),
      request_id: strField,
      message_id: numField,
      content: z.unknown(),
      summary: z.unknown().optional(),
    })
    .transform((data) => ({
      event: data.event,
      request_id: data.request_id,
      message_id: data.message_id,
      content: String(data.content ?? data.summary ?? ""),
    })),
  citations: z.object({
    event: z.literal("citations"),
    request_id: strField,
    message_id: numField,
    citations: z
      .array(z.unknown())
      .default([])
      .transform((arr) =>
        arr
          .map(parseCitation)
          .filter((c): c is Citation => c !== null),
      ),
  }),
  done: z.object({
    event: z.literal("done"),
    request_id: strField,
    session_id: strField,
    message_id: numField,
    payload: z
      .unknown()
      .transform((v) =>
        typeof v === "object" && v !== null ? (v as Record<string, unknown>) : {},
      ),
  }),
  error: z.object({
    event: z.literal("error"),
    request_id: strField,
    code: strField,
    message: strField,
  }),
};

/** Parse SSE `data` JSON into the generated wire {@link ChatEvent} shape. */
export function parseWireChatEvent(eventName: string, dataText: string): ChatEvent | null {
  if (!eventName || !dataText.trim()) {
    return null;
  }

  if (!CHAT_EVENT_NAMES.has(eventName as ChatEvent["event"])) {
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

  if (typeof raw.event !== "string") {
    raw.event = eventName;
  }

  const schema = CHAT_EVENT_SCHEMAS[eventName as ChatEvent["event"]];
  if (!schema) {
    return null;
  }

  const result = schema.safeParse(raw);
  if (!result.success) {
    if (process.env.NODE_ENV !== "production") {
      console.warn("parseWireChatEvent: schema validation failed", result.error.issues);
    }
    return null;
  }

  return result.data as ChatEvent;
}

/** Normalize wire citation records into UI {@link Citation} objects. */
export function parseStreamCitations(items: unknown): Citation[] {
  if (!Array.isArray(items)) {
    return [];
  }

  return items
    .map(parseCitation)
    .filter((citation): citation is Citation => citation !== null);
}

export function parseIpcChatEvent(payload: unknown): ChatEvent | null {
  if (payload === null || typeof payload !== "object") {
    return null;
  }

  const raw = payload as Record<string, unknown>;
  const eventName = typeof raw.event === "string" ? raw.event : null;

  if (!eventName) {
    return null;
  }

  return parseWireChatEvent(eventName, JSON.stringify(raw));
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
  onEvent: (event: ChatEvent) => void | Promise<void>,
) {
  const event = parseWireChatEvent(eventName, dataLines.join("\n"));

  if (event) {
    await onEvent(event);
  }
}

export async function parseWorkspaceChatEventStream(
  stream: ReadableStream<Uint8Array> | null,
  onEvent: (event: ChatEvent) => void | Promise<void>,
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

/** Web-only SSE chat stream; Tauri uses IPC via `lib/runtime/transport.ts`. */
export async function streamWorkspaceChat(
  token: string,
  request: ChatRequest,
  onEvent: (event: ChatEvent) => void | Promise<void>,
  options?: { signal?: AbortSignal },
): Promise<void> {
  const response = await fetchResponse(
    "/api/v1/chat",
    {
      method: "POST",
      signal: options?.signal,
      headers: {
        Accept: "text/event-stream",
        "Content-Type": "application/json",
      },
      body: JSON.stringify({
        ...request,
        stream: true,
      }),
    },
    { token },
  );

  await parseWorkspaceChatEventStream(response.body, onEvent);
}
