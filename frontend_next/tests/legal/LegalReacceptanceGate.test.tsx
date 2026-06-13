import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";

import { LegalReacceptanceGate } from "@/components/legal/LegalReacceptanceGate";

const fetchLegalStatusMock = vi.hoisted(() => vi.fn());
const recordLegalAcceptanceMock = vi.hoisted(() => vi.fn());

vi.mock("@/lib/auth/context", () => ({
  useAuth: () => ({ token: "token-1", user: { id: "u1" } }),
}));

vi.mock("@/lib/ui-preferences", () => ({
  useUiPreferences: () => ({ locale: "zh-CN" as const }),
}));

vi.mock("@/lib/legal/client", () => ({
  fetchLegalStatus: fetchLegalStatusMock,
  recordLegalAcceptance: recordLegalAcceptanceMock,
}));

vi.mock("next/link", () => ({
  default: ({
    children,
    href,
  }: {
    children: React.ReactNode;
    href: string;
  }) => <a href={href}>{children}</a>,
}));

describe("LegalReacceptanceGate", () => {
  it("renders children when re-acceptance is not required", async () => {
    fetchLegalStatusMock.mockResolvedValueOnce({
      needs_re_acceptance: false,
      published_terms_version: "2026-06-13",
      published_privacy_version: "2026-06-13",
    });

    render(
      <LegalReacceptanceGate>
        <p>protected content</p>
      </LegalReacceptanceGate>,
    );

    expect(await screen.findByText("protected content")).toBeTruthy();
  });

  it("renders re-acceptance panel when status requires it", async () => {
    fetchLegalStatusMock.mockResolvedValueOnce({
      needs_re_acceptance: true,
      published_terms_version: "2026-06-13",
      published_privacy_version: "2026-06-13",
    });

    render(
      <LegalReacceptanceGate>
        <p>protected content</p>
      </LegalReacceptanceGate>,
    );

    expect(await screen.findByText("协议已更新")).toBeTruthy();
    expect(screen.getByRole("checkbox")).toBeTruthy();
    expect(screen.getByRole("button", { name: "确认并继续" })).toBeTruthy();
    expect(screen.queryByText("protected content")).toBeNull();
  });

  it("shows error when submit fails", async () => {
    fetchLegalStatusMock.mockResolvedValueOnce({
      needs_re_acceptance: true,
      published_terms_version: "2026-06-13",
      published_privacy_version: "2026-06-13",
    });
    recordLegalAcceptanceMock.mockRejectedValueOnce(
      new Error("请先阅读并同意用户协议与隐私政策。"),
    );

    render(
      <LegalReacceptanceGate>
        <p>protected content</p>
      </LegalReacceptanceGate>,
    );

    await screen.findByText("协议已更新");
    fireEvent.click(screen.getByRole("checkbox"));
    fireEvent.click(screen.getByRole("button", { name: "确认并继续" }));

    await waitFor(() => {
      expect(screen.getByText(/请先阅读并同意用户协议与隐私政策/)).toBeTruthy();
    });
  });
});
