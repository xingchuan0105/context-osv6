import { describe, expect, it } from "vitest";

import { markdownToRichTextHtml } from "../../components/workspace/workspace-note-rich-text";

describe("markdownToRichTextHtml ordered lists", () => {
  it("keeps the model's own numbers as plain text (no browser <ol> re-numbering)", () => {
    const html = markdownToRichTextHtml("1. Alpha\n2. Beta\n3. Gamma");
    expect(html).toBe("<p>1. Alpha</p><p>2. Beta</p><p>3. Gamma</p>");
    expect(html.includes("<ol")).toBe(false);
  });

  it("preserves blank-line-separated numbered lines without inventing list markers", () => {
    const html = markdownToRichTextHtml("1. Alpha\n\n2. Beta\n\n3. Gamma");
    expect(html).toBe("<p>1. Alpha</p><p>2. Beta</p><p>3. Gamma</p>");
    expect(html.includes("<ol")).toBe(false);
  });
});

describe("markdownToRichTextHtml headings (W5 #16②)", () => {
  it("renders h1–h6 as tags (#### must not leak as literal text)", () => {
    const html = markdownToRichTextHtml(
      ["# One", "## Two", "### Three", "#### Four", "##### Five", "###### Six"].join("\n"),
    );
    expect(html).toContain("<h1>One</h1>");
    expect(html).toContain("<h2>Two</h2>");
    expect(html).toContain("<h3>Three</h3>");
    expect(html).toContain("<h4>Four</h4>");
    expect(html).toContain("<h5>Five</h5>");
    expect(html).toContain("<h6>Six</h6>");
    expect(html).not.toContain("####");
  });
});
