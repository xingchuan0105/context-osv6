import { render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";

import LegalLayout from "@/components/legal/LegalLayout";

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

describe("LegalLayout", () => {
  it("renders title and children", () => {
    render(
      <LegalLayout title="用户协议">
        <p>协议内容</p>
      </LegalLayout>,
    );

    expect(screen.getByText("用户协议")).toBeTruthy();
    expect(screen.getByText("协议内容")).toBeTruthy();
  });

  it("renders lastUpdated when provided", () => {
    render(
      <LegalLayout title="隐私政策" lastUpdated="2025-01-01">
        <p>内容</p>
      </LegalLayout>,
    );

    expect(screen.getByText(/最后更新: 2025-01-01/)).toBeTruthy();
  });

  it("renders version when provided", () => {
    render(
      <LegalLayout title="用户协议" version="v1.0">
        <p>内容</p>
      </LegalLayout>,
    );

    expect(screen.getByText(/版本: v1.0/)).toBeTruthy();
  });

  it("does not render lastUpdated or version when omitted", () => {
    render(
      <LegalLayout title="用户协议">
        <p>内容</p>
      </LegalLayout>,
    );

    expect(screen.queryByText(/最后更新/)).toBeNull();
    expect(screen.queryByText(/版本:/)).toBeNull();
  });

  it("renders back link to /legal", () => {
    render(
      <LegalLayout title="用户协议">
        <p>内容</p>
      </LegalLayout>,
    );

    const link = screen.getByText("返回法律中心");
    expect(link).toBeTruthy();
    expect(link.closest("a")).toHaveAttribute("href", "/legal");
  });
});
