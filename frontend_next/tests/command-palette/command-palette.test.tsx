import { fireEvent, render, screen } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

const pushMock = vi.fn();

vi.mock("next/navigation", () => ({
  useRouter: () => ({ push: pushMock }),
}));

vi.mock("@/lib/ui-preferences", () => ({
  useUiPreferences: () => ({ locale: "zh-CN" as const, theme: "system" as const }),
}));

import { CommandPaletteHost } from "@/components/command-palette/command-palette";

describe("CommandPaletteHost", () => {
  beforeEach(() => {
    pushMock.mockReset();
  });

  it("opens on Ctrl+K and navigates to dashboard", () => {
    render(<CommandPaletteHost />);

    expect(screen.queryByTestId("command-palette")).toBeNull();

    fireEvent.keyDown(document, { key: "k", ctrlKey: true });
    expect(screen.getByTestId("command-palette")).toBeTruthy();
    expect(screen.getByTestId("command-palette-item-dashboard")).toBeTruthy();

    fireEvent.click(screen.getByTestId("command-palette-item-topup"));
    expect(pushMock).toHaveBeenCalledWith("/pricing#topup");
  });

  it("filters commands by query", () => {
    render(<CommandPaletteHost />);
    fireEvent.keyDown(document, { key: "k", metaKey: true });

    fireEvent.change(screen.getByTestId("command-palette-input"), {
      target: { value: "充值" },
    });

    expect(screen.getByTestId("command-palette-item-topup")).toBeTruthy();
    expect(screen.queryByTestId("command-palette-item-dashboard")).toBeNull();
  });
});
