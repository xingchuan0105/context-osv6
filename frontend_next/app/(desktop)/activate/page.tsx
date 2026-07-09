"use client";

import { useRouter } from "next/navigation";
import { useEffect, useState } from "react";

import styles from "@/components/desktop/desktop.module.css";
import { ContextOsMark } from "@/components/context-os-mark";
import {
  activateLicense,
  formatLicenseError,
  getDeviceId,
  licenseKindLabel,
  licenseSeatsMax,
  licenseTypeLabel,
  listenDeepLinkActivate,
  openInBrowser,
  startTrial,
  type ActivationResult,
} from "@/lib/desktop/tauri-license";

type View = "choice" | "input" | "success" | "error";

type ActivationDisplay = {
  product: string;
  license_type: string;
  seats_used: number;
  seats_max: number;
};

function toActivationDisplay(result: ActivationResult): ActivationDisplay {
  return {
    product: licenseKindLabel(result.kind),
    license_type: licenseTypeLabel(result.kind),
    seats_used: 1,
    seats_max: licenseSeatsMax(result.kind),
  };
}

export default function ActivatePage() {
  const router = useRouter();
  const [view, setView] = useState<View>("choice");
  const [licenseKey, setLicenseKey] = useState("");
  const [deviceId, setDeviceId] = useState("");
  const [activation, setActivation] = useState<ActivationDisplay | null>(null);
  const [error, setError] = useState("");
  const [loading, setLoading] = useState(false);

  useEffect(() => {
    void getDeviceId()
      .then(setDeviceId)
      .catch(() => {});

    let cleanup: (() => void) | undefined;

    void listenDeepLinkActivate((key) => {
      setLicenseKey(key);
      setView("input");
    })
      .then((unlisten) => {
        cleanup = () => {
          void unlisten();
        };
      })
      .catch(() => {});

    return () => {
      cleanup?.();
    };
  }, []);

  async function handleStartTrial() {
    setLoading(true);
    setError("");

    try {
      await startTrial();
      router.push("/setup");
    } catch (trialError) {
      setError(formatLicenseError(trialError));
      setView("error");
    } finally {
      setLoading(false);
    }
  }

  async function handleActivate() {
    setLoading(true);
    setError("");

    try {
      const result = await activateLicense(licenseKey.trim());
      setActivation(toActivationDisplay(result));
      setView("success");
    } catch (activateError) {
      setError(formatLicenseError(activateError));
      setView("error");
    } finally {
      setLoading(false);
    }
  }

  async function handleOpenBuyPage() {
    const query = deviceId ? `?device_id=${encodeURIComponent(deviceId)}` : "";
    await openInBrowser(`https://app.avrag.com/desktop/buy${query}`);
  }

  async function handleOpenHelp() {
    await openInBrowser("https://app.avrag.com/help/desktop-activation");
  }

  return (
    <section className={styles.card}>
      {view === "choice" ? (
        <>
          <header className={styles.header}>
            <ContextOsMark style={{ width: "4rem", height: "4rem", margin: "0 auto" }} />
            <h1 className={styles.title}>欢迎使用 AVRag Desktop</h1>
            <p className={styles.subtitle}>请选择激活方式</p>
          </header>

          <div className={styles.choiceGrid}>
            <button
              type="button"
              className={styles.choiceCard}
              onClick={() => void handleStartTrial()}
              disabled={loading}
            >
              <p className={styles.choiceTitle}>开始 7 天试用</p>
              <p className={styles.choiceHint}>全功能免费体验，无需信用卡</p>
            </button>
            <button
              type="button"
              className={styles.choiceCard}
              onClick={() => setView("input")}
              disabled={loading}
            >
              <p className={styles.choiceTitle}>我已有授权码</p>
              <p className={styles.choiceHint}>输入 AVRG-XXXX 格式授权码</p>
            </button>
          </div>

          <div className={styles.footerLinks}>
            <button type="button" className={styles.footerLink} onClick={() => void handleOpenBuyPage()}>
              购买授权
            </button>
            <button type="button" className={styles.footerLink} onClick={() => void handleOpenHelp()}>
              查看帮助
            </button>
          </div>
        </>
      ) : null}

      {view === "input" ? (
        <>
          <header className={styles.header}>
            <h1 className={styles.title}>输入授权码</h1>
          </header>

          <div style={{ display: "grid", gap: "1rem" }}>
            <div>
              <label className="app-form-label" htmlFor="license-key">
                授权码
              </label>
              <input
                id="license-key"
                className="app-input"
                value={licenseKey}
                onChange={(event) => setLicenseKey(event.target.value)}
                placeholder="AVRG-XXXX-XXXX-XXXX-XXXX"
              />
            </div>

            <div className="app-button-row" style={{ justifyContent: "flex-end" }}>
              <button
                type="button"
                className="app-button-secondary"
                onClick={() => {
                  setError("");
                  setView("choice");
                }}
              >
                取消
              </button>
              <button
                type="button"
                className="app-button-primary"
                onClick={() => void handleActivate()}
                disabled={loading || !licenseKey.trim()}
              >
                {loading ? "激活中…" : "激活"}
              </button>
            </div>
          </div>

          {deviceId ? (
            <p className={styles.deviceId}>本机设备 ID: {deviceId.slice(0, 8)}…（用于绑定）</p>
          ) : null}
        </>
      ) : null}

      {view === "success" && activation ? (
        <>
          <header className={styles.header}>
            <div className={styles.successIcon} aria-hidden="true">
              ✓
            </div>
            <h1 className={styles.title}>激活成功</h1>
          </header>

          <div className={styles.metaList}>
            <div className={styles.metaRow}>
              <span className={styles.metaLabel}>产品</span>
              <span>{activation.product}</span>
            </div>
            <div className={styles.metaRow}>
              <span className={styles.metaLabel}>授权</span>
              <span>{activation.license_type}</span>
            </div>
            <div className={styles.metaRow}>
              <span className={styles.metaLabel}>设备</span>
              <span>
                {activation.seats_used}/{activation.seats_max} 已激活
              </span>
            </div>
          </div>

          <div className="app-button-row" style={{ justifyContent: "center" }}>
            <button
              type="button"
              className="app-button-secondary"
              onClick={() => router.push("/setup")}
            >
              配置 LLM 模型
            </button>
            <button
              type="button"
              className="app-button-primary"
              onClick={() => router.push("/dashboard")}
            >
              开始使用
            </button>
          </div>
        </>
      ) : null}

      {view === "error" ? (
        <>
          <header className={styles.header}>
            <h1 className={styles.title}>激活失败</h1>
          </header>
          {error ? (
            <p className={styles.errorBox} role="alert">
              {error}
            </p>
          ) : null}
          <div className="app-button-row" style={{ justifyContent: "center" }}>
            <button type="button" className="app-button-primary" onClick={() => setView("choice")}>
              返回
            </button>
          </div>
        </>
      ) : null}
    </section>
  );
}
