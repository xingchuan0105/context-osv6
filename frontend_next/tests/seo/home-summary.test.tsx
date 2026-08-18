import { render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";

vi.mock("next/navigation", () => ({
  useRouter: () => ({ replace: vi.fn(), push: vi.fn(), prefetch: vi.fn() }),
}));

vi.mock("@/lib/runtime/tauri-ipc", () => ({
  isTauri: () => false,
}));

vi.mock("@/lib/desktop/tauri-license", () => ({
  getLicenseStatus: vi.fn().mockResolvedValue(undefined),
}));

vi.mock("@/lib/ui-preferences", () => ({
  useUiPreferences: () => ({ locale: "zh-CN" as const, theme: "system" as const }),
}));

import HomeClient from "@/app/home-client";

describe("HomeClient SSR 摘要（GEO 方案 A2）", () => {
  it("renders exactly one H1 with the value proposition, bullets and CTAs", () => {
    render(<HomeClient />);

    const headings = screen.getAllByRole("heading", { level: 1 });
    expect(headings).toHaveLength(1);
    expect(headings[0]?.textContent).toContain("可分享的个人知识工作区");

    const h2 = screen.getAllByRole("heading", { level: 2 });
    expect(h2.length).toBeGreaterThanOrEqual(3);
    expect(screen.getByRole("heading", { level: 2, name: "文档入库与问答" })).toBeTruthy();
    expect(screen.getByRole("heading", { level: 2, name: /外接 Agent/ })).toBeTruthy();
    expect(screen.getByRole("heading", { level: 2, name: /会员与可分享/ })).toBeTruthy();

    expect(screen.getByText(/回答可溯源到具体文档/)).toBeTruthy();
    expect(screen.getByText(/把知识库接到 Cursor、Claude/)).toBeTruthy();
    expect(screen.getByText(/访客免登录浏览公开库/)).toBeTruthy();
    expect(screen.getByText(/ContextLM/)).toBeTruthy();
    expect(screen.getByText(/2026-08-18/)).toBeTruthy();

    expect(screen.getByRole("link", { name: "进入应用" }).getAttribute("href")).toBe("/dashboard");
    expect(screen.getByRole("link", { name: "查看定价" }).getAttribute("href")).toBe("/pricing");
    expect(screen.getByRole("link", { name: "Agent 接入说明" }).getAttribute("href")).toBe(
      "/help/api-access/agents",
    );
  });
});

