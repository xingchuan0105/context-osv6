"use client";

import { useEffect } from "react";
import { useRouter } from "next/navigation";

/**
 * `/setup` is retired: BYOK config lives only in `/settings?tab=providers`
 * (canonical, PRODUCT_IA §2). Keep the route as a redirect so stale links
 * land on the provider surface instead of 404.
 */
export default function SetupRedirect() {
  const router = useRouter();

  useEffect(() => {
    router.replace("/settings?tab=providers");
  }, [router]);

  return null;
}
