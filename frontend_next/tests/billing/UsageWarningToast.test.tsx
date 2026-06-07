import { render, screen, fireEvent } from "@testing-library/react";
import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";
import { UsageWarningToast } from "../../components/billing/UsageWarningToast";

beforeEach(() => {
  localStorage.clear();
  vi.useFakeTimers();
});

afterEach(() => {
  vi.useRealTimers();
});

describe("UsageWarningToast", () => {
  it("renders actual percentage with upgrade link", () => {
    render(
      <UsageWarningToast
        threshold={80}
        percentage={85}
        windowType="5h"
        locale="zh-CN"
        userId="user_123"
        used={85000}
        limit={100000}
        resetAt="2026-06-07T20:00:00Z"
        onDismiss={() => {}}
        onUpgradeClick={() => {}}
      />,
    );
    expect(screen.getByText(/85%/)).toBeTruthy();
    expect(screen.getByText(/升级 Plus 解锁 6× 用量/)).toBeTruthy();
  });

  it("does not render if already dismissed for this window in localStorage", () => {
    localStorage.setItem("toast_dismissed_user_123_5h_80", "true");
    const { container } = render(
      <UsageWarningToast
        threshold={80}
        percentage={85}
        windowType="5h"
        locale="zh-CN"
        userId="user_123"
        used={85000}
        limit={100000}
        resetAt="2026-06-07T20:00:00Z"
        onDismiss={() => {}}
      />,
    );
    expect(container.firstChild).toBeNull();
  });

  it("writes localStorage with user_id + windowType + threshold on dismiss", () => {
    const onDismiss = vi.fn();
    render(
      <UsageWarningToast
        threshold={80}
        percentage={85}
        windowType="5h"
        locale="zh-CN"
        userId="user_abc"
        used={85000}
        limit={100000}
        resetAt="2026-06-07T20:00:00Z"
        onDismiss={onDismiss}
      />,
    );
    fireEvent.click(screen.getByLabelText("关闭"));
    expect(localStorage.getItem("toast_dismissed_user_abc_5h_80")).toBe("true");
    expect(onDismiss).toHaveBeenCalled();
  });
});
