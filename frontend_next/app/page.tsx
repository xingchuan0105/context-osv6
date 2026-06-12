"use client";

import { useRouter } from "next/navigation";
import { useEffect } from "react";

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

  useEffect(() => {
    const hasAuthSession = getCookie(AUTH_SESSION_COOKIE_NAME) === "1";
    router.replace(hasAuthSession ? "/dashboard" : "/login");
  }, [router]);

  return (
    <div style={{ display: "flex", minHeight: "100vh", alignItems: "center", justifyContent: "center" }}>
      <div style={{ color: "#6b7280", fontSize: "14px" }}>正在进入 Context OS...</div>
    </div>
  );
}
