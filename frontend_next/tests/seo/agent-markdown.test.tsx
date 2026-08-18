import { render, screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";

import { renderAgentMarkdown } from "@/lib/api-access/render-agent-markdown";

describe("renderAgentMarkdown", () => {
  it("emits real H2 nodes from ATX headings", () => {
    const md = `# Title\n\n## Connection\n\nHello world.\n\n## Scope\n\n- a\n- b\n`;
    render(<div>{renderAgentMarkdown(md)}</div>);

    expect(screen.getByRole("heading", { level: 1, name: "Title" })).toBeTruthy();
    expect(screen.getByRole("heading", { level: 2, name: "Connection" })).toBeTruthy();
    expect(screen.getByRole("heading", { level: 2, name: "Scope" })).toBeTruthy();
    expect(screen.getByText("Hello world.")).toBeTruthy();
    expect(screen.getByText("a")).toBeTruthy();
  });
});
