"use client";

import { useRouter } from "next/navigation";
import { useMemo, useState } from "react";

import styles from "@/components/desktop/desktop.module.css";
import { LLMDiagnosticPanel } from "@/components/desktop/LLMDiagnosticPanel";
import { findLlmPreset, LLM_PRESETS, type LlmPreset } from "@/lib/desktop/llm-presets";
import { openInBrowser } from "@/lib/desktop/tauri-license";
import {
  setLlmConfig,
  testLlmConnection,
  type LocalLlmConfig,
  type TestResult,
} from "@/lib/desktop/tauri-llm";

type SetupStep = 1 | 2 | 3;

const FEATURED_PRESET_IDS = [
  "zhipu",
  "anthropic",
  "deepseek",
  "openai",
  "google",
  "siliconflow",
  "dashscope",
  "ollama",
  "custom",
] as const;

export default function SetupPage() {
  const router = useRouter();
  const [step, setStep] = useState<SetupStep>(1);
  const [selectedPresetId, setSelectedPresetId] = useState<string>("zhipu");
  const [apiKey, setApiKey] = useState("");
  const [baseUrl, setBaseUrl] = useState("");
  const [model, setModel] = useState("");
  const [testResult, setTestResult] = useState<TestResult | null>(null);
  const [loading, setLoading] = useState(false);
  const [showDiagnostic, setShowDiagnostic] = useState(false);
  const [error, setError] = useState("");

  const featuredPresets = useMemo(
    () =>
      FEATURED_PRESET_IDS.map((id) => findLlmPreset(id)).filter(
        (preset): preset is LlmPreset => preset != null,
      ),
    [],
  );

  const selectedPreset = findLlmPreset(selectedPresetId);

  function buildConfig(): LocalLlmConfig {
    return {
      provider: selectedPresetId,
      base_url: baseUrl,
      api_key: apiKey,
      model,
      timeout_ms: 30_000,
    };
  }

  function selectPreset(preset: LlmPreset) {
    setSelectedPresetId(preset.id);
    setBaseUrl(preset.base_url);
    setModel(preset.model);
    setTestResult(null);
    setError("");
    setStep(2);
  }

  async function handleTestConnection() {
    setLoading(true);
    setTestResult(null);
    setError("");

    try {
      const result = await testLlmConnection(buildConfig());
      setTestResult(result);
      setStep(3);
    } catch (testError) {
      setTestResult({
        ok: false,
        message: testError instanceof Error ? testError.message : "连接测试失败",
      });
      setStep(3);
    } finally {
      setLoading(false);
    }
  }

  async function handleSaveAndFinish() {
    setLoading(true);
    setError("");

    try {
      await setLlmConfig(buildConfig());
      router.push("/dashboard");
    } catch (saveError) {
      setError(saveError instanceof Error ? saveError.message : "保存配置失败");
    } finally {
      setLoading(false);
    }
  }

  return (
    <section className={styles.card}>
      <p className={styles.stepLabel}>{step}/3</p>

      {step === 1 ? (
        <>
          <header className={styles.header}>
            <h1 className={styles.title}>配置 AI 模型</h1>
            <p className={styles.subtitle}>选择你的 AI 服务商</p>
          </header>

          <div className={styles.presetGrid}>
            {featuredPresets.map((preset) => (
              <button
                key={preset.id}
                type="button"
                className={`${styles.presetCard} ${selectedPresetId === preset.id ? styles.presetCardActive : ""}`}
                onClick={() => selectPreset(preset)}
              >
                <p className={styles.presetLabel}>{preset.label.split("（")[0]}</p>
                <p className={styles.presetNote}>{preset.pricing_note || preset.description}</p>
              </button>
            ))}
          </div>

          <details style={{ marginTop: "1rem" }}>
            <summary>查看全部 {LLM_PRESETS.length} 个服务商</summary>
            <div className={styles.presetGrid} style={{ marginTop: "0.75rem" }}>
              {LLM_PRESETS.filter(
                (preset) =>
                  !FEATURED_PRESET_IDS.includes(preset.id as (typeof FEATURED_PRESET_IDS)[number]),
              ).map((preset) => (
                <button
                  key={preset.id}
                  type="button"
                  className={styles.presetCard}
                  onClick={() => selectPreset(preset)}
                >
                  <p className={styles.presetLabel}>{preset.label}</p>
                </button>
              ))}
            </div>
          </details>

          <div className="app-button-row" style={{ justifyContent: "flex-end", marginTop: "1.25rem" }}>
            <button type="button" className="app-button-ghost" onClick={() => router.push("/dashboard")}>
              跳过，稍后配置
            </button>
          </div>
        </>
      ) : null}

      {step === 2 && selectedPreset ? (
        <>
          <header className={styles.header}>
            <h1 className={styles.title}>配置 AI 模型</h1>
            <p className={styles.subtitle}>{selectedPreset.label}</p>
          </header>

          <div style={{ display: "grid", gap: "1rem" }}>
            {selectedPreset.id !== "ollama" ? (
              <div>
                <label className="app-form-label" htmlFor="api-key">
                  API Key
                </label>
                <div className="app-button-row">
                  <input
                    id="api-key"
                    className="app-input"
                    type="password"
                    value={apiKey}
                    onChange={(event) => setApiKey(event.target.value)}
                    placeholder="粘贴 API Key"
                    style={{ flex: 1 }}
                  />
                  {selectedPreset.api_key_url ? (
                    <button
                      type="button"
                      className="app-button-secondary"
                      onClick={() => void openInBrowser(selectedPreset.api_key_url)}
                    >
                      申请 →
                    </button>
                  ) : null}
                </div>
              </div>
            ) : null}

            <div>
              <label className="app-form-label" htmlFor="model">
                Model
              </label>
              <input
                id="model"
                className="app-input"
                value={model}
                onChange={(event) => setModel(event.target.value)}
              />
            </div>

            <div>
              <label className="app-form-label" htmlFor="base-url">
                Base URL
              </label>
              <input
                id="base-url"
                className="app-input"
                value={baseUrl}
                onChange={(event) => setBaseUrl(event.target.value)}
              />
            </div>

            <div className="app-button-row" style={{ justifyContent: "space-between" }}>
              <button type="button" className="app-button-secondary" onClick={() => setStep(1)}>
                上一步
              </button>
              <button
                type="button"
                className="app-button-primary"
                onClick={() => void handleTestConnection()}
                disabled={loading || (selectedPreset.id !== "ollama" && !apiKey.trim())}
              >
                {loading ? "测试中…" : "测试连接"}
              </button>
            </div>
          </div>
        </>
      ) : null}

      {step === 3 ? (
        <>
          <header className={styles.header}>
            <h1 className={styles.title}>配置 AI 模型</h1>
          </header>

          {testResult ? (
            <div
              className="app-inline-surface"
              style={{
                borderColor: testResult.ok ? "hsl(var(--success))" : "hsl(var(--destructive-border))",
                marginBottom: "1rem",
              }}
            >
              <p style={{ margin: 0 }}>
                {testResult.ok ? "✓" : "✗"} {testResult.message}
                {testResult.latency_ms != null ? `，延迟 ${testResult.latency_ms}ms` : ""}
              </p>
            </div>
          ) : null}

          {!testResult?.ok ? (
            <div style={{ marginBottom: "1rem" }}>
              <button
                type="button"
                className="app-button-secondary"
                onClick={() => setShowDiagnostic((value) => !value)}
              >
                {showDiagnostic ? "隐藏诊断" : "运行诊断"}
              </button>
              {showDiagnostic ? (
                <div style={{ marginTop: "0.75rem" }}>
                  <LLMDiagnosticPanel config={buildConfig()} />
                </div>
              ) : null}
            </div>
          ) : null}

          {error ? (
            <p className={styles.errorBox} role="alert">
              {error}
            </p>
          ) : null}

          <div className="app-button-row" style={{ justifyContent: "flex-end" }}>
            <button
              type="button"
              className="app-button-primary"
              onClick={() => void handleSaveAndFinish()}
              disabled={loading}
            >
              {loading ? "保存中…" : "完成配置"}
            </button>
          </div>
        </>
      ) : null}
    </section>
  );
}
