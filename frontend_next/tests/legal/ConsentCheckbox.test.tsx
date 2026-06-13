import { fireEvent, render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";

import ConsentCheckbox from "@/components/legal/ConsentCheckbox";

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

describe("ConsentCheckbox", () => {
  it("renders checkbox and consent text", () => {
    render(<ConsentCheckbox onConsentChange={vi.fn()} />);

    expect(screen.getByRole("checkbox")).toBeTruthy();
    expect(screen.getByText(/我已阅读并同意/)).toBeTruthy();
  });

  it("renders links to terms and privacy pages", () => {
    render(<ConsentCheckbox onConsentChange={vi.fn()} />);

    const termsLink = screen.getByText("《用户服务协议》").closest("a");
    expect(termsLink).toHaveAttribute("href", "/legal/terms");
    expect(termsLink).toHaveAttribute("target", "_blank");

    const privacyLink = screen.getByText("《隐私政策》").closest("a");
    expect(privacyLink).toHaveAttribute("href", "/legal/privacy");
    expect(privacyLink).toHaveAttribute("target", "_blank");
  });

  it("calls onConsentChange with true when checked", () => {
    const onConsentChange = vi.fn();
    render(<ConsentCheckbox onConsentChange={onConsentChange} />);

    fireEvent.click(screen.getByRole("checkbox"));

    expect(onConsentChange).toHaveBeenCalledWith(true);
  });

  it("calls onConsentChange with false when unchecked", () => {
    const onConsentChange = vi.fn();
    render(<ConsentCheckbox onConsentChange={onConsentChange} />);

    fireEvent.click(screen.getByRole("checkbox"));
    fireEvent.click(screen.getByRole("checkbox"));

    expect(onConsentChange).toHaveBeenLastCalledWith(false);
  });

  it("renders hidden fields with default version values", () => {
    render(<ConsentCheckbox onConsentChange={vi.fn()} />);

    const termsVersion = document.querySelector(
      'input[name="terms_version"]',
    ) as HTMLInputElement;
    const privacyVersion = document.querySelector(
      'input[name="privacy_version"]',
    ) as HTMLInputElement;
    const acceptedAt = document.querySelector(
      'input[name="accepted_at"]',
    ) as HTMLInputElement;

    expect(termsVersion.value).toBe("2026-06-13");
    expect(privacyVersion.value).toBe("2026-06-13");
    expect(acceptedAt.value).toBeTruthy();
  });

  it("renders hidden fields with custom version values", () => {
    render(
      <ConsentCheckbox
        onConsentChange={vi.fn()}
        termsVersion="2025-01-01"
        privacyVersion="2025-02-01"
      />,
    );

    const termsVersion = document.querySelector(
      'input[name="terms_version"]',
    ) as HTMLInputElement;
    const privacyVersion = document.querySelector(
      'input[name="privacy_version"]',
    ) as HTMLInputElement;

    expect(termsVersion.value).toBe("2025-01-01");
    expect(privacyVersion.value).toBe("2025-02-01");
  });

  it("sets checkbox as required by default", () => {
    render(<ConsentCheckbox onConsentChange={vi.fn()} />);

    expect(screen.getByRole("checkbox")).toHaveAttribute("required");
  });

  it("does not set checkbox as required when required=false", () => {
    render(<ConsentCheckbox onConsentChange={vi.fn()} required={false} />);

    expect(screen.getByRole("checkbox")).not.toHaveAttribute("required");
  });
});
