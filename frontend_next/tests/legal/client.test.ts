import { describe, expect, it, vi } from "vitest";

import {
  PaymentConsentRequiredError,
  recordPaymentLegalAcceptance,
} from "@/lib/legal/client";

const requestMock = vi.hoisted(() => vi.fn());

vi.mock("@/lib/http/request", () => ({
  request: requestMock,
}));

describe("recordPaymentLegalAcceptance", () => {
  it("throws PaymentConsentRequiredError when not consented", async () => {
    await expect(recordPaymentLegalAcceptance("token-1", false)).rejects.toBeInstanceOf(
      PaymentConsentRequiredError,
    );
    expect(requestMock).not.toHaveBeenCalled();
  });

  it("records payment context when consented", async () => {
    requestMock.mockResolvedValueOnce({ success: true });

    await recordPaymentLegalAcceptance("token-1", true);

    expect(requestMock).toHaveBeenCalledWith(
      "/api/auth/legal-acceptance",
      expect.objectContaining({
        method: "POST",
        body: expect.stringContaining('"context":"payment"'),
      }),
      "token-1",
    );
  });
});
