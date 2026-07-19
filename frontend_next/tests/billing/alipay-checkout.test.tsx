import { act, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import { PricingPageClient } from "../../app/(marketing)/pricing/pricing-page-client";
import { AlipayQrDialog } from "../../components/billing/AlipayQrDialog";

const mockPlans = vi.hoisted(() => globalThis.__mockProviders.createPricingPageMockPlans());
const harness = vi.hoisted(() => ({
  locale: "zh-CN" as "zh-CN" | "en",
  pushMock: vi.fn(),
}));

vi.mock("../../lib/auth/context", () => ({
  useAuth: () => ({ token: "token-1", user: { id: "u1" } }),
}));

vi.mock("../../lib/ui-preferences", () => ({
  useUiPreferences: () => ({ locale: harness.locale }),
}));

vi.mock("next/navigation", () => ({
  useRouter: () => ({ push: harness.pushMock }),
}));

vi.mock("../../lib/settings/client", () => ({
  createCheckoutSession: vi.fn(),
  getBillingOrderStatus: vi.fn(),
}));

vi.mock("../../lib/legal/client", () => ({
  recordPaymentLegalAcceptance: vi.fn().mockResolvedValue(undefined),
  PaymentConsentRequiredError: class PaymentConsentRequiredError extends Error {
    constructor(message = "请先阅读并同意用户协议与隐私政策") {
      super(message);
      this.name = "PaymentConsentRequiredError";
    }
  },
}));

vi.mock("../../lib/billing/api", () => ({
  billingApi: {
    getPlans: vi.fn().mockResolvedValue({ plans: mockPlans, current_plan_id: "free" }),
  },
}));

vi.mock("qrcode", () => ({
  default: { toDataURL: vi.fn().mockResolvedValue("data:image/png;base64,mock-qr") },
  toDataURL: vi.fn().mockResolvedValue("data:image/png;base64,mock-qr"),
}));

import { billingApi } from "../../lib/billing/api";
import { createCheckoutSession, getBillingOrderStatus } from "../../lib/settings/client";

beforeEach(() => {
  vi.clearAllMocks();
  harness.locale = "zh-CN";
});

afterEach(() => {
  vi.useRealTimers();
});

describe("Alipay QR checkout (pricing page)", () => {
  it("checks out with provider=alipay under zh-CN and renders the QR dialog", async () => {
    vi.mocked(createCheckoutSession).mockResolvedValue({
      url: "",
      session_id: "session-1",
      qr_code: "alipay-qr-string",
      order_id: "order-1",
    });
    vi.mocked(getBillingOrderStatus).mockResolvedValue({
      order_id: "order-1",
      status: "pending",
      plan_id: "plus",
    });

    render(<PricingPageClient />);
    fireEvent.click(screen.getByRole("button", { name: /升级 Plus/ }));

    await waitFor(() => {
      expect(createCheckoutSession).toHaveBeenCalledWith("token-1", {
        plan_id: "plus",
        provider: "alipay",
      });
    });
    expect(await screen.findByText("支付宝扫码支付")).toBeTruthy();
    expect(screen.getByRole("img", { name: "支付宝扫码支付" }).getAttribute("src")).toContain(
      "data:image",
    );
    expect(screen.getByText(/等待支付确认/)).toBeTruthy();
    expect(harness.pushMock).not.toHaveBeenCalled();
  });

  it("stops polling, shows the success notice and refetches plans once the order is paid", async () => {
    vi.useFakeTimers();
    vi.mocked(createCheckoutSession).mockResolvedValue({
      url: "",
      session_id: "session-1",
      qr_code: "alipay-qr-string",
      order_id: "order-1",
    });
    vi.mocked(getBillingOrderStatus).mockResolvedValue({
      order_id: "order-1",
      status: "paid",
      plan_id: "plus",
    });

    render(<PricingPageClient />);
    fireEvent.click(screen.getByRole("button", { name: /升级 Plus/ }));
    await act(async () => {
      await vi.advanceTimersByTimeAsync(10);
    });
    expect(screen.getByText("支付宝扫码支付")).toBeTruthy();

    await act(async () => {
      await vi.advanceTimersByTimeAsync(2_000);
    });

    expect(getBillingOrderStatus).toHaveBeenCalledWith("token-1", "order-1");
    expect(screen.queryByRole("dialog")).toBeNull();
    expect(screen.getByRole("status")).toHaveTextContent("支付成功");
    // Initial load + refetch after payment.
    expect(vi.mocked(billingApi.getPlans).mock.calls.length).toBe(2);
  });

  it("keeps redirecting to the checkout url under en locale", async () => {
    harness.locale = "en";
    vi.mocked(createCheckoutSession).mockResolvedValue({
      url: "https://checkout.example/session-1",
      session_id: "session-1",
    });

    render(<PricingPageClient />);
    fireEvent.click(screen.getByRole("button", { name: /Upgrade to Plus/ }));

    await waitFor(() => {
      expect(harness.pushMock).toHaveBeenCalledWith("https://checkout.example/session-1");
    });
    expect(createCheckoutSession).toHaveBeenCalledWith("token-1", {
      plan_id: "plus",
      provider: "creem",
    });
    expect(screen.queryByRole("dialog")).toBeNull();
  });
});

describe("AlipayQrDialog", () => {
  function renderDialog(overrides?: Partial<Parameters<typeof AlipayQrDialog>[0]>) {
    const props = {
      token: "token-1",
      qrCode: "alipay-qr-string",
      orderId: "order-1",
      planName: "Plus",
      priceLabel: "¥49 / 月",
      locale: "zh-CN" as const,
      onPaid: vi.fn(),
      onCancel: vi.fn(),
      ...overrides,
    };
    render(<AlipayQrDialog {...props} />);
    return props;
  }

  it("fires onPaid once the order status turns paid", async () => {
    vi.useFakeTimers();
    vi.mocked(getBillingOrderStatus).mockResolvedValue({
      order_id: "order-1",
      status: "paid",
      plan_id: "plus",
    });
    const props = renderDialog();

    await act(async () => {
      await vi.advanceTimersByTimeAsync(2_000);
    });

    expect(props.onPaid).toHaveBeenCalledTimes(1);
    expect(screen.getByRole("status")).toHaveTextContent("支付成功");
  });

  it("stops polling and fires onCancel when the user cancels", async () => {
    vi.useFakeTimers();
    vi.mocked(getBillingOrderStatus).mockResolvedValue({
      order_id: "order-1",
      status: "pending",
      plan_id: "plus",
    });
    const props = renderDialog();

    fireEvent.click(screen.getByRole("button", { name: "取消支付" }));
    expect(props.onCancel).toHaveBeenCalledTimes(1);

    await act(async () => {
      await vi.advanceTimersByTimeAsync(10_000);
    });
    expect(getBillingOrderStatus).not.toHaveBeenCalled();
    expect(props.onPaid).not.toHaveBeenCalled();
  });

  it("shows the timeout copy after 5 minutes without payment", async () => {
    vi.useFakeTimers();
    vi.mocked(getBillingOrderStatus).mockResolvedValue({
      order_id: "order-1",
      status: "pending",
      plan_id: "plus",
    });
    renderDialog();

    await act(async () => {
      await vi.advanceTimersByTimeAsync(5 * 60_000 + 2_000);
    });

    expect(screen.getByRole("alert")).toHaveTextContent(/支付超时/);
  });
});
