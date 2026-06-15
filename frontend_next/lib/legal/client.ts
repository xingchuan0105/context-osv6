import { ApiError, request } from "../http/request";
import {
  PUBLISHED_PRIVACY_VERSION,
  PUBLISHED_TERMS_VERSION,
} from "./versions";

export type LegalAcceptanceContext = "payment" | "re_acceptance";

export type LegalStatus = {
  needs_re_acceptance: boolean;
  accepted_terms_version?: string | null;
  accepted_privacy_version?: string | null;
  published_terms_version: string;
  published_privacy_version: string;
};

type LegalStatusEnvelope = {
  success: boolean;
  data: LegalStatus | null;
  error?: string | null;
};

type LegalAcceptanceEnvelope = {
  success: boolean;
  error?: string | null;
};

export async function fetchLegalStatus(token: string): Promise<LegalStatus> {
  const response = await request<LegalStatusEnvelope>(
    "/api/auth/legal-status",
    { method: "GET" },
    token,
  );
  if (!response.success || !response.data) {
    const code = response.error?.trim() || null;
    throw new ApiError(400, code, code ?? "Failed to load legal status");
  }
  return response.data;
}

export async function recordLegalAcceptance(
  token: string,
  context: LegalAcceptanceContext,
): Promise<void> {
  const response = await request<LegalAcceptanceEnvelope>(
    "/api/auth/legal-acceptance",
    {
      method: "POST",
      body: JSON.stringify({
        terms_version: PUBLISHED_TERMS_VERSION,
        privacy_version: PUBLISHED_PRIVACY_VERSION,
        context,
      }),
    },
    token,
  );
  if (!response.success) {
    const code = response.error?.trim() || null;
    throw new ApiError(400, code, code ?? "Failed to record legal acceptance");
  }
}

export class PaymentConsentRequiredError extends Error {
  readonly code = "payment_consent_required";

  constructor(message = "Payment consent required") {
    super(message);
    this.name = "PaymentConsentRequiredError";
  }
}

export async function recordPaymentLegalAcceptance(
  token: string,
  consented: boolean,
): Promise<void> {
  if (!consented) {
    throw new PaymentConsentRequiredError();
  }
  await recordLegalAcceptance(token, "payment");
}
