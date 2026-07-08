import { readFileSync } from "node:fs";
import { join } from "node:path";
import { describe, expect, it } from "vitest";

import type { ChatResponse } from "../lib/contracts/generated/contracts";

// Keys from the Rust `ChatResponse` struct (contracts/src/chat.rs).
// When a field is added or removed in Rust, update this list and run
// `pnpm generate:contracts` to regenerate the TS contract.
// The `satisfies` constraint ensures every listed key exists in the
// generated TypeScript type — if typeshare silently drops a field that
// is listed here, `pnpm typecheck` will fail.
const EXPECTED_CHAT_RESPONSE_KEYS = [
  "agent_operation_guide",
  "agent_type",
  "answer",
  "answer_blocks",
  "citations",
  "degrade_trace",
  "guard_report",
  "message_id",
  "mode_debug",
  "planner_output",
  "session_id",
  "sources",
  "tool_results",
  "trace",
  "usage",
] as const satisfies readonly (keyof ChatResponse)[];

describe("contract completeness", () => {
  it("ChatResponse TS interface has exactly the Rust-source keys (guards against silent typeshare drift)", () => {
    const contractsPath = join(
      process.cwd(),
      "lib",
      "contracts",
      "generated",
      "contracts.ts",
    );
    const source = readFileSync(contractsPath, "utf-8");

    // Extract the ChatResponse interface body.
    const match = source.match(/export interface ChatResponse \{([\s\S]*?)\}/);
    expect(match).not.toBeNull();

    // Extract field names from the interface body.
    const body = match![1];
    const fields = [...body.matchAll(/^\s*(\w+)\??:/gm)].map((m) => m[1]);

    expect(fields.sort()).toEqual([...EXPECTED_CHAT_RESPONSE_KEYS].sort());
  });
});
