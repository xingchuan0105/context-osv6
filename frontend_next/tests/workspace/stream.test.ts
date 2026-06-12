import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import {
  parseWorkspaceChatEventStream,
  streamWorkspaceChat,
  type ChatRequest,
  type WorkspaceChatStreamEvent,
} from "../../lib/workspace/stream";

const fetchMock = vi.fn();

function makeStream(chunks: string[]) {
  const encoder = new TextEncoder();

  return new ReadableStream<Uint8Array>({
    start(controller) {
      for (const chunk of chunks) {
        controller.enqueue(encoder.encode(chunk));
      }

      controller.close();
    },
  });
}

beforeEach(() => {
  process.env.NEXT_PUBLIC_API_BASE_URL = "https://api.example.test";
  fetchMock.mockReset();
  vi.stubGlobal("fetch", fetchMock);
});

afterEach(() => {
  delete process.env.NEXT_PUBLIC_API_BASE_URL;
  vi.unstubAllGlobals();
});

describe("workspace chat stream transport", () => {
  it("parses chunked SSE frames into reducer-friendly events", async () => {
    const events: WorkspaceChatStreamEvent[] = [];

    await parseWorkspaceChatEventStream(
      makeStream([
        'event: start\ndata: {"request_id":"req-1","session_id":"sess-1"}\n\n',
        'event: activity\ndata: {"request_id":"req-1","phase":"retrieving","title":"正在读取来源","detail":"已命中多个网页来源","counts":{"sources":4},"sources_preview":[{"id":"source-1","label":"example.com"}],"timestamp":"10:00"}\n\n',
        'event: answer_start\ndata: {"request_id":"req-1","session_id":"sess-1","message_id":0,"agent_type":"rag"}\n\n',
        'event: trace\ndata: {"request_id":"req-1","stage":"rag","status":"started","detail":{"step":1}}\n\n',
        'event: reasoning_summary_delta\ndata: {"request_id":"req-1","message_id":7,"content":"正在比较候选证据"}\n\n',
        'event: token\ndata: {"request_id":"req-1","message_id":7,"content":"Hel',
        'lo"}\n\n',
        'event: citations\ndata: {"request_id":"req-1","message_id":7,"citations":[{"citation_id":1,"doc_id":"doc-1","doc_name":"Doc 1","score":0.9}]}\n\n',
        'event: done\ndata: {"request_id":"req-1","session_id":"sess-1","message_id":7,"payload":{"answer":"Hello","session_id":"sess-1","agent_type":"rag","sources":[],"citations":[],"trace":{"mode":"rag"},"degrade_trace":[],"answer_blocks":[]}}\n\n',
        'event: error\ndata: {"request_id":"req-1","code":"stream_closed","message":"closed"}\n\n',
      ]),
      (event) => {
        events.push(event);
      },
    );

    expect(events).toEqual([
      {
        kind: "start",
        request_id: "req-1",
        session_id: "sess-1",
      },
      {
        kind: "activity",
        request_id: "req-1",
        phase: "retrieving",
        title: "正在读取来源",
        detail: "已命中多个网页来源",
        counts: {
          sources: 4,
        },
        sources_preview: [
          {
            id: "source-1",
            label: "example.com",
            href: null,
          },
        ],
        timestamp: "10:00",
      },
      {
        kind: "answer_start",
        request_id: "req-1",
        session_id: "sess-1",
        message_id: 0,
        agent_type: "rag",
      },
      {
        kind: "trace",
        request_id: "req-1",
        stage: "rag",
        status: "started",
        detail: { step: 1 },
      },
      {
        kind: "reasoning_summary_delta",
        request_id: "req-1",
        message_id: 7,
        content: "正在比较候选证据",
      },
      {
        kind: "token",
        request_id: "req-1",
        message_id: 7,
        content: "Hello",
      },
      {
        kind: "citations",
        request_id: "req-1",
        message_id: 7,
        citations: [
          {
            citation_id: 1,
            doc_id: "doc-1",
            chunk_id: null,
            page: null,
            doc_name: "Doc 1",
            preview: null,
            content: null,
            score: 0.9,
            layer: null,
            chunk_type: null,
            asset_id: null,
            caption: null,
            image_url: null,
            source_locator: null,
            parse_run_id: null,
          },
        ],
      },
      {
        kind: "done",
        request_id: "req-1",
        session_id: "sess-1",
        message_id: 7,
        payload: {
          answer: "Hello",
          session_id: "sess-1",
          agent_type: "rag",
          sources: [],
          citations: [],
          trace: { mode: "rag" },
          degrade_trace: [],
          answer_blocks: [],
        },
      },
      {
        kind: "error",
        request_id: "req-1",
        code: "stream_closed",
        message: "closed",
      },
    ]);
  });

  it("does not log token or trace payloads while parsing streaming diagnostics", async () => {
    const infoSpy = vi.spyOn(console, "info").mockImplementation(() => {});

    await parseWorkspaceChatEventStream(
      makeStream([
        'event: activity\ndata: {"request_id":"req-log","phase":"planning","title":"正在生成网络搜索计划","detail":"系统正在拆解问题并准备搜索网页来源。","counts":{"queries":1},"sources_preview":[],"timestamp":"10:00"}\n\n',
        'event: trace\ndata: {"request_id":"req-log","stage":"rag","status":"started","detail":{"secret_query":"sensitive"}}\n\n',
        'event: token\ndata: {"request_id":"req-log","message_id":3,"content":"private-token"}\n\n',
      ]),
      () => {},
    );

    expect(infoSpy).not.toHaveBeenCalled();
  });

  it("posts the chat request to /api/v1/chat and streams the response", async () => {
    const requestBody: ChatRequest = {
      query: "Explain the plan",
      notebook_id: "ws-1",
      session_id: "sess-1",
      agent_type: "rag",
      source_type: "docs",
      source_token: "token-1",
      doc_scope: ["doc-1"],
      messages: [{ role: "user", content: "Explain the plan" }],
      stream: true,
    };

    fetchMock.mockResolvedValue(
      new Response(
        makeStream([
          'event: start\ndata: {"request_id":"req-2","session_id":"sess-2"}\n\n',
          'event: answer_start\ndata: {"request_id":"req-2","session_id":"sess-2","message_id":0,"agent_type":"rag"}\n\n',
          'event: done\ndata: {"request_id":"req-2","session_id":"sess-2","message_id":11,"payload":{"answer":"Done","session_id":"sess-2","agent_type":"rag","sources":[],"citations":[],"trace":{"mode":"rag"},"degrade_trace":[],"answer_blocks":[]}}\n\n',
        ]),
        {
          status: 200,
          headers: { "Content-Type": "text/event-stream" },
        },
      ),
    );

    const events: WorkspaceChatStreamEvent[] = [];

    await streamWorkspaceChat("token-123", requestBody, (event) => {
      events.push(event);
    });

    expect(fetchMock).toHaveBeenCalledWith(
      "https://api.example.test/api/v1/chat",
      expect.objectContaining({
        method: "POST",
        cache: "no-store",
      }),
    );

    const [, init] = fetchMock.mock.calls[0]!;
    const headers = new Headers(init.headers);

    expect(headers.get("Authorization")).toBe("Bearer token-123");
    expect(headers.get("Accept")).toBe("text/event-stream");
    expect(headers.get("Content-Type")).toBe("application/json");
    expect(JSON.parse(String(init.body))).toEqual({
      query: "Explain the plan",
      notebook_id: "ws-1",
      session_id: "sess-1",
      agent_type: "rag",
      source_type: "docs",
      source_token: "token-1",
      doc_scope: ["doc-1"],
      messages: [{ role: "user", content: "Explain the plan" }],
      stream: true,
    });

    expect(events).toEqual([
      {
        kind: "start",
        request_id: "req-2",
        session_id: "sess-2",
      },
      {
        kind: "answer_start",
        request_id: "req-2",
        session_id: "sess-2",
        message_id: 0,
        agent_type: "rag",
      },
      {
        kind: "done",
        request_id: "req-2",
        session_id: "sess-2",
        message_id: 11,
        payload: {
          answer: "Done",
          session_id: "sess-2",
          agent_type: "rag",
          sources: [],
          citations: [],
          trace: { mode: "rag" },
          degrade_trace: [],
          answer_blocks: [],
        },
      },
    ]);
  });
});
