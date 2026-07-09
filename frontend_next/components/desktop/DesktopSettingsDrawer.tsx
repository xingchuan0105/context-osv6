"use client";

import Link from "next/link";
import { useEffect, useState } from "react";

import styles from "./desktop.module.css";
import { LLMDiagnosticPanel } from "@/components/desktop/LLMDiagnosticPanel";
import { findLlmPreset } from "@/lib/desktop/llm-presets";
import {
  getLicenseStatus,
  licenseKindLabel,
  licenseTypeLabel,
  openInBrowser,
} from "@/lib/desktop/tauri-license";
import {
  getLlmConfig,
  setLlmConfig,
  testLlmConnection,
  type LocalLlmConfig,
} from "@/lib/desktop/tauri-llm";

type DrawerTab = "llm" | "license" | "diagnostic";

type DesktopSettingsDrawerProps = {
  open: boolean;
  onClose: () => void;
};

export function DesktopSettingsDrawer({ open, onClose }: DesktopSettingsDrawerProps) {
  const [tab, setTab] = useState<DrawerTab>("llm");
  const [config, setConfig] = useState<LocalLlmConfig | null>(null);
  const [provider, setProvider] = useState("zhipu");
  const [apiKey, setApiKey] = useState("");
  const [baseUrl, setBaseUrl] = useState("");
  const [model, setModel] = useState("");
  const [licenseLabel, setLicenseLabel] = useState("");
  const [licenseDetail, setLicenseDetail] = useState("");
  const [loading, setLoading] = useState(false);
  const [message, setMessage] = useState("");
  const [error, setError] = useState("");

  useEffect(() => {
    if (!open) return;

    void getLlmConfig()
      .then((saved) => {
        if (!saved) return;
        setConfig(saved);
        setProvider(saved.provider);
        setApiKey(saved.api_key);
        setBaseUrl(saved.base_url);
        setModel(saved.model);
      })
      .catch(() => {});

    void getLicenseStatus()
      .then((status) => {
        const kind = status.license_kind ?? "standard";
        setLicenseLabel(licenseKindLabel(kind));
        setLicenseDetail(licenseTypeLabel(kind, status.days_remaining));
      })
      .catch(() => {
        setLicenseLabel("未激活");
        setLicenseDetail("");
      });
  }, [open]);

  if (!open) {
    return null;
  }

  function buildDraftConfig(): LocalLlmConfig {
    return {
      provider,
      base_url: baseUrl,
      api_key: apiKey,
      model,
      timeout_ms: config?.timeout_ms ?? 30_000,
      embedding: config?.embedding ?? null,
    };
  }

  async function handleSaveLlm() {
    setLoading(true);
    setError("");
    setMessage("");

    try {
      const next = buildDraftConfig();
      await setLlmConfig(next);
      setConfig(next);
      setMessage("LLM 配置已保存");
    } catch (saveError) {
      setError(saveError instanceof Error ? saveError.message : "保存失败");
    } finally {
      setLoading(false);
    }
  }

  async function handleTestLlm() {
    setLoading(true);
    setError("");
    setMessage("");

    try {
      const result = await testLlmConnection(buildDraftConfig());
      setMessage(result.message);
    } catch (testError) {
      setError(testError instanceof Error ? testError.message : "连接测试失败");
    } finally {
      setLoading(false);
    }
  }

  function applyPreset(presetId: string) {
    const preset = findLlmPreset(presetId);
    if (!preset) return;
    setProvider(preset.id);
    setBaseUrl(preset.base_url);
    setModel(preset.model);
  }

  return (
    <div className={styles.drawerOverlay} role="presentation" onClick={onClose}>
      <aside
        className={styles.drawerPanel}
        role="dialog"
        aria-label="桌面端设置"
        onClick={(event) => event.stopPropagation()}
      >
        <header className={styles.drawerHeader}>
          <h2 className={styles.drawerTitle}>桌面端设置</h2>
          <button type="button" className="app-button-ghost" onClick={onClose}>
            关闭
          </button>
        </header>

        <div className={styles.drawerTabs}>
          {([
            { id: "llm" as const, label: "AI 模型" },
            { id: "license" as const, label: "授权管理" },
            { id: "diagnostic" as const, label: "诊断工具" },
          ]).map((item) => (
            <button
              key={item.id}
              type="button"
              className={tab === item.id ? styles.drawerTabActive : styles.drawerTab}
              onClick={() => setTab(item.id)}
            >
              {item.label}
            </button>
          ))}
        </div>

        {error ? (
          <p className={styles.errorBox} role="alert">
            {error}
          </p>
        ) : null}
        {message ? <p className={styles.subtitle}>{message}</p> : null}

        {tab === "llm" ? (
          <div className={styles.drawerSection}>
            <label className="app-form-label" htmlFor="desktop-provider">
              Provider
            </label>
            <select
              id="desktop-provider"
              className="app-input"
              value={provider}
              onChange={(event) => {
                setProvider(event.target.value);
                applyPreset(event.target.value);
              }}
            >
              <option value="zhipu">智谱 GLM</option>
              <option value="anthropic">Anthropic</option>
              <option value="deepseek">DeepSeek</option>
              <option value="openai">OpenAI</option>
              <option value="ollama">Ollama</option>
              <option value="custom">自定义</option>
            </select>

            <label className="app-form-label" htmlFor="desktop-api-key">
              API Key
            </label>
            <input
              id="desktop-api-key"
              className="app-input"
              type="password"
              value={apiKey}
              onChange={(event) => setApiKey(event.target.value)}
            />

            <label className="app-form-label" htmlFor="desktop-model">
              Model
            </label>
            <input
              id="desktop-model"
              className="app-input"
              value={model}
              onChange={(event) => setModel(event.target.value)}
            />

            <label className="app-form-label" htmlFor="desktop-base-url">
              Base URL
            </label>
            <input
              id="desktop-base-url"
              className="app-input"
              value={baseUrl}
              onChange={(event) => setBaseUrl(event.target.value)}
            />

            <div className="app-button-row">
              <button
                type="button"
                className="app-button-secondary"
                disabled={loading}
                onClick={() => void handleTestLlm()}
              >
                测试连接
              </button>
              <button
                type="button"
                className="app-button-primary"
                disabled={loading}
                onClick={() => void handleSaveLlm()}
              >
                保存
              </button>
            </div>
          </div>
        ) : null}

        {tab === "license" ? (
          <div className={styles.drawerSection}>
            <p>
              <strong>{licenseLabel}</strong>
            </p>
            {licenseDetail ? <p className={styles.subtitle}>{licenseDetail}</p> : null}
            <div className="app-button-row">
              <Link href="/licenses" className="app-button-secondary" onClick={onClose}>
                在浏览器管理授权
              </Link>
              <button
                type="button"
                className="app-button-secondary"
                onClick={() => void openInBrowser("https://app.avrag.com/desktop/buy")}
              >
                购买/升级
              </button>
            </div>
          </div>
        ) : null}

        {tab === "diagnostic" ? (
          <div className={styles.drawerSection}>
            <LLMDiagnosticPanel
              config={buildDraftConfig()}
              onConfigUpdated={(next) => {
                setConfig(next);
                setProvider(next.provider);
                setApiKey(next.api_key);
                setBaseUrl(next.base_url);
                setModel(next.model);
              }}
            />
          </div>
        ) : null}
      </aside>
    </div>
  );
}
