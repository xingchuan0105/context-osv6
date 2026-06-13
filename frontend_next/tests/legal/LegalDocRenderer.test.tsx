import { render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";

import LegalDocRenderer from "@/components/legal/LegalDocRenderer";

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

describe("LegalDocRenderer", () => {
  it("renders HTML content inside layout", () => {
    render(
      <LegalDocRenderer
        content="<p>这是一段法律文本</p>"
        title="用户协议"
      />,
    );

    expect(screen.getByText("用户协议")).toBeTruthy();
    expect(screen.getByText("这是一段法律文本")).toBeTruthy();
  });

  it("passes lastUpdated and version to layout", () => {
    render(
      <LegalDocRenderer
        content="<p>内容</p>"
        title="隐私政策"
        lastUpdated="2025-06-01"
        version="v2.0"
      />,
    );

    expect(screen.getByText(/最后更新: 2025-06-01/)).toBeTruthy();
    expect(screen.getByText(/版本: v2.0/)).toBeTruthy();
  });

  it("renders content with dangerouslySetInnerHTML", () => {
    const html = "<h2>第一条</h2><p>服务条款内容</p>";
    render(
      <LegalDocRenderer content={html} title="用户协议" />,
    );

    expect(screen.getByText("第一条")).toBeTruthy();
    expect(screen.getByText("服务条款内容")).toBeTruthy();
  });
});
