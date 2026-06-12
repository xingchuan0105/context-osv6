import { describe, expect, it } from "vitest";

import { isSafeHttpUrl, toSafeHttpUrl } from "../../lib/url/isSafeHttpUrl";

describe("isSafeHttpUrl", () => {
  it.each([
    "https://example.com",
    "http://example.com/path",
    "mailto:user@example.com",
    "HTTPS://EXAMPLE.COM",
  ])("allows safe URL %s", (url) => {
    expect(isSafeHttpUrl(url)).toBe(true);
    expect(toSafeHttpUrl(url)).toBe(url.trim());
  });

  it.each([
    "javascript:alert(1)",
    "data:text/html,<script>alert(1)</script>",
    "vbscript:msgbox(1)",
    "blob:https://example.com/uuid",
    "file:///etc/passwd",
    "",
    "   ",
    "ftp://example.com",
    "not-a-url",
  ])("rejects unsafe URL %s", (url) => {
    expect(isSafeHttpUrl(url)).toBe(false);
    expect(toSafeHttpUrl(url)).toBeNull();
  });
});
