import { describe, it, expect } from "vitest";

import { formatCompactToken, formatFullToken, formatCountdown, formatPct } from "../../lib/billing/format";

describe("formatCompactToken", () => {
  it("formats 100K / 1.5M / 200", () => {
    expect(formatCompactToken(100_000)).toBe("100K");
    expect(formatCompactToken(1_500_000)).toBe("1.5M");
    expect(formatCompactToken(200)).toBe("200");
  });

  it("rounds large M to integer (>=10M)", () => {
    expect(formatCompactToken(12_500_000)).toBe("13M");
  });

  it("rounds large K to integer (>=100K)", () => {
    expect(formatCompactToken(150_000)).toBe("150K");
  });
});

describe("formatFullToken", () => {
  it("formats with thousands separators", () => {
    expect(formatFullToken(100_000)).toBe("100,000");
    expect(formatFullToken(1_500_000)).toBe("1,500,000");
    expect(formatFullToken(0)).toBe("0");
  });
});

describe("formatCountdown", () => {
  it("formats 5h 23m / 2d 4h / 30m", () => {
    expect(formatCountdown(5 * 3600_000 + 23 * 60_000)).toBe("5h 23m");
    expect(formatCountdown(2 * 86400_000 + 4 * 3600_000)).toBe("2d 4h");
    expect(formatCountdown(30 * 60_000)).toBe("30m");
  });

  it("returns 0m for non-positive input", () => {
    expect(formatCountdown(0)).toBe("0m");
    expect(formatCountdown(-1)).toBe("0m");
  });

  it("omits hours when day has no remainder", () => {
    expect(formatCountdown(2 * 86400_000)).toBe("2d");
  });
});

describe("formatPct", () => {
  it("rounds to whole percent", () => {
    expect(formatPct(80)).toBe("80%");
    expect(formatPct(80.4)).toBe("80%");
    expect(formatPct(80.6)).toBe("81%");
  });
});
