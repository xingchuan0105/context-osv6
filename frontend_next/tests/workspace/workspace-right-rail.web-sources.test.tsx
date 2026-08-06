import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";

import { WorkspaceWebSourcesModal } from "../../components/workspace/workspace-web-sources-modal";

vi.mock("../../lib/ui-preferences", () => ({
  useUiPreferences: () => ({ locale: "en" as const, theme: "system" as const }),
}));

describe("WorkspaceWebSourcesModal", () => {
  it("renders web sources in a modal and closes via close control", async () => {
    const user = userEvent.setup();
    const onClose = vi.fn();

    render(
      <WorkspaceWebSourcesModal
        request={{
          sources: [
            {
              title: "Primary result",
              url: "https://example.test/primary",
              snippet: "Primary snippet",
            },
            {
              title: "Secondary result",
              url: "https://example.test/secondary",
              snippet: "Secondary snippet",
            },
          ],
        }}
        onClose={onClose}
      />,
    );

    expect(await screen.findByTestId("workspace-web-sources-modal")).toBeTruthy();
    expect(screen.getByText("2 sources")).toBeTruthy();
    const primaryLink = screen.getByRole("link", { name: "Primary result" });
    expect(primaryLink.getAttribute("href")).toBe("https://example.test/primary");
    expect(screen.getByText("Primary snippet")).toBeTruthy();

    await user.click(screen.getByRole("button", { name: "Close" }));
    expect(onClose).toHaveBeenCalled();
  });
});
