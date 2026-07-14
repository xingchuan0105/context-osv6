import { describe, expect, it } from "vitest";

import {
  formatBytes,
  windowsDownloadFromManifest,
  type DesktopReleaseManifest,
} from "@/lib/desktop/release-manifest";

const sample: DesktopReleaseManifest = {
  product: "AVRag Desktop",
  version: "0.1.0",
  published_at: "2026-07-14T00:00:00Z",
  platforms: {
    "windows-x64": {
      url: "/releases/desktop/v0.1.0/AVRag-Desktop_0.1.0_x64.exe",
      sha256: "abc",
      size_bytes: 21_380_056,
      format: "portable",
      filename: "AVRag-Desktop_0.1.0_x64.exe",
    },
  },
};

describe("desktop release manifest helpers", () => {
  it("reads windows-x64 platform", () => {
    const win = windowsDownloadFromManifest(sample);
    expect(win?.url).toContain("/releases/desktop/");
    expect(win?.format).toBe("portable");
  });

  it("returns null when platform missing", () => {
    expect(windowsDownloadFromManifest({ ...sample, platforms: {} })).toBeNull();
    expect(windowsDownloadFromManifest(null)).toBeNull();
  });

  it("formats bytes", () => {
    expect(formatBytes(21_380_056)).toMatch(/MB/);
    expect(formatBytes(0)).toBe("—");
  });
});
