import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { render, screen } from "@testing-library/react";

const replaceMock = vi.fn();

vi.mock("next/navigation", () => ({
  useRouter: () => ({
    replace: replaceMock,
  }),
}));

vi.mock("@/lib/runtime/tauri-ipc", () => ({
  isTauri: vi.fn(),
}));

import { isTauri } from "@/lib/runtime/tauri-ipc";
import { DesktopOnlyGate } from "@/components/desktop/DesktopOnlyGate";

describe("DesktopOnlyGate", () => {
  beforeEach(() => {
    replaceMock.mockReset();
    vi.mocked(isTauri).mockReset();
  });

  afterEach(() => {
    vi.unstubAllGlobals();
  });

  it("redirects to dashboard when not running in Tauri", () => {
    vi.mocked(isTauri).mockReturnValue(false);

    const { container } = render(
      <DesktopOnlyGate>
        <div data-testid="desktop-child">Desktop content</div>
      </DesktopOnlyGate>,
    );

    expect(replaceMock).toHaveBeenCalledWith("/dashboard");
    expect(screen.queryByTestId("desktop-child")).not.toBeInTheDocument();
    expect(container).toBeEmptyDOMElement();
  });

  it("renders children when running in Tauri", () => {
    vi.mocked(isTauri).mockReturnValue(true);

    render(
      <DesktopOnlyGate>
        <div data-testid="desktop-child">Desktop content</div>
      </DesktopOnlyGate>,
    );

    expect(replaceMock).not.toHaveBeenCalled();
    expect(screen.getByTestId("desktop-child")).toBeInTheDocument();
  });
});
