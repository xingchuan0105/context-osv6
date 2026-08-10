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

vi.mock("next/navigation", () => ({
  usePathname: () => "/help",
  useRouter: () => ({ push: vi.fn(), replace: vi.fn(), prefetch: vi.fn() }),
}));

// AppTopBar heavy leaves are covered by dashboard-surface tests; stub them here.
vi.mock("../../components/account-menu", () => ({ AccountMenu: () => null }));
vi.mock("../../components/notifications/notification-bell", () => ({
  NotificationBell: () => null,
}));
vi.mock("../../components/plan-entry", () => ({ PlanEntry: () => null }));

import HelpPage from "../../app/(app)/help/page";

describe("HelpPage", () => {
  it("renders the migrated help center content and primary links", () => {
    render(<HelpPage />);

    expect(screen.getByRole("heading", { name: "帮助中心" })).toBeTruthy();
    expect(screen.getByRole("link", { name: "返回 Dashboard" }).getAttribute("href")).toBe("/dashboard");
    expect(screen.getByRole("link", { name: "账户设置" }).getAttribute("href")).toBe("/settings?tab=profile");
    expect(screen.getByText("5. API 接入")).toBeTruthy();
    expect(screen.getByText(/资料上传、URL 导入和知识库查询/)).toBeTruthy();
  });
});
