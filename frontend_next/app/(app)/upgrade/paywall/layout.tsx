"use client";

import type { ReactNode } from "react";

import { PricingRevampGate } from "@/components/billing/PricingRevampGate";

export default function PaywallLayout({ children }: { children: ReactNode }) {
  return <PricingRevampGate redirectTo="/dashboard">{children}</PricingRevampGate>;
}
