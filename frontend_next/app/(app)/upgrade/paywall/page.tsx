"use client";

import { useSearchParams } from "next/navigation";

import { PaywallPageClient } from "./paywall-page-client";

export default function PaywallPage() {
  const searchParams = useSearchParams();
  const reason = searchParams.get("reason") === "7d" ? "7d" : "5h";

  return <PaywallPageClient reason={reason} />;
}
