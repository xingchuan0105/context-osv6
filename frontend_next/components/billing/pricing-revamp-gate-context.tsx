"use client";

import { createContext, useContext, type ReactNode } from "react";

import {
  usePricingRevampGate,
  type PricingRevampGateOptions,
  type PricingRevampGateState,
} from "../../lib/billing/usePricingRevampGate";

const PricingRevampGateContext = createContext<PricingRevampGateState | null>(null);

export function PricingRevampGateProvider({
  redirectTo,
  requireUsageProbe,
  children,
}: PricingRevampGateOptions & { children: ReactNode }) {
  const state = usePricingRevampGate({ redirectTo, requireUsageProbe });

  if (!state.ssrEnabled) {
    return null;
  }

  return (
    <PricingRevampGateContext.Provider value={state}>{children}</PricingRevampGateContext.Provider>
  );
}

export function usePricingRevampGateResult(): PricingRevampGateState {
  const state = useContext(PricingRevampGateContext);

  if (!state) {
    throw new Error("usePricingRevampGateResult must be used within PricingRevampGateProvider");
  }

  return state;
}
