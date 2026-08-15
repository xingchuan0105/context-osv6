"use client";

import Link from "next/link";
import { useEffect, useState } from "react";

import styles from "./desktop.module.css";
import { NavRail } from "../ui/nav-rail";
import { useAuth } from "@/lib/auth/context";
import {
  cloudLogout,
  getCloudSession,
  getCloudWalletBalance,
  type CloudSessionView,
  type CloudWalletView,
} from "@/lib/desktop/tauri-cloud";
import { openInBrowser } from "@/lib/desktop/tauri-license";
import {
  getAppDataDir,
  getAppVersion,
  getLocalProductStatus,
  getLocalStackStatus,
  openDataDir,
  openLogsDir,
  type LocalStackStatus,
} from "@/lib/desktop/tauri-local";
import { formatUiMessage, type UiMessageKey } from "@/lib/i18n/messages";
import { listProviderSecrets, type ProviderSecretRow } from "@/lib/settings/client";
import { useUiPreferences } from "@/lib/ui-preferences";
import { APP_PATHS, appAbsoluteUrl } from "@/lib/site-map";

/**
 * User-facing client settings drawer (2026-08-15 wave, W4): 账户 · 模型 ·
 * 数据 · 关于 + 诊断（默认收起，只读）per PRODUCT_IA §5. The dev stack
 * console (start/stop, raw client.env, CLI hints, local session block) is
 * gone — lifecycle stays automatic in ClientLocalSessionBootstrap.
 */

/** IPC/probe error → displayable message. */
function ipcErrorMessage(error: unknown): string {
  return error instanceof Error ? error.message : String(error);
}

/** Fire-and-forget probe: apply the value, or the fallback when the call fails. */
function probe<T>(call: () => Promise<T>, apply: (value: T) => void, fallback: T) {
  void call().then(apply).catch(() => apply(fallback));
}

type DrawerSection = "account" | "models" | "data" | "about" | "diagnostics";

const SECTIONS: DrawerSection[] = ["account", "models", "data", "about", "diagnostics"];

const SECTION_LABEL_KEY: Record<DrawerSection, UiMessageKey> = {
  account: "desktop.drawer.account",
  models: "desktop.drawer.models",
  data: "desktop.drawer.data",
  about: "desktop.drawer.about",
  diagnostics: "desktop.drawer.diagnostics",
};

/** provider-secrets purpose → 模型区条目label;unknown purposes fall back to the raw string. */
const PURPOSE_LABEL_KEY: Record<string, UiMessageKey> = {
  llm: "desktop.drawer.modelChat",
  embedding: "desktop.drawer.modelEmbedding",
  rerank: "desktop.drawer.modelRerank",
};

type DesktopSettingsDrawerProps = {
  open: boolean;
  onClose: () => void;
};

export function DesktopSettingsDrawer({ open, onClose }: DesktopSettingsDrawerProps) {
  const { locale } = useUiPreferences();
  const { token } = useAuth();
  const [section, setSection] = useState<DrawerSection>("account");
  const [session, setSession] = useState<CloudSessionView | null>(null);
  const [wallet, setWallet] = useState<CloudWalletView | null>(null);
  const [walletError, setWalletError] = useState("");
  const [byokSecrets, setByokSecrets] = useState<ProviderSecretRow[] | null>(null);
  const [dataDir, setDataDir] = useState("");
  const [logDir, setLogDir] = useState("");
  const [logDirLoaded, setLogDirLoaded] = useState(false);
  const [version, setVersion] = useState("");
  const [stack, setStack] = useState<LocalStackStatus | null>(null);
  const [logoutConfirm, setLogoutConfirm] = useState(false);
  const [logoutBusy, setLogoutBusy] = useState(false);
  const [error, setError] = useState("");

  function t(key: UiMessageKey) {
    return formatUiMessage(locale, key);
  }

  useEffect(() => {
    if (!open) return;
    setLogoutConfirm(false);
    setError("");
    setWalletError("");

    void getCloudSession()
      .then((next) => {
        setSession(next);
        if (!next.logged_in) {
          setWallet(null);
          return;
        }
        void getCloudWalletBalance()
          .then(setWallet)
          .catch((walletFetchError: unknown) => {
            setWallet(null);
            setWalletError(ipcErrorMessage(walletFetchError));
          });
      })
      .catch(() => setSession(null));

    probe(getAppDataDir, setDataDir, "");
    probe(getAppVersion, setVersion, "");
    // BYOK 优先序(设计文档 §3):配置了有效自备 Key 即覆盖平台 relay。
    if (token) {
      probe(() => listProviderSecrets(token).then((r) => r.secrets), setByokSecrets, []);
    }
  }, [open, token]);

  // 数据/诊断 both show the logs dir — probe product status only when one of
  // those sections is actually selected (same lazy rule as the stack status).
  useEffect(() => {
    if (!open || logDirLoaded || (section !== "data" && section !== "diagnostics")) return;
    setLogDirLoaded(true);
    probe(() => getLocalProductStatus().then((status) => status.log_dir ?? ""), setLogDir, "");
  }, [open, section, logDirLoaded]);

  // 诊断 content loads lazily — it is the collapsed-by-default last rail entry.
  useEffect(() => {
    if (!open || section !== "diagnostics" || stack) return;
    probe<LocalStackStatus | null>(getLocalStackStatus, setStack, null);
  }, [open, section, stack]);

  function reopenToGate() {
    // The shell re-mounts into CloudLoginGate, which renders the login card
    // whenever no cloud session exists.
    onClose();
    window.location.reload();
  }

  async function handleLogout() {
    if (logoutBusy) return;
    setLogoutBusy(true);
    setError("");
    try {
      await cloudLogout();
      reopenToGate();
    } catch (logoutError) {
      setError(ipcErrorMessage(logoutError));
      setLogoutBusy(false);
    }
  }

  async function handleOpenDir(which: "data" | "logs") {
    setError("");
    try {
      if (which === "data") {
        await openDataDir();
      } else {
        await openLogsDir();
      }
    } catch (openError) {
      setError(ipcErrorMessage(openError));
    }
  }

  if (!open) {
    return null;
  }

  const loggedIn = Boolean(session?.logged_in && session.user);
  const relay = session?.relay;
  // BYOK 优先:存在任一未撤销的自备 Key 即走自备,官方 relay 被覆盖(§3)。
  const activeSecrets = (byokSecrets ?? []).filter((secret) => !secret.revoked_at);
  const modelSource: "official" | "byok" = loggedIn && activeSecrets.length === 0 ? "official" : "byok";

  return (
    <div className={styles.drawerOverlay} role="presentation" onClick={onClose}>
      <aside
        className={styles.drawerPanel}
        role="dialog"
        aria-label={t("desktop.drawer.title")}
        onClick={(event) => event.stopPropagation()}
      >
        <header className={styles.drawerHeader}>
          <h2 className={styles.drawerTitle}>{t("desktop.drawer.title")}</h2>
          <button type="button" className="app-button-ghost" onClick={onClose}>
            {t("desktop.drawer.close")}
          </button>
        </header>

        <div className={styles.drawerBody}>
          <NavRail
            activeId={section}
            ariaLabel={t("desktop.drawer.railLabel")}
            items={SECTIONS.map((id) => ({ id, label: t(SECTION_LABEL_KEY[id]) }))}
            testId="desktop-settings-nav-rail"
            onSelect={(id) => setSection(id as DrawerSection)}
          />

          <div className={styles.drawerContent}>
            {error ? (
              <p className={styles.errorBox} role="alert">
                {error}
              </p>
            ) : null}

            {section === "account" ? (
              <div className={styles.drawerSection}>
                {loggedIn && session?.user ? (
                  <>
                    <div className={styles.drawerBlock}>
                      <p className={styles.drawerLabel}>{t("desktop.drawer.accountCloud")}</p>
                      <p className={styles.drawerValue}>{session.user.email}</p>
                    </div>
                    <div className={styles.drawerBlock}>
                      <p className={styles.drawerLabel}>{t("desktop.drawer.balance")}</p>
                      {walletError ? (
                        <p className={styles.errorBox} role="alert">
                          {walletError}
                        </p>
                      ) : wallet ? (
                        <p className={styles.drawerValue}>
                          ¥{(wallet.balance_fen / 100).toFixed(2)}
                        </p>
                      ) : (
                        <p className={styles.subtitle}>…</p>
                      )}
                    </div>
                    <div className="app-button-row">
                      <button
                        type="button"
                        className="app-button-secondary"
                        onClick={() =>
                          void openInBrowser(`${appAbsoluteUrl(APP_PATHS.pricing)}#topup`)
                        }
                      >
                        {t("desktop.drawer.topup")}
                      </button>
                      {logoutConfirm ? (
                        <>
                          <button
                            type="button"
                            className="app-button-secondary"
                            disabled={logoutBusy}
                            onClick={() => void handleLogout()}
                          >
                            {logoutBusy
                              ? t("desktop.drawer.logoutWorking")
                              : t("desktop.drawer.logoutConfirmAction")}
                          </button>
                          <button
                            type="button"
                            className="app-button-ghost"
                            disabled={logoutBusy}
                            onClick={() => setLogoutConfirm(false)}
                          >
                            {t("commonCancel")}
                          </button>
                        </>
                      ) : (
                        <button
                          type="button"
                          className="app-button-secondary"
                          onClick={() => setLogoutConfirm(true)}
                        >
                          {t("desktop.drawer.logout")}
                        </button>
                      )}
                    </div>
                    {logoutConfirm ? (
                      <p className={styles.subtitle}>{t("desktop.drawer.logoutConfirm")}</p>
                    ) : null}
                  </>
                ) : (
                  <>
                    <p className={styles.subtitle}>{t("desktop.drawer.notLoggedIn")}</p>
                    <div className="app-button-row">
                      <button
                        type="button"
                        className="app-button-primary"
                        onClick={reopenToGate}
                      >
                        {t("desktop.drawer.login")}
                      </button>
                    </div>
                  </>
                )}
              </div>
            ) : null}

            {section === "models" ? (
              <div className={styles.drawerSection}>
                <div className={styles.drawerBlock}>
                  <p className={styles.drawerLabel}>{t("desktop.drawer.modelSource")}</p>
                  <p className={styles.drawerValue}>
                    {modelSource === "official"
                      ? t("desktop.drawer.modelOfficial")
                      : t("desktop.drawer.modelByok")}
                  </p>
                </div>
                <p className={styles.subtitle}>
                  {modelSource === "official"
                    ? t("desktop.drawer.modelOfficialHint")
                    : t("desktop.drawer.modelByokHint")}
                </p>
                {modelSource === "official" && relay ? (
                  <>
                    <div className={styles.drawerBlock}>
                      <p className={styles.drawerLabel}>{t("desktop.drawer.modelChat")}</p>
                      <p className={styles.pathText}>{relay.chat_model}</p>
                    </div>
                    <div className={styles.drawerBlock}>
                      <p className={styles.drawerLabel}>{t("desktop.drawer.modelEmbedding")}</p>
                      <p className={styles.pathText}>{relay.embedding_model}</p>
                    </div>
                  </>
                ) : null}
                {modelSource === "byok"
                  ? activeSecrets.map((secret) => {
                      const labelKey = PURPOSE_LABEL_KEY[secret.purpose];
                      return (
                        <div className={styles.drawerBlock} key={secret.id}>
                          <p className={styles.drawerLabel}>
                            {labelKey ? t(labelKey) : secret.purpose}
                          </p>
                          <p className={styles.pathText}>
                            {secret.provider}
                            {secret.model_hint ? ` · ${secret.model_hint}` : ""}
                          </p>
                        </div>
                      );
                    })
                  : null}
                <p className={styles.subtitle}>
                  <Link href="/settings?tab=providers" onClick={onClose}>
                    {t("desktop.drawer.modelManage")}
                  </Link>
                </p>
              </div>
            ) : null}

            {section === "data" ? (
              <div className={styles.drawerSection}>
                <div className={styles.drawerBlock}>
                  <p className={styles.drawerLabel}>{t("desktop.drawer.dataDir")}</p>
                  <div className={styles.rowBetween}>
                    <p className={styles.pathText}>{dataDir || "…"}</p>
                    <button
                      type="button"
                      className="app-button-secondary"
                      disabled={!dataDir}
                      onClick={() => void handleOpenDir("data")}
                    >
                      {t("desktop.drawer.open")}
                    </button>
                  </div>
                </div>
                <div className={styles.drawerBlock}>
                  <p className={styles.drawerLabel}>{t("desktop.drawer.logsDir")}</p>
                  <div className={styles.rowBetween}>
                    <p className={styles.pathText}>{logDir || "…"}</p>
                    <button
                      type="button"
                      className="app-button-secondary"
                      disabled={!logDir}
                      onClick={() => void handleOpenDir("logs")}
                    >
                      {t("desktop.drawer.open")}
                    </button>
                  </div>
                </div>
              </div>
            ) : null}

            {section === "about" ? (
              <div className={styles.drawerSection}>
                <div className={styles.drawerBlock}>
                  <p className={styles.drawerLabel}>{t("desktop.drawer.version")}</p>
                  <p className={styles.drawerValue}>v{version || "…"}</p>
                </div>
                <p className={styles.subtitle}>{t("desktop.drawer.aboutFree")}</p>
                <div className="app-button-row">
                  <button
                    type="button"
                    className="app-button-secondary"
                    onClick={() => void openInBrowser(appAbsoluteUrl(APP_PATHS.desktop))}
                  >
                    {t("desktop.drawer.clientPage")}
                  </button>
                  <button
                    type="button"
                    className="app-button-secondary"
                    onClick={() => void openInBrowser(appAbsoluteUrl(APP_PATHS.pricing))}
                  >
                    {t("desktop.drawer.pricingPage")}
                  </button>
                </div>
              </div>
            ) : null}

            {section === "diagnostics" ? (
              <div className={styles.drawerSection}>
                <p className={styles.subtitle}>{t("desktop.drawer.diagnosticsHint")}</p>
                <div className={styles.drawerBlock}>
                  <p className={styles.drawerLabel}>{t("desktop.drawer.stackStatus")}</p>
                  {stack ? (
                    <ul className={styles.serviceList}>
                      {stack.services.map((s) => (
                        <li key={s.id}>
                          <strong>{s.label}</strong> {s.endpoint} —{" "}
                          <span className={s.ok ? styles.statusActive : styles.statusError}>
                            {s.ok ? "OK" : "DOWN"}
                          </span>
                          <div className={styles.subtitle}>{s.detail}</div>
                        </li>
                      ))}
                    </ul>
                  ) : (
                    <p className={styles.subtitle}>…</p>
                  )}
                </div>
                {stack?.env_file_path ? (
                  <div className={styles.drawerBlock}>
                    <p className={styles.drawerLabel}>{t("desktop.drawer.envFile")}</p>
                    <p className={styles.pathText}>{stack.env_file_path}</p>
                  </div>
                ) : null}
                <div className={styles.drawerBlock}>
                  <p className={styles.drawerLabel}>{t("desktop.drawer.logsDir")}</p>
                  <p className={styles.pathText}>{logDir || "…"}</p>
                </div>
              </div>
            ) : null}
          </div>
        </div>
      </aside>
    </div>
  );
}
