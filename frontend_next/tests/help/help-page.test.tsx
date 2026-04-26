import { render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";

vi.mock("../../lib/ui-preferences", () => ({
  useUiPreferences: () => ({
    locale: "zh-CN" as const,
    theme: "system" as const,
    setLocale: vi.fn(),
    setTheme: vi.fn(),
  }),
}));

import HelpPage from "../../app/(app)/help/page";

describe("HelpPage", () => {
  it("renders the migrated help center content and primary links", () => {
    render(<HelpPage />);

    expect(screen.getByRole("heading", { name: "帮助中心" })).toBeTruthy();
    expect(screen.getByRole("link", { name: "返回 Dashboard" }).getAttribute("href")).toBe("/dashboard");
    expect(screen.getByRole("link", { name: "账户设置" }).getAttribute("href")).toBe("/settings?tab=profile");
    expect(screen.getByText("5. API 接入")).toBeTruthy();
    expect(screen.getByText(/资料上传、URL 导入和 RAG 查询/)).toBeTruthy();
  });
});
