import { describe, expect, it } from "vitest";

import {
  labelDegradeReasons,
  userVisibleDegradeReasons,
} from "../../lib/workspace/degrade-display";

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

describe("labelDegradeReasons", () => {
  it("maps codes to product language and omits unknown raw ids", () => {
    expect(
      labelDegradeReasons(
        ["fallback_to_summary", "tool_unavailable", "mystery_code"],
        "zh-CN",
      ),
    ).toEqual(["改为摘要回答"]);
    expect(labelDegradeReasons(["no_retrieval_evidence"], "en")).toEqual([
      "Not enough retrieval evidence",
    ]);
  });
});
