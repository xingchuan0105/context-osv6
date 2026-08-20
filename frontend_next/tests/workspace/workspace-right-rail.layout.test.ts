import { readFileSync } from "node:fs";
import { join } from "node:path";
import { describe, expect, it } from "vitest";

function readCss(relativePath: string) {
  return readFileSync(join(process.cwd(), relativePath), "utf8");
}

describe("workspace right-rail overflow", () => {
  it("keeps the sources list as the scroll owner inside a height-capped pane", () => {
    const rail = readCss("components/workspace/workspace-right-rail.module.css");
    const shell = readCss("components/workspace/workspace-shell.module.css");

    expect(rail).toMatch(/\.railPane\s*\{[^}]*height:\s*100%/);
    expect(rail).toMatch(/\.railPane\s*\{[^}]*min-height:\s*0/);
    expect(rail).toMatch(/\.sectionScroller\s*\{[^}]*flex:\s*1 1 0/);
    expect(rail).toMatch(/\.sectionScroller\s*\{[^}]*overflow-y:\s*auto/);
    expect(shell).toMatch(/\.desktopRightRail\s*\{[^}]*display:\s*flex/);
    expect(shell).toMatch(/\.desktopRightRail\s*>\s*\*\s*\{[^}]*height:\s*100%/);
  });
});
