import { render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";

import LicensesSummary from "@/app/(marketing)/legal/licenses/page";

vi.mock("@/lib/ui-preferences", () => ({
  useUiPreferences: () => ({ locale: "zh-CN" as const, theme: "system" as const }),
}));

vi.mock("next/link", () => {
  return {
    default: ({
      children,
      href,
      ...props
    }: {
      children: React.ReactNode;
      href: string;
    }) => (
      <a href={href} {...props}>
        {children}
      </a>
    ),
  };
});

describe("LicensesSummary", () => {
  it("renders the page title", () => {
    render(<LicensesSummary />);
    expect(screen.getByText("开源软件说明")).toBeTruthy();
  });

  it("renders all major component categories", () => {
    render(<LicensesSummary />);
    expect(screen.getByText("Web框架")).toBeTruthy();
    expect(screen.getByText("后端运行时")).toBeTruthy();
    expect(screen.getByText("向量检索")).toBeTruthy();
    expect(screen.getByText("文档解析")).toBeTruthy();
    expect(screen.getByText("客户端壳")).toBeTruthy();
    expect(screen.getByText("AI推理")).toBeTruthy();
  });

  it("renders license badges", () => {
    render(<LicensesSummary />);
    expect(screen.getAllByText("MIT").length).toBeGreaterThanOrEqual(2);
    expect(screen.getByText("商业API")).toBeTruthy();
    expect(screen.getByText(/markitdown/)).toBeTruthy();
    expect(screen.getByText(/firecrawl-anydoc/)).toBeTruthy();
    expect(screen.getAllByText(/pgvector/).length).toBeGreaterThanOrEqual(1);
  });

  it("renders weak copyleft section", () => {
    render(<LicensesSummary />);
    expect(screen.getByText("dompurify")).toBeTruthy();
    expect(screen.getByText(/选择Apache-2.0版本/)).toBeTruthy();
    expect(screen.getByText("cssparser")).toBeTruthy();
    expect(screen.getByText(/MPL，未修改则仅需NOTICE/)).toBeTruthy();
    expect(screen.getByText(/MinIO/)).toBeTruthy();
  });

  it("renders links to full third-party notices", () => {
    render(<LicensesSummary />);
    const fullLink = screen.getByText("查看完整第三方声明");
    expect(fullLink.closest("a")).toHaveAttribute("href", "/legal/licenses/third-party");

    const downloadLink = screen.getByText("下载Markdown");
    expect(downloadLink.closest("a")).toHaveAttribute("href", "/legal/third-party-notices.md");
    expect(downloadLink.closest("a")).toHaveAttribute("download");
  });

  it("renders MIT license link", () => {
    render(<LicensesSummary />);
    const link = screen.getByText("查看MIT许可证全文");
    expect(link.closest("a")).toHaveAttribute("href", "/legal/licenses/project");
  });

  it("renders desktop client section", () => {
    render(<LicensesSummary />);
    expect(screen.getByText("客户端")).toBeTruthy();
    expect(screen.getByText(/runtime\/THIRD_PARTY\.txt/)).toBeTruthy();
    expect(screen.getByText(/BSD-3-Clause/)).toBeTruthy();
  });
});
