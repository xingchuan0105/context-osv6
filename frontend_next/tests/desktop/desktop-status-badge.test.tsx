import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { fireEvent, render, screen, waitFor } from "@testing-library/react";

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(),
}));

vi.mock("@/lib/desktop/tauri-license", () => ({
  getLicenseStatus: vi.fn(),
}));

import { getLicenseStatus } from "@/lib/desktop/tauri-license";
import { DesktopStatusBadge } from "@/components/desktop/DesktopStatusBadge";

describe("DesktopStatusBadge", () => {
  beforeEach(() => {
    vi.mocked(getLicenseStatus).mockReset();
  });

  afterEach(() => {
    vi.clearAllMocks();
  });

  it("rendersTrialStatusLabel", async () => {
    vi.mocked(getLicenseStatus).mockResolvedValue({
      kind: "trial",
      days_remaining: 4,
      dev_mode: false,
    });

    render(<DesktopStatusBadge />);

    await waitFor(() => {
      expect(screen.getByText("试用 4d")).toBeInTheDocument();
    });
  });

  it("callsOnClickWhenBadgeClicked", async () => {
    const onClick = vi.fn();
    vi.mocked(getLicenseStatus).mockResolvedValue({
      kind: "active",
      dev_mode: false,
    });

    render(<DesktopStatusBadge onClick={onClick} />);

    await waitFor(() => {
      expect(screen.getByLabelText("授权状态")).toBeInTheDocument();
    });

    fireEvent.click(screen.getByLabelText("授权状态"));
    expect(onClick).toHaveBeenCalledTimes(1);
  });
});
