import type { Metadata } from "next";
import type { ReactNode } from "react";
import { Space_Grotesk, IBM_Plex_Sans, JetBrains_Mono } from "next/font/google";

import "./globals.css";
import { AuthProvider } from "../lib/auth/context";
import { normalizeLocale } from "../lib/i18n/config";
import { QueryProvider } from "../lib/query/provider";
import { ClientI18nProvider } from "../lib/runtime/client-i18n";
import { UiPreferencesProvider } from "../lib/ui-preferences";

const spaceGrotesk = Space_Grotesk({
  subsets: ["latin"],
  variable: "--font-heading",
  display: "swap",
});

const ibmPlexSans = IBM_Plex_Sans({
  weight: ["400", "500", "600"],
  subsets: ["latin"],
  variable: "--font-body",
  display: "swap",
});

const jetbrainsMono = JetBrains_Mono({
  subsets: ["latin"],
  variable: "--font-mono",
  display: "swap",
});

const siteUrl = process.env.NEXT_PUBLIC_SITE_URL?.trim() || "http://localhost:3000";

export const metadata: Metadata = {
  metadataBase: new URL(siteUrl),
  title: {
    default: "Context OS",
    template: "%s · Context OS",
  },
  description: "Second-brain workspace for organizing, distributing, and querying knowledge with AI.",
  icons: {
    icon: "/icon.svg",
    shortcut: "/icon.svg",
    apple: "/apple-icon",
  },
  manifest: "/manifest.webmanifest",
  openGraph: {
    title: "Context OS",
    description: "Second-brain workspace for organizing, distributing, and querying knowledge with AI.",
    siteName: "Context OS",
    images: [
      {
        url: "/opengraph-image",
        width: 1200,
        height: 630,
        alt: "Context OS",
      },
    ],
  },
  twitter: {
    card: "summary_large_image",
    title: "Context OS",
    description: "Second-brain workspace for organizing, distributing, and querying knowledge with AI.",
    images: ["/twitter-image"],
  },
};

// 桌面端构建时使用客户端 i18n Provider
const isDesktopBuild = process.env.BUILD_TARGET === "desktop";

export default async function RootLayout({ children }: { children: ReactNode }) {
  // 桌面端：使用默认 locale，客户端会从 cookie 读取
  // Web 端：使用服务端获取的 locale
  let locale: "zh-CN" | "en" = "zh-CN";
  let messages = {};

  if (!isDesktopBuild) {
    try {
      const { getLocale, getMessages } = await import("next-intl/server");
      locale = normalizeLocale(await getLocale());
      messages = await getMessages();
    } catch {
      // 降级到默认值
    }
  }

  // 桌面端：使用客户端 i18n Provider
  if (isDesktopBuild) {
    return (
      <html lang={locale} suppressHydrationWarning>
        <body className={`${spaceGrotesk.variable} ${ibmPlexSans.variable} ${jetbrainsMono.variable}`}>
          <QueryProvider>
            <ClientI18nProvider>
              <UiPreferencesProvider initialLocale={locale}>
                <AuthProvider>{children}</AuthProvider>
              </UiPreferencesProvider>
            </ClientI18nProvider>
          </QueryProvider>
        </body>
      </html>
    );
  }

  // Web 端：使用 NextIntlClientProvider
  // eslint-disable-next-line @typescript-eslint/no-require-imports
  const { NextIntlClientProvider } = require("next-intl");

  return (
    <html lang={locale} suppressHydrationWarning>
      <body className={`${spaceGrotesk.variable} ${ibmPlexSans.variable} ${jetbrainsMono.variable}`}>
        <QueryProvider>
          <NextIntlClientProvider locale={locale} messages={messages}>
            <UiPreferencesProvider initialLocale={locale}>
              <AuthProvider>{children}</AuthProvider>
            </UiPreferencesProvider>
          </NextIntlClientProvider>
        </QueryProvider>
      </body>
    </html>
  );
}
