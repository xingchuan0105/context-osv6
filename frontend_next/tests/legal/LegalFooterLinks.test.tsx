import { render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";

import LegalFooterLinks from "@/components/legal/LegalFooterLinks";

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

describe("LegalFooterLinks", () => {
  it("renders all three legal links", () => {
    render(<LegalFooterLinks />);

    expect(screen.getByText("用户协议")).toBeTruthy();
    expect(screen.getByText("隐私政策")).toBeTruthy();
    expect(screen.getByText("开源声明")).toBeTruthy();
  });

  it("has correct href for each link", () => {
    render(<LegalFooterLinks />);

    expect(screen.getByText("用户协议").closest("a")).toHaveAttribute(
      "href",
      "/legal/terms",
    );
    expect(screen.getByText("隐私政策").closest("a")).toHaveAttribute(
      "href",
      "/legal/privacy",
    );
    expect(screen.getByText("开源声明").closest("a")).toHaveAttribute(
      "href",
      "/legal/licenses",
    );
  });

  it("renders copyright with current year", () => {
    render(<LegalFooterLinks />);

    const currentYear = new Date().getFullYear();
    expect(screen.getByText(`© ${currentYear} Context-OS`)).toBeTruthy();
  });
});
