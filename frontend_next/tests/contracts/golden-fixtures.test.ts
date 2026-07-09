import { readFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import { describe, expect, it } from "vitest";

import {
  GuardAction,
  RiskLevel,
  type ChatDonePayload,
  type ChatEvent,
  type ChatRequest,
  type ChatResponse,
  type GuardReport,
  type PlannerOutput,
} from "../../lib/contracts";
import {
  parseWireChatEvent,
  parseWorkspaceChatEventStream,
} from "../../lib/workspace/stream";

const fixturesDir = join(
  dirname(fileURLToPath(import.meta.url)),
  "../../lib/contracts/generated/fixtures",
);

function loadFixture<T>(name: string): T {
  return JSON.parse(readFileSync(join(fixturesDir, name), "utf8")) as T;
}

function makeSseStream(chunks: string[]) {
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

function sseFrame(eventName: string, payload: unknown) {
  return `event: ${eventName}\ndata: ${JSON.stringify(payload)}\n\n`;
}

describe("contract golden fixtures", () => {
  it("chat_request_minimal matches Rust-exported defaults including debug=false", () => {
    const request = loadFixture<ChatRequest>("chat_request_minimal.json");

    expect(request).toEqual({
      query: "hello",
      workspace_id: null,
      session_id: null,
      agent_type: "chat",
      source_type: null,
      source_token: null,
      doc_scope: [],
      messages: [],
      stream: false,
      debug: false,
    });
    expect("request_id" in request).toBe(false);
  });

  it("chat_request_debug preserves debug flag from Rust contract", () => {
    const request = loadFixture<ChatRequest>("chat_request_debug.json");

    expect(request.query).toBe("hello");
    expect(request.debug).toBe(true);
  });

  it("chat_event_start roundtrips the generated ChatEvent shape", () => {
    const event = loadFixture<ChatEvent>("chat_event_start.json");

    expect(event).toEqual({
      event: "start",
      request_id: "req-123",
      session_id: "session-123",
    });
  });

  it("chat_event_error roundtrips the generated ChatEvent shape", () => {
    const event = loadFixture<ChatEvent>("chat_event_error.json");

    expect(event).toEqual({
      event: "error",
      request_id: "req-err",
      code: "validation_error",
      message: "boom",
    });
  });

  it("parseWireChatEvent decodes golden start fixture from SSE data", () => {
    const wire = loadFixture<ChatEvent>("chat_event_start.json");

    expect(parseWireChatEvent("start", JSON.stringify(wire))).toEqual(wire);
  });

  it("parseWireChatEvent decodes golden error fixture from SSE data", () => {
    const wire = loadFixture<ChatEvent>("chat_event_error.json");

    expect(parseWireChatEvent("error", JSON.stringify(wire))).toEqual(wire);
  });

  it("parseWorkspaceChatEventStream decodes golden fixtures over SSE framing", async () => {
    const start = loadFixture<ChatEvent>("chat_event_start.json");
    const error = loadFixture<ChatEvent>("chat_event_error.json");
    const events: ChatEvent[] = [];

    await parseWorkspaceChatEventStream(
      makeSseStream([sseFrame("start", start), sseFrame("error", error)]),
      (event) => {
        events.push(event);
      },
    );

    expect(events).toEqual([start, error]);
  });

  it("chat_response_roundtrip matches generated ChatResponse including nullable guard/planner fields", () => {
    const response = loadFixture<ChatResponse>("chat_response_roundtrip.json");

    expect(response.answer).toBe("hello");
    expect(response.answer_blocks).toEqual([
      {
        type: "text",
        text: "hello",
        citations: ["1"],
      },
    ]);
    expect(response.guard_report).toBeNull();
    expect(response.planner_output).toBeNull();
    expect(response.degrade_trace).toEqual([
      {
        stage: "planner",
        reason: "planner_failed",
        impact: "quality",
      },
    ]);
  });

  it("chat_done_payload nests ChatResponse with nullable guard/planner fields", () => {
    const payload = loadFixture<ChatDonePayload>("chat_done_payload.json");

    expect(payload.request_id).toBe("req-123");
    expect(payload.session_id).toBe("session-123");
    expect(payload.message_id).toBe(7);
    expect(payload.response.answer).toBe("done");
    expect(payload.response.guard_report).toBeNull();
    expect(payload.response.planner_output).toBeNull();
  });

  it("GuardReport and PlannerOutput align with converged Rust contract shapes", () => {
    const guardReport: GuardReport = {
      blocked: false,
      output_results: [
        {
          passed: true,
          guard_type: "pii_scrubber",
          risk_level: RiskLevel.Medium,
          action: GuardAction.Flag,
          reason: "sensitive entity detected",
        },
      ],
    };

    const plannerOutput: PlannerOutput = {
      mode: "rag",
      rag_plan: {
        plan_version: "1",
        plan_confidence: 0.9,
        items: [{ priority: 1, query: "hello", bm25_terms: ["hello"] }],
      },
    };

    expect(guardReport.blocked).toBe(false);
    expect(guardReport.output_results?.[0]?.action).toBe("Flag");
    expect(plannerOutput.mode).toBe("rag");
    expect(plannerOutput.rag_plan?.items?.[0]?.query).toBe("hello");
  });
});
