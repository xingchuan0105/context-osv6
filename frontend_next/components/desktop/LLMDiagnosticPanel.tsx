"use client";

import { useState } from "react";

import styles from "./desktop.module.css";
import {
  diagnoseLlm,
  executeRepairAction,
  repairActionLabel,
  type DiagnosticReport,
  type DiagnosticStatus,
  type LocalLlmConfig,
  type RepairActionResult,
} from "@/lib/desktop/tauri-llm";

type LLMDiagnosticPanelProps = {
  config?: LocalLlmConfig;
  onConfigUpdated?: (config: LocalLlmConfig) => void;
};

function statusIcon(status: DiagnosticStatus): string {
  if (status === "ok") return "✓";
  if (status === "warning") return "⚠";
  return "✗";
}

function statusClass(status: DiagnosticStatus): string {
  if (status === "ok") return styles.checkOk;
  if (status === "warning") return styles.checkWarning;
  return styles.checkError;
}

export function LLMDiagnosticPanel({ config, onConfigUpdated }: LLMDiagnosticPanelProps) {
  const [report, setReport] = useState<DiagnosticReport | null>(null);
  const [loading, setLoading] = useState(false);
  const [repairingCode, setRepairingCode] = useState<string | null>(null);
  const [repairFeedback, setRepairFeedback] = useState<RepairActionResult | null>(null);
  const [error, setError] = useState("");

  async function runDiagnostic() {
    if (!config) {
      setError("请先填写 LLM 配置后再运行诊断");
      return;
    }

    setLoading(true);
    setError("");
    setRepairFeedback(null);

    try {
      const result = await diagnoseLlm(config);
      setReport(result);
    } catch (diagnosticError) {
      setError(
        diagnosticError instanceof Error ? diagnosticError.message : "诊断失败，请稍后重试",
      );
      setReport(null);
    } finally {
      setLoading(false);
    }
  }

  async function handleRepair(suggestionCode: string, action: NonNullable<DiagnosticReport["suggestions"][number]["action"]>) {
    setRepairingCode(suggestionCode);
    setRepairFeedback(null);

    try {
      const result = await executeRepairAction(action, {
        currentConfig: config ?? null,
        onConfigUpdated,
      });
      setRepairFeedback(result);
    } catch (repairError) {
      setRepairFeedback({
        applied: false,
        message: repairError instanceof Error ? repairError.message : "修复动作执行失败",
      });
    } finally {
      setRepairingCode(null);
    }
  }

  return (
    <section className={styles.diagnosticPanel} aria-label="LLM 连接诊断">
      <div className="app-button-row">
        <button
          type="button"
          className="app-button-secondary"
          onClick={() => void runDiagnostic()}
          disabled={loading || !config}
        >
          {loading ? "诊断中…" : "运行诊断"}
        </button>
      </div>

      {error ? (
        <p className={styles.errorBox} role="alert">
          {error}
        </p>
      ) : null}

      {repairFeedback ? (
        <p className={styles.subtitle} role="status">
          {repairFeedback.message}
        </p>
      ) : null}

      {report ? (
        <div>
          {report.checks.map((check) => (
            <div key={check.name} className={styles.diagnosticCheck}>
              <span className={statusClass(check.status)} aria-hidden="true">
                {statusIcon(check.status)}
              </span>
              <div>
                <div>
                  <strong>{check.name}</strong>
                  {check.latency_ms != null ? ` (${check.latency_ms}ms)` : null}
                </div>
                <div>{check.message}</div>
              </div>
            </div>
          ))}

          {report.suggestions.length > 0 ? (
            <div className={styles.repairList}>
              {report.suggestions.map((suggestion) => (
                <div key={suggestion.code} className={styles.repairSuggestion}>
                  <p className={styles.repairMessage}>{suggestion.message}</p>
                  {suggestion.action ? (
                    <button
                      type="button"
                      className="app-button-secondary"
                      disabled={repairingCode === suggestion.code}
                      onClick={() => void handleRepair(suggestion.code, suggestion.action!)}
                    >
                      {repairingCode === suggestion.code
                        ? "处理中…"
                        : repairActionLabel(suggestion.action)}
                    </button>
                  ) : null}
                </div>
              ))}
            </div>
          ) : null}
        </div>
      ) : (
        <p className={styles.subtitle}>点击「运行诊断」检查 LLM 连接状态。</p>
      )}
    </section>
  );
}
