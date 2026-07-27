"use client";

import { useEffect, useState } from "react";

import styles from "./desktop.module.css";
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
      <div className={styles.downloadWrap}>
        <button type="button" className={className} disabled>
          {formatUiMessage(locale, "desktop.downloadUnavailable")}
        </button>
        {!compact ? (
          <p className={`app-page-subtitle ${styles.downloadHint}`}>
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
    <div className={`${styles.downloadWrap} ${compact ? styles.justifyStart : styles.justifyCenter}`}>
      <a
        className={className}
        data-testid="desktop-download-windows"
        download={win.filename ?? true}
        href={win.url}
      >
        {formatUiMessage(locale, "desktop.downloadWindows")}
      </a>
      {!compact ? (
        <p className={`app-page-subtitle ${styles.downloadHint}`}>
          {meta}
          {state.manifest.min_os ? ` · ${state.manifest.min_os}` : ""}
        </p>
      ) : (
        <span className={styles.downloadMetaCompact}>{meta}</span>
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
    <dl data-testid="desktop-release-details" className={styles.releaseDetails}>
      <div>
        <dt className={styles.releaseTerm}>
          {formatUiMessage(locale, "desktop.versionLabel")}:{" "}
        </dt>
        <dd className={styles.releaseValue}>v{manifest.version}</dd>
      </div>
      <div>
        <dt className={styles.releaseTerm}>
          {formatUiMessage(locale, "desktop.sha256Label")}:{" "}
        </dt>
        <dd className={styles.releaseHash}>
          {win.sha256}
        </dd>
      </div>
      {win.format === "portable" ? (
        <div>{formatUiMessage(locale, "desktop.portableHint")}</div>
      ) : null}
      {win.authenticode === true ? (
        <div data-testid="desktop-signed-hint">{formatUiMessage(locale, "desktop.signedHint")}</div>
      ) : win.authenticode === false ? (
        <div data-testid="desktop-unsigned-hint">{formatUiMessage(locale, "desktop.unsignedHint")}</div>
      ) : null}
    </dl>
  );
}
