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
