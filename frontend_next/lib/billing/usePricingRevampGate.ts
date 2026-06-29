"use client";

import { useEffect, useState } from "react";
import { useRouter } from "next/navigation";

import { useAuth } from "../auth/context";
import { isPricingRevampEnabled, isPricingRevampEnabledSSR } from "./featureFlag";

export type PricingRevampGateOptions = {
  redirectTo: string;
  /** Marketing /pricing only needs the SSR env gate; skip usage/window bucket probe. */
  requireUsageProbe?: boolean;
};

export type PricingRevampGateState = {
  ssrEnabled: boolean;
  ready: boolean;
  enabled: boolean;
};

export function usePricingRevampGate({
  redirectTo,
  requireUsageProbe = true,
}: PricingRevampGateOptions): PricingRevampGateState {
  const router = useRouter();
  const { token } = useAuth();
  const ssrEnabled = isPricingRevampEnabledSSR();
  const [probeState, setProbeState] = useState<"pending" | "passed" | "failed">(
    ssrEnabled ? (requireUsageProbe ? "pending" : "passed") : "failed",
  );

  useEffect(() => {
    if (!ssrEnabled) {
      router.replace(redirectTo);
      return;
    }

    if (!requireUsageProbe) {
      setProbeState("passed");
      return;
    }

    if (!token) {
      return;
    }

    let cancelled = false;

    void isPricingRevampEnabled(token).then((enabled) => {
      if (cancelled) {
        return;
      }
      if (!enabled) {
        setProbeState("failed");
        router.replace(redirectTo);
        return;
      }
      setProbeState("passed");
    });

    return () => {
      cancelled = true;
    };
  }, [redirectTo, requireUsageProbe, router, ssrEnabled, token]);

  return {
    ssrEnabled,
    ready: !ssrEnabled || probeState !== "pending",
    enabled: ssrEnabled && probeState === "passed",
  };
}
