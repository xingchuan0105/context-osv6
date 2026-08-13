"use client";

import { useEffect, useRef, useState, type ReactNode } from "react";

import { useAuth } from "@/lib/auth/context";
import {
  ensureLocalProduct,
  ensureLocalSession,
  ensureLocalStack,
} from "@/lib/desktop/tauri-local";
import { isTauri } from "@/lib/runtime/tauri-ipc";

function formatBootstrapError(err: unknown, fallback: string): string {
  if (err instanceof Error && err.message.trim()) {
    return err.message;
  }
  if (typeof err === "string" && err.trim()) {
    try {
      const parsed: unknown = JSON.parse(err);
      if (
        parsed &&
        typeof parsed === "object" &&
        "message" in parsed &&
        typeof (parsed as { message: unknown }).message === "string"
      ) {
        return (parsed as { message: string }).message;
      }
    } catch {
      return err;
    }
    return err;
  }
  if (err && typeof err === "object") {
    const rec = err as Record<string, unknown>;
    if (typeof rec.message === "string" && rec.message.trim()) {
      return rec.message;
    }
  }
  return fallback;
}

const BOOTSTRAP_TIMEOUT_MS = 120_000;

function withTimeout<T>(promise: Promise<T>, ms: number, label: string): Promise<T> {
  return new Promise<T>((resolve, reject) => {
    const t = window.setTimeout(() => {
      reject(new Error(`${label} 超时（${Math.round(ms / 1000)}s）。可关闭后重试，或查看 logs/ensure-native.log`));
    }, ms);
    promise.then(
      (v) => {
        window.clearTimeout(t);
        resolve(v);
      },
      (e: unknown) => {
        window.clearTimeout(t);
        reject(e);
      },
    );
  });
}

/**
 * Desktop cold start: data stack → product API/worker → local B2C session.
 * Never redirects to cloud /login.
 * Hard client-side timeout so a stuck IPC cannot freeze the shell forever.
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
    const started = Date.now();
    const tick = window.setInterval(() => {
      const sec = Math.round((Date.now() - started) / 1000);
      setDetail((prev) => {
        const base = prev.replace(/\s*（已等待 \d+s）$/, "");
        return `${base} （已等待 ${sec}s）`;
      });
    }, 1000);

    void (async () => {
      try {
        setDetail("正在启动本机数据面（PostgreSQL / Redis）…");
        const stack = await withTimeout(
          ensureLocalStack(),
          Math.min(BOOTSTRAP_TIMEOUT_MS, 95_000),
          "本机数据面",
        );
        if (!stack.ok) {
          throw new Error(stack.message || "本机数据面启动失败");
        }

        setDetail("正在启动本机产品进程（API / Worker）…");
        const product = await withTimeout(
          ensureLocalProduct(),
          Math.min(BOOTSTRAP_TIMEOUT_MS, 55_000),
          "本机产品进程",
        );
        if (!product.ok) {
          throw new Error(product.message || "本机产品进程启动失败");
        }

        setDetail("正在准备本机账户…");
        const session = await withTimeout(ensureLocalSession(), 30_000, "本机会话");
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
        setDetail(session.message || "本机会话未就绪（可稍后在设置中重试）");
        setPhase("error");
      } catch (err: unknown) {
        setDetail(formatBootstrapError(err, "本机会话初始化失败"));
        setPhase("error");
      } finally {
        window.clearInterval(tick);
      }
    })();

    return () => {
      window.clearInterval(tick);
    };
  }, [completeAuth, initialized, isAuthenticated]);

  // Block shell until cold-start finishes (or soft-fails with banner).
  if (isTauri() && phase === "working" && !isAuthenticated) {
    return (
      <main className="app-auth-shell">
        <section className="app-surface-card" style={{ maxWidth: "28rem", textAlign: "center" }}>
          <p style={{ margin: 0, color: "hsl(var(--muted-foreground))" }}>
            {detail || "正在准备本机环境…"}
          </p>
          <p
            style={{
              margin: "0.75rem 0 0",
              fontSize: "0.8rem",
              color: "hsl(var(--muted-foreground))",
            }}
          >
            首次启动可能需要数十秒；若超过 2 分钟仍无进展，请结束进程后重试。
          </p>
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
