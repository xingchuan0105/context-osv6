import { describe, expect, it } from "vitest";

import { userVisibleDegradeReasons } from "../../lib/workspace/degrade-display";

describe("userVisibleDegradeReasons", () => {
  it("hides tool_unavailable and dedupes", () => {
    expect(
      userVisibleDegradeReasons([
        "tool_unavailable",
        "tool_unavailable",
        "tool_unavailable",
      ]),
    ).toEqual([]);
  });

  it("keeps user-facing codes and drops hidden ones", () => {
    expect(
      userVisibleDegradeReasons([
        "tool_unavailable",
        "fallback_to_summary",
        "tool_degraded",
        "fallback_to_summary",
        "no_retrieval_evidence",
      ]),
    ).toEqual(["fallback_to_summary", "no_retrieval_evidence"]);
  });
});
