"use client";

import type { ReactNode } from "react";

import {
  usePricingRevampGate,
  type PricingRevampGateOptions,
} from "../../lib/billing/usePricingRevampGate";

export function PricingRevampGate({
  redirectTo,
  children,
}: PricingRevampGateOptions & { children: ReactNode }) {
  const { ssrEnabled } = usePricingRevampGate({ redirectTo });

  if (!ssrEnabled) {
    return null;
  }

  return children;
}
