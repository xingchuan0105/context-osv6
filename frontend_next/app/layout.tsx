import type { Metadata } from "next";
import type { ReactNode } from "react";
import { NextIntlClientProvider } from "next-intl";
import { getLocale, getMessages } from "next-intl/server";

import "./globals.css";
import { AuthProvider } from "../lib/auth/context";
import { normalizeLocale } from "../lib/i18n/config";
import { QueryProvider } from "../lib/query/provider";
import { UiPreferencesProvider } from "../lib/ui-preferences";

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

export default async function RootLayout({ children }: { children: ReactNode }) {
  const locale = normalizeLocale(await getLocale());
  const messages = await getMessages();

  return (
    <html lang={locale} suppressHydrationWarning>
      <body>
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
