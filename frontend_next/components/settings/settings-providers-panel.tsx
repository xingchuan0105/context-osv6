"use client";

import { useCallback, useMemo, useState } from "react";
import { useQuery, useQueryClient } from "@tanstack/react-query";

import { describeAuthError } from "../../lib/auth/errors";
import { useAuth } from "../../lib/auth/context";
import { formatUiMessage } from "../../lib/i18n/messages";
import {
  listProviderSecrets,
  revokeProviderSecret,
  upsertProviderSecret,
  type ProviderSecretRow,
} from "../../lib/settings/client";
import { useUiPreferences } from "../../lib/ui-preferences";
import { settingsKeys } from "./settings-shared";
import shared from "./settings-ui-shared.module.css";
import styles from "./settings-providers-panel.module.css";

type Purpose = "llm" | "embedding" | "rerank";

type SlotTarget = {
  purpose: Purpose;
  modelHint: string;
};

/**
 * Fixed product rows: type + model names locked; KEY only.
 * SiliconFlow embedding + rerank share one key (one UI row, two store purposes).
 */
const FIXED_ROWS = [
  {
    id: "agent_llm",
    provider: "deepseek",
    baseUrl: "https://api.deepseek.com",
    typeKey: "settingsProvider.type.agentLlm" as const,
    modelKey: "settingsProvider.model.deepseek" as const,
    targets: [{ purpose: "llm" as Purpose, modelHint: "deepseek-v4-flash" }],
  },
  {
    id: "parse_llm",
    provider: "bailian",
    baseUrl: "https://dashscope.aliyuncs.com/compatible-mode/v1",
    typeKey: "settingsProvider.type.parseLlm" as const,
    modelKey: "settingsProvider.model.bailian" as const,
    targets: [{ purpose: "llm" as Purpose, modelHint: "qwen3.7-flash" }],
  },
  {
    id: "siliconflow",
    provider: "siliconflow",
    baseUrl: "https://api.siliconflow.cn/v1",
    typeKey: "settingsProvider.type.embedRerank" as const,
    modelKey: "settingsProvider.model.siliconflow" as const,
    targets: [
      { purpose: "embedding" as Purpose, modelHint: "BAAI/bge-m3" },
      { purpose: "rerank" as Purpose, modelHint: "BAAI/bge-reranker-v2-m3" },
    ],
  },
] as const;

function matchSecret(secrets: ProviderSecretRow[], purpose: Purpose, provider: string) {
  return (
    secrets.find(
      (s) =>
        !s.revoked_at &&
        s.purpose === purpose &&
        s.provider.toLowerCase() === provider.toLowerCase(),
    ) ?? null
  );
}

function matchSecretsForRow(
  secrets: ProviderSecretRow[],
  provider: string,
  targets: readonly SlotTarget[],
) {
  return targets
    .map((t) => matchSecret(secrets, t.purpose, provider))
    .filter((s): s is ProviderSecretRow => Boolean(s));
}

function ProviderKeyRow({
  row,
  secrets,
  onSaved,
}: {
  row: (typeof FIXED_ROWS)[number];
  secrets: ProviderSecretRow[];
  onSaved: () => void;
}) {
  const { token } = useAuth();
  const { locale } = useUiPreferences();
  const existingList = matchSecretsForRow(secrets, row.provider, row.targets);
  const existing = existingList[0] ?? null;
  const allSet = existingList.length === row.targets.length;
  const [apiKey, setApiKey] = useState("");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState("");
  const [notice, setNotice] = useState("");

  const typeLabel = formatUiMessage(locale, row.typeKey);
  const modelLabel = formatUiMessage(locale, row.modelKey);

  const save = useCallback(async () => {
    if (!token || !apiKey.trim()) {
      return;
    }
    setBusy(true);
    setError("");
    setNotice("");
    try {
      const key = apiKey.trim();
      // One key → all targets (e.g. SiliconFlow embedding + rerank).
      await Promise.all(
        row.targets.map((t) =>
          upsertProviderSecret(token, {
            purpose: t.purpose,
            provider: row.provider,
            api_key: key,
            base_url: row.baseUrl,
            model_hint: t.modelHint,
          }),
        ),
      );
      setApiKey("");
      setNotice(formatUiMessage(locale, "settingsProviderSaved"));
      onSaved();
    } catch (err) {
      setError(describeAuthError(formatUiMessage(locale, "settingsProviderSaveFailed"), err, locale));
    } finally {
      setBusy(false);
    }
  }, [apiKey, locale, onSaved, row, token]);

  const revoke = useCallback(async () => {
    if (!token || existingList.length === 0) {
      return;
    }
    setBusy(true);
    setError("");
    try {
      await Promise.all(existingList.map((s) => revokeProviderSecret(token, s.id)));
      setNotice(formatUiMessage(locale, "settingsProviderCleared"));
      onSaved();
    } catch (err) {
      setError(describeAuthError(formatUiMessage(locale, "settingsProviderClearFailed"), err, locale));
    } finally {
      setBusy(false);
    }
  }, [existingList, locale, onSaved, token]);

  const placeholder = (() => {
    if (existing) {
      const fp = existing.key_fingerprint;
      if (allSet) {
        return formatUiMessage(locale, "settingsProviderConfiguredFp", { fp });
      }
      return formatUiMessage(locale, "settingsProviderPartialFp", { fp });
    }
    return formatUiMessage(locale, "settingsProviderPasteKey");
  })();

  return (
    <div className={styles.row} data-testid={`provider-row-${row.id}`}>
      <div className={styles.typeCell} title={typeLabel}>
        {typeLabel}
      </div>
      <div className={styles.modelCell} title={modelLabel}>
        {modelLabel}
      </div>
      <div className={styles.keyCell}>
        <input
          className={`app-input ${styles.keyInput}`}
          type="password"
          value={apiKey}
          autoComplete="off"
          placeholder={placeholder}
          onChange={(e) => setApiKey(e.target.value)}
          data-testid={`provider-key-${row.id}`}
        />
      </div>
      <div className={styles.actionsCell}>
        <button
          type="button"
          className="app-button-primary"
          disabled={busy || !apiKey.trim()}
          onClick={() => void save()}
        >
          {busy ? "…" : formatUiMessage(locale, "settingsProviderSave")}
        </button>
        {existingList.length > 0 ? (
          <button
            type="button"
            className="app-button-ghost"
            disabled={busy}
            onClick={() => void revoke()}
          >
            {formatUiMessage(locale, "settingsProviderClear")}
          </button>
        ) : null}
      </div>
      {error ? (
        <p className={`${styles.rowMsg} app-notice-banner`} role="alert">
          {error}
        </p>
      ) : null}
      {notice ? (
        <p className={styles.rowMsg} role="status">
          {notice}
        </p>
      ) : null}
    </div>
  );
}

export function ProvidersPanel() {
  const { token } = useAuth();
  const { locale } = useUiPreferences();
  const queryClient = useQueryClient();

  const secretsQuery = useQuery({
    queryKey: [...settingsKeys.billing(token), "provider-secrets"],
    enabled: Boolean(token),
    queryFn: () => listProviderSecrets(token as string),
  });

  const secrets = useMemo(
    () => secretsQuery.data?.secrets ?? [],
    [secretsQuery.data?.secrets],
  );

  const refresh = useCallback(() => {
    void queryClient.invalidateQueries({
      queryKey: [...settingsKeys.billing(token), "provider-secrets"],
    });
  }, [queryClient, token]);

  return (
    <div data-testid="settings-providers-panel" className={styles.panel}>
      <header className={styles.header}>
        <h2 className={shared.flushTitle}>
          {formatUiMessage(locale, "settingsProviderTitle")}
        </h2>
        <p className={shared.mutedText}>
          {formatUiMessage(locale, "settingsProviderSubtitle")}
        </p>
      </header>

      {secretsQuery.isLoading ? (
        <p className={shared.mutedText}>{formatUiMessage(locale, "settingsProviderLoading")}</p>
      ) : null}

      <div
        className={styles.table}
        role="table"
        aria-label={formatUiMessage(locale, "settingsProviderKeysLabel")}
      >
        <div className={styles.tableHead} role="row">
          <span role="columnheader">{formatUiMessage(locale, "settingsProviderTypeColumn")}</span>
          <span role="columnheader">{formatUiMessage(locale, "settingsProviderModelColumn")}</span>
          <span role="columnheader">API Key</span>
          <span role="columnheader">{formatUiMessage(locale, "settingsProviderActionsColumn")}</span>
        </div>
        {FIXED_ROWS.map((row) => (
          <ProviderKeyRow key={row.id} row={row} secrets={secrets} onSaved={refresh} />
        ))}
      </div>
    </div>
  );
}
