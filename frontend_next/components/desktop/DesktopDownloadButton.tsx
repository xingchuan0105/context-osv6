"use client";

import { useEffect, useState } from "react";

import {
  DESKTOP_LATEST_JSON_URL,
  formatBytes,
  type DesktopReleaseManifest,
  windowsDownloadFromManifest,
} from "@/lib/desktop/release-manifest";
import { formatUiMessage } from "@/lib/i18n/messages";
import { useUiPreferences } from "@/lib/ui-preferences";

type LoadState =
  | { status: "loading" }
  | { status: "ready"; manifest: DesktopReleaseManifest }
  | { status: "missing" }
  | { status: "error" };

export function DesktopDownloadButton({
  className = "app-button-primary",
  compact = false,
}: {
  className?: string;
  compact?: boolean;
}) {
  const { locale } = useUiPreferences();
  const [state, setState] = useState<LoadState>({ status: "loading" });

  useEffect(() => {
    let cancelled = false;
    void (async () => {
      try {
        const response = await fetch(DESKTOP_LATEST_JSON_URL, {
          cache: "no-cache",
        });
        if (!response.ok) {
          if (!cancelled) setState({ status: "missing" });
          return;
        }
        const manifest = (await response.json()) as DesktopReleaseManifest;
        if (!windowsDownloadFromManifest(manifest)) {
          if (!cancelled) setState({ status: "missing" });
          return;
        }
        if (!cancelled) setState({ status: "ready", manifest });
      } catch {
        if (!cancelled) setState({ status: "error" });
      }
    })();
    return () => {
      cancelled = true;
    };
  }, []);

  if (state.status === "loading") {
    return (
      <button type="button" className={className} disabled>
        {formatUiMessage(locale, "desktop.downloadLoading")}
      </button>
    );
  }

  if (state.status === "missing" || state.status === "error") {
    return (
      <div style={{ display: "grid", gap: "0.35rem" }}>
        <button type="button" className={className} disabled>
          {formatUiMessage(locale, "desktop.downloadUnavailable")}
        </button>
        {!compact ? (
          <p className="app-page-subtitle" style={{ margin: 0, fontSize: "0.85rem" }}>
            {formatUiMessage(locale, "desktop.downloadUnavailableHint")}
          </p>
        ) : null}
      </div>
    );
  }

  const win = windowsDownloadFromManifest(state.manifest)!;
  const meta = formatUiMessage(locale, "desktop.downloadMeta", {
    version: state.manifest.version,
    size: formatBytes(win.size_bytes),
  });

  return (
    <div style={{ display: "grid", gap: "0.35rem", justifyItems: compact ? "start" : "center" }}>
      <a
        className={className}
        data-testid="desktop-download-windows"
        download={win.filename ?? true}
        href={win.url}
      >
        {formatUiMessage(locale, "desktop.downloadWindows")}
      </a>
      {!compact ? (
        <p className="app-page-subtitle" style={{ margin: 0, fontSize: "0.85rem" }}>
          {meta}
          {state.manifest.min_os ? ` · ${state.manifest.min_os}` : ""}
        </p>
      ) : (
        <span style={{ fontSize: "0.8rem", color: "hsl(var(--muted-foreground))" }}>{meta}</span>
      )}
    </div>
  );
}

export function DesktopReleaseDetails() {
  const { locale } = useUiPreferences();
  const [manifest, setManifest] = useState<DesktopReleaseManifest | null>(null);

  useEffect(() => {
    let cancelled = false;
    void fetch(DESKTOP_LATEST_JSON_URL, { cache: "no-cache" })
      .then((r) => (r.ok ? r.json() : null))
      .then((data: DesktopReleaseManifest | null) => {
        if (!cancelled) setManifest(data);
      })
      .catch(() => {});
    return () => {
      cancelled = true;
    };
  }, []);

  const win = windowsDownloadFromManifest(manifest);
  if (!manifest || !win) return null;

  return (
    <dl
      data-testid="desktop-release-details"
      style={{
        margin: "1rem 0 0",
        display: "grid",
        gap: "0.35rem",
        fontSize: "0.85rem",
        color: "hsl(var(--muted-foreground))",
      }}
    >
      <div>
        <dt style={{ display: "inline", fontWeight: 600 }}>
          {formatUiMessage(locale, "desktop.versionLabel")}:{" "}
        </dt>
        <dd style={{ display: "inline", margin: 0 }}>v{manifest.version}</dd>
      </div>
      <div>
        <dt style={{ display: "inline", fontWeight: 600 }}>
          {formatUiMessage(locale, "desktop.sha256Label")}:{" "}
        </dt>
        <dd
          style={{
            display: "inline",
            margin: 0,
            fontFamily: "var(--font-mono)",
            wordBreak: "break-all",
          }}
        >
          {win.sha256}
        </dd>
      </div>
      {win.format === "portable" ? (
        <div>{formatUiMessage(locale, "desktop.portableHint")}</div>
      ) : null}
    </dl>
  );
}
