/**
 * Public site URL for SEO surfaces (robots.ts / sitemap.ts).
 * Mirrors the metadataBase fallback chain in app/layout.tsx.
 */
export function getPublicSiteUrl(): string {
  return (
    process.env.NEXT_PUBLIC_SITE_URL?.trim() ||
    process.env.NEXT_PUBLIC_APP_ORIGIN?.trim() ||
    "https://app.contextlm.top"
  );
}
