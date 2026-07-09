"use client";

import type { ReactNode } from "react";

import { DesktopCenterLayout } from "@/components/desktop/DesktopCenterLayout";
import { DesktopOnlyGate } from "@/components/desktop/DesktopOnlyGate";

export default function DesktopLayout({ children }: { children: ReactNode }) {
  return (
    <DesktopOnlyGate>
      <DesktopCenterLayout>{children}</DesktopCenterLayout>
    </DesktopOnlyGate>
  );
}
