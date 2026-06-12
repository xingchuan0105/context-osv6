"use client";

/**
 * 客户端 i18n 配置
 *
 * 替代 i18n/request.ts 的服务端 locale 获取，用于桌面端静态导出场景。
 * 静态导出不支持 cookies() 等服务端 API，所以需要客户端读取 locale。
 */

import { NextIntlClientProvider } from "next-intl";
import { useEffect, useState } from "react";

import { DEFAULT_LOCALE, LOCALE_COOKIE_NAME, normalizeLocale, type UiLocale } from "../i18n/config";
import { getMessageCatalog } from "../i18n/messages";

function getCookie(name: string): string | null {
  if (typeof document === "undefined") {
    return null;
  }

  const match = document.cookie
    .split("; ")
    .find((row) => row.startsWith(`${name}=`));

  return match ? match.split("=")[1] ?? null : null;
}

function getStoredLocale(): UiLocale {
  const cookieLocale = getCookie(LOCALE_COOKIE_NAME);
  return normalizeLocale(cookieLocale);
}

/**
 * 客户端 i18n Provider
 *
 * 用于桌面端静态导出场景，替代服务端的 getRequestConfig。
 * 从 cookie 或 localStorage 读取 locale，加载对应的 message catalog。
 *
 * 用法：
 * ```tsx
 * // app/layout.tsx（桌面端版本）
 * import { ClientI18nProvider } from "@/lib/runtime/client-i18n";
 *
 * export default function RootLayout({ children }) {
 *   return (
 *     <html>
 *       <body>
 *         <ClientI18nProvider>{children}</ClientI18nProvider>
 *       </body>
 *     </html>
 *   );
 * }
 * ```
 */
export function ClientI18nProvider({ children }: { children: React.ReactNode }) {
  const [locale, setLocale] = useState<UiLocale>(DEFAULT_LOCALE);
  const [messages, setMessages] = useState(() => getMessageCatalog(DEFAULT_LOCALE));
  const [isInitialized, setIsInitialized] = useState(false);

  useEffect(() => {
    // 客户端读取 locale
    const storedLocale = getStoredLocale();
    setLocale(storedLocale);
    setMessages(getMessageCatalog(storedLocale));
    setIsInitialized(true);

    // 监听同 tab 内的 locale 切换。
    // 浏览器原生 "storage" 事件仅在其它 tab 触发，同 tab 需用自定义事件。
    const handleLocaleChange = () => {
      const newLocale = getStoredLocale();
      setLocale(newLocale);
      setMessages(getMessageCatalog(newLocale));
    };

    window.addEventListener("locale-change", handleLocaleChange);
    return () => window.removeEventListener("locale-change", handleLocaleChange);
  }, []);

  // 初始化完成前显示加载状态
  if (!isInitialized) {
    return null;
  }

  return (
    <NextIntlClientProvider locale={locale} messages={messages}>
      {children}
    </NextIntlClientProvider>
  );
}

/**
 * 切换 locale 的工具函数
 *
 * 更新 cookie 并触发重新渲染
 */
export function setLocale(locale: UiLocale): void {
  if (typeof document === "undefined") {
    return;
  }

  document.cookie = `${LOCALE_COOKIE_NAME}=${locale};path=/;max-age=${365 * 24 * 60 * 60}`;
  window.dispatchEvent(new CustomEvent("locale-change", { detail: locale }));
}
