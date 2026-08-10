import { readdirSync, readFileSync } from "node:fs";
import { join } from "node:path";
import { describe, expect, it } from "vitest";

/**
 * Design-baseline guard (docs/design/STYLE_BASELINE.md, PRODUCT_IA_AUDIT A2-9):
 * the Cursor/x.ai token system is the only source of weight, color, and
 * elevation. These rules make A2-3 / A2-5 / A2-6 stick mechanically:
 *
 * - no numeric font-weight ≥ 500 (weights are all 400 by token design);
 * - no bare hex colors outside token-definition files;
 * - no literal drop shadows except on floating overlays (allowlisted files).
 */

const SCAN_DIRS = ["app", "components"];

/** Token definitions may carry raw hex values. */
const HEX_ALLOWED_FILES = new Set([
  "app/design-tokens.css",
  "app/globals.css",
]);

/** OG/social preview brand asset: fixed colors, not token-themed UI. */
const TSX_HEX_ALLOWED_FILES = new Set([
  "app/metadata-brand.tsx",
]);

/** Floating overlays (command palette, dropdown menus) may keep drop shadows. */
const SHADOW_ALLOWED_FILES = new Set([
  "components/command-palette/command-palette.module.css",
  "components/workspace/workspace-shell.module.css",
]);

function collectFiles(dir: string, extensions: string[]): string[] {
  const out: string[] = [];
  let entries: string[];
  try {
    entries = readdirSync(join(process.cwd(), dir), { recursive: true }) as unknown as string[];
  } catch {
    return out;
  }
  for (const entry of entries) {
    const file = String(entry);
    if (file.includes("node_modules") || file.includes("/.next/")) {
      continue;
    }
    if (extensions.some((ext) => file.endsWith(ext))) {
      out.push(join(dir, file));
    }
  }
  return out;
}

const CSS_FILES = SCAN_DIRS.flatMap((dir) => collectFiles(dir, [".css"]));
const TSX_FILES = SCAN_DIRS.flatMap((dir) => collectFiles(dir, [".tsx"]));

function violations(
  files: string[],
  pattern: RegExp,
  skip?: (file: string) => boolean,
): string[] {
  const hits: string[] = [];
  for (const file of files) {
    if (skip?.(file)) {
      continue;
    }
    const lines = readFileSync(join(process.cwd(), file), "utf-8").split("\n");
    lines.forEach((line, index) => {
      if (pattern.test(line)) {
        hits.push(`${file}:${index + 1} → ${line.trim()}`);
      }
    });
  }
  return hits;
}

describe("design baseline guard", () => {
  it("has no font-weight ≥ 500 (numeric or named) in CSS (weights stay on tokens = 400)", () => {
    expect(violations(CSS_FILES, /font-weight:\s*([5-9]\d{2}\b|bold\b|bolder\b)/)).toEqual([]);
  });

  it("has no fontWeight ≥ 500 (numeric, quoted, or named) in inline TSX styles", () => {
    expect(violations(TSX_FILES, /fontWeight:\s*["']?([5-9]\d{2}\b|bold\b|bolder\b)/)).toEqual([]);
  });

  it("has no bare hex colors outside token-definition files", () => {
    expect(
      violations(CSS_FILES, /#[0-9a-fA-F]{3,8}\b/, (file) => HEX_ALLOWED_FILES.has(file)),
    ).toEqual([]);
  });

  it("has no bare hex colors in inline TSX styles", () => {
    expect(
      violations(TSX_FILES, /#[0-9a-fA-F]{3,8}\b/, (file) => TSX_HEX_ALLOWED_FILES.has(file)),
    ).toEqual([]);
  });

  it("has no literal drop shadows outside floating overlays", () => {
    // Drop shadow = non-inset shadow with real offset/blur. Hairline rings
    // (0 0 0 1px …) and inset 1px rings are allowed, as are var(--shadow-*).
    expect(
      violations(
        CSS_FILES,
        /box-shadow:\s*(?!none\b)(?!var\()(?!inset\b)0(?:px)?\s+(\d+px)\s+(\d+px)/,
        (file) => SHADOW_ALLOWED_FILES.has(file),
      ),
    ).toEqual([]);
  });
});
