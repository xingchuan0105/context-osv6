"use client";

import { useEffect, useRef, useState, type ReactNode } from "react";

import { useAuth } from "@/lib/auth/context";
import { ensureLocalSession } from "@/lib/desktop/tauri-llm";
import { isTauri } from "@/lib/runtime/tauri-ipc";

/**
 * After license allows workspace, ensure a local B2C personal session against
 * the on-machine product API (register/login local@context-os.client).
 * Never redirects to cloud /login.
 */
export function ClientLocalSessionBootstrap({ children }: { children: ReactNode }) {
  const { completeAuth, isAuthenticated, initialized } = useAuth();
  const [phase, setPhase] = useState<"idle" | "working" | "done" | "error">("idle");
  const [detail, setDetail] = useState("");
  const attempted = useRef(false);

  useEffect(() => {
    if (!isTauri() || !initialized || isAuthenticated || attempted.current) {
      if (isAuthenticated) {
        setPhase("done");
      }
      return;
    }

    attempted.current = true;
    setPhase("working");
    setDetail("正在准备本机账户…");

    void ensureLocalSession()
      .then((session) => {
        if (session.ready && session.token && session.user) {
          completeAuth({
            token: session.token,
            user: {
              id: session.user.id,
              email: session.user.email,
              full_name: session.user.full_name,
            },
            reset_ticket: null,
          });
          setDetail(session.message);
          setPhase("done");
          return;
        }
        setDetail(session.message || "本机会话未就绪（可稍后在设置中启动产品进程）");
        setPhase("error");
      })
      .catch((err: unknown) => {
        setDetail(err instanceof Error ? err.message : "本机会话初始化失败");
        setPhase("error");
      });
  }, [completeAuth, initialized, isAuthenticated]);

  // Soft fail: still render children so BYOK chat / license UI works without API.
  if (isTauri() && phase === "working" && !isAuthenticated) {
    return (
      <main className="app-auth-shell">
        <section className="app-surface-card" style={{ maxWidth: "28rem", textAlign: "center" }}>
          <p style={{ margin: 0, color: "hsl(var(--muted-foreground))" }}>{detail || "准备本机账户…"}</p>
        </section>
      </main>
    );
  }

  return (
    <>
      {isTauri() && phase === "error" && !isAuthenticated ? (
        <div
          role="status"
          style={{
            padding: "0.5rem 1rem",
            fontSize: "0.85rem",
            color: "hsl(var(--muted-foreground))",
            borderBottom: "1px solid hsl(var(--border))",
          }}
        >
          本机产品会话未就绪：{detail}
        </div>
      ) : null}
      {children}
    </>
  );
}
