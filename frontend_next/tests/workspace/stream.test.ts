import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import {
  parseWorkspaceChatEventStream,
  streamWorkspaceChat,
  type ChatRequest,
} from "../../lib/workspace/stream";
import type { ChatEvent } from "../../lib/contracts";

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
    const events: ChatEvent[] = [];

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
        event: "start",
        request_id: "req-1",
        session_id: "sess-1",
      },
      {
        event: "activity",
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
            href: undefined,
          },
        ],
        timestamp: "10:00",
      },
      {
        event: "answer_start",
        request_id: "req-1",
        session_id: "sess-1",
        message_id: 0,
        agent_type: "rag",
      },
      {
        event: "trace",
        request_id: "req-1",
        stage: "rag",
        status: "started",
        detail: { step: 1 },
      },
      {
        event: "reasoning_summary_delta",
        request_id: "req-1",
        message_id: 7,
        content: "正在比较候选证据",
      },
      {
        event: "token",
        request_id: "req-1",
        message_id: 7,
        content: "Hello",
      },
      {
        event: "citations",
        request_id: "req-1",
        message_id: 7,
        citations: [
          {
            citation_id: 1,
            doc_id: "doc-1",
            chunk_id: undefined,
            page: undefined,
            doc_name: "Doc 1",
            preview: undefined,
            content: undefined,
            score: 0.9,
            layer: undefined,
            chunk_type: undefined,
            asset_id: undefined,
            caption: undefined,
            image_url: undefined,
            parser_backend: undefined,
            source_locator: undefined,
            parse_run_id: undefined,
          },
        ],
      },
      {
        event: "done",
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
        event: "error",
        request_id: "req-1",
        code: "stream_closed",
        message: "closed",
      },
    ]);
  });

  it("drops invalid source_locator shapes when parsing citations", async () => {
    const warnSpy = vi.spyOn(console, "warn").mockImplementation(() => {});
    const events: ChatEvent[] = [];

    await parseWorkspaceChatEventStream(
      makeStream([
        'event: citations\ndata: {"request_id":"req-3","message_id":8,"citations":[{"citation_id":2,"doc_id":"doc-2","doc_name":"Doc 2","score":0.5,"source_locator":"not-an-object"}]}\n\n',
        'event: citations\ndata: {"request_id":"req-3","message_id":8,"citations":[{"citation_id":3,"doc_id":"doc-3","doc_name":"Doc 3","score":0.6,"source_locator":{"url":"https://example.test/source","page":4}}]}\n\n',
      ]),
      (event) => {
        events.push(event);
      },
    );

    expect(events).toEqual([
      {
        event: "citations",
        request_id: "req-3",
        message_id: 8,
        citations: [
          {
            citation_id: 2,
            doc_id: "doc-2",
            chunk_id: undefined,
            page: undefined,
            doc_name: "Doc 2",
            preview: undefined,
            content: undefined,
            score: 0.5,
            layer: undefined,
            chunk_type: undefined,
            asset_id: undefined,
            caption: undefined,
            image_url: undefined,
            parser_backend: undefined,
            source_locator: undefined,
            parse_run_id: undefined,
          },
        ],
      },
      {
        event: "citations",
        request_id: "req-3",
        message_id: 8,
        citations: [
          {
            citation_id: 3,
            doc_id: "doc-3",
            chunk_id: undefined,
            page: undefined,
            doc_name: "Doc 3",
            preview: undefined,
            content: undefined,
            score: 0.6,
            layer: undefined,
            chunk_type: undefined,
            asset_id: undefined,
            caption: undefined,
            image_url: undefined,
            parser_backend: undefined,
            source_locator: {
              url: "https://example.test/source",
              page: 4,
            },
            parse_run_id: undefined,
          },
        ],
      },
    ]);
    expect(warnSpy).toHaveBeenCalled();
    warnSpy.mockRestore();
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
      workspace_id: "ws-1",
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

    const events: ChatEvent[] = [];

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
      workspace_id: "ws-1",
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
        event: "start",
        request_id: "req-2",
        session_id: "sess-2",
      },
      {
        event: "answer_start",
        request_id: "req-2",
        session_id: "sess-2",
        message_id: 0,
        agent_type: "rag",
      },
      {
        event: "done",
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
