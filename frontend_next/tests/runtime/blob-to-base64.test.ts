import { describe, expect, it } from "vitest";

import { blobToBase64 } from "@/lib/runtime/bytes";

describe("blobToBase64", () => {
  it("round-trips ascii bytes", async () => {
    const blob = new Blob(["hello"], { type: "text/plain" });
    expect(await blobToBase64(blob)).toBe(btoa("hello"));
  });
});
