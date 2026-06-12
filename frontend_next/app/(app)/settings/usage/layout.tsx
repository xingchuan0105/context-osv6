"use client";

import type { ReactNode } from "react";

import { PricingRevampGate } from "@/components/billing/PricingRevampGate";

export default function UsageLayout({ children }: { children: ReactNode }) {
  return <PricingRevampGate redirectTo="/settings">{children}</PricingRevampGate>;
}
