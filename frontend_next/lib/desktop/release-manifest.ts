export type DesktopPlatformManifest = {
  url: string;
  sha256: string;
  size_bytes: number;
  format: "nsis" | "portable" | string;
  filename?: string;
};

export type DesktopReleaseManifest = {
  product: string;
  version: string;
  published_at: string;
  platforms: {
    "windows-x64"?: DesktopPlatformManifest;
  };
  min_os?: string;
  notes_url?: string;
};

export const DESKTOP_LATEST_JSON_URL = "/releases/desktop/latest.json";

export function formatBytes(bytes: number): string {
  if (!Number.isFinite(bytes) || bytes <= 0) return "—";
  const mb = bytes / (1024 * 1024);
  if (mb < 1) return `${Math.max(1, Math.round(bytes / 1024))} KB`;
  return `${mb.toFixed(mb >= 10 ? 0 : 1)} MB`;
}

export function windowsDownloadFromManifest(
  manifest: DesktopReleaseManifest | null,
): DesktopPlatformManifest | null {
  return manifest?.platforms?.["windows-x64"] ?? null;
}
