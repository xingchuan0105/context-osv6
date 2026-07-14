"use client";

import { useRouter } from "next/navigation";
import { useEffect, useState } from "react";

import LegalFooterLinks from "@/components/legal/LegalFooterLinks";
import { isTauri } from "@/lib/runtime/tauri-ipc";
import { getLicenseStatus } from "@/lib/desktop/tauri-license";
import { AUTH_SESSION_COOKIE_NAME } from "../lib/auth/server-session";

function getCookie(name: string): string | null {
  if (typeof document === "undefined") {
    return null;
  }
  const match = document.cookie.split("; ").find((row) => row.startsWith(`${name}=`));
  return match ? match.split("=")[1] ?? null : null;
}

export default function HomePage() {
  const router = useRouter();
  const [label, setLabel] = useState("正在进入 Context-OS…");

  useEffect(() => {
    if (isTauri()) {
      setLabel("正在检查客户端许可…");
      void getLicenseStatus()
        .then((status) => {
          const open =
            status.kind === "trial" ||
            status.kind === "active" ||
            status.kind === "offline_grace";
          router.replace(open ? "/dashboard" : "/activate");
        })
        .catch(() => {
          router.replace("/activate");
        });
      return;
    }

    const hasAuthSession = getCookie(AUTH_SESSION_COOKIE_NAME) === "1";
    router.replace(hasAuthSession ? "/dashboard" : "/login");
  }, [router]);

  return (
    <div
      style={{
        display: "flex",
        minHeight: "100vh",
        alignItems: "center",
        justifyContent: "center",
        flexDirection: "column",
        gap: "1rem",
      }}
    >
      <div style={{ color: "hsl(var(--muted-foreground))", fontSize: "14px" }}>{label}</div>
      <LegalFooterLinks />
    </div>
  );
}
