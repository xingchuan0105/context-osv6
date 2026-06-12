import type { createUsagePageMocks } from "../../helpers/mock-providers";

export function resetUsagePageMocks(mocks: ReturnType<typeof createUsagePageMocks>) {
  mocks.pushMock.mockReset();
  mocks.replaceMock.mockReset();
  mocks.isPricingRevampEnabledMock.mockReset();
  mocks.isPricingRevampEnabledMock.mockResolvedValue(true);
}
