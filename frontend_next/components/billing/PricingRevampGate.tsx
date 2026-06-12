"use client";

import type { ReactNode } from "react";

import type { PricingRevampGateOptions } from "../../lib/billing/usePricingRevampGate";
import { PricingRevampGateProvider } from "./pricing-revamp-gate-context";

export function PricingRevampGate({
  redirectTo,
  children,
}: PricingRevampGateOptions & { children: ReactNode }) {
  return <PricingRevampGateProvider redirectTo={redirectTo}>{children}</PricingRevampGateProvider>;
}

export { usePricingRevampGateResult } from "./pricing-revamp-gate-context";
