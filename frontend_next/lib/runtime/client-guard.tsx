"use client";

/**
 * 客户端认证守卫
 *
 * 替代 middleware.ts 的登录态跳转逻辑，用于桌面端静态导出场景。
 * 静态导出不运行 middleware，所以需要在客户端进行认证检查和重定向。
 */

import { useEffect, useState } from "react";
import { usePathname, useRouter } from "next/navigation";

import { AUTH_SESSION_COOKIE_NAME } from "../auth/server-session";

/** 需要登录才能访问的路径前缀 */
const PROTECTED_PREFIXES = ["/dashboard", "/workspace", "/settings", "/admin", "/invite"];

/** 登录/注册页面路径 */
const AUTH_PATHS = ["/login", "/register", "/forgot-password"];

/** 不需要认证检查的路径（公开页面） */
const PUBLIC_PATHS = ["/", "/help", "/pricing", "/shared"];

function isProtectedPath(pathname: string): boolean {
  return PROTECTED_PREFIXES.some((prefix) => pathname.startsWith(prefix));
}

function isAuthPath(pathname: string): boolean {
  return AUTH_PATHS.some((path) => pathname.startsWith(path));
}

function isPublicPath(pathname: string): boolean {
  return PUBLIC_PATHS.some((path) => pathname === path || pathname.startsWith(path + "/"));
}

function getCookie(name: string): string | null {
  if (typeof document === "undefined") {
    return null;
  }

  const match = document.cookie
    .split("; ")
    .find((row) => row.startsWith(`${name}=`));

  return match ? match.split("=")[1] ?? null : null;
}

function hasSession(): boolean {
  return getCookie(AUTH_SESSION_COOKIE_NAME) !== null;
}

/**
 * 客户端认证守卫组件
 *
 * 用法：在需要保护的布局中包裹子组件
 *
 * ```tsx
 * // app/(app)/layout.tsx
 * import { ClientAuthGuard } from "@/lib/runtime/client-guard";
 *
 * export default function AppLayout({ children }) {
 *   return <ClientAuthGuard>{children}</ClientAuthGuard>;
 * }
 * ```
 */
export function ClientAuthGuard({ children }: { children: React.ReactNode }) {
  const pathname = usePathname();
  const router = useRouter();
  const [isChecking, setIsChecking] = useState(true);

  useEffect(() => {
    // 跳过公开路径和登录页面
    if (isPublicPath(pathname) || isAuthPath(pathname)) {
      setIsChecking(false);
      return;
    }

    // 检查登录态
    if (!hasSession()) {
      // 未登录，重定向到登录页
      router.replace("/login");
      return;
    }

    setIsChecking(false);
  }, [pathname, router]);

  // 正在检查或已重定向时显示加载状态
  if (isChecking && isProtectedPath(pathname)) {
    return (
      <div className="flex min-h-screen items-center justify-center">
        <div className="text-muted-foreground text-sm">正在检查登录状态...</div>
      </div>
    );
  }

  return <>{children}</>;
}

/**
 * 客户端重定向组件
 *
 * 用于已登录用户访问登录/注册页面时，自动重定向到 dashboard
 *
 * ```tsx
 * // app/login/page.tsx
 * import { AuthRedirect } from "@/lib/runtime/client-guard";
 *
 * export default function LoginPage() {
 *   return (
 *     <AuthRedirect>
 *       <LoginForm />
 *     </AuthRedirect>
 *   );
 * }
 * ```
 */
export function AuthRedirect({ children }: { children: React.ReactNode }) {
  const router = useRouter();
  const [isChecking, setIsChecking] = useState(true);

  useEffect(() => {
    if (hasSession()) {
      // 已登录，重定向到 dashboard
      router.replace("/dashboard");
      return;
    }

    setIsChecking(false);
  }, [router]);

  if (isChecking) {
    return (
      <div className="flex min-h-screen items-center justify-center">
        <div className="text-muted-foreground text-sm">正在检查登录状态...</div>
      </div>
    );
  }

  return <>{children}</>;
}
