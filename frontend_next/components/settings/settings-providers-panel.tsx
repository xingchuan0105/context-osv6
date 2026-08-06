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
import styles from "./settings-billing-panel.module.css";
import shared from "./settings-ui-shared.module.css";

type Purpose = "llm" | "embedding" | "rerank";

type ProviderPreset = {
  id: string;
  label: string;
  baseUrl: string;
  models: string[];
  docsUrl: string;
  /** Only listed purposes may use this provider. */
  purposes: Purpose[];
};

/**
 * LiteLLM-oriented catalog. Embedding/rerank stay on system default (SiliconFlow);
 * LLM may use several OpenAI-compatible gateways.
 */
const PROVIDER_PRESETS: ProviderPreset[] = [
  {
    id: "siliconflow",
    label: "SiliconFlow",
    baseUrl: "https://api.siliconflow.cn/v1",
    models: [
      "deepseek-ai/DeepSeek-V3",
      "Qwen/Qwen2.5-72B-Instruct",
      "BAAI/bge-m3",
      "BAAI/bge-reranker-v2-m3",
    ],
    docsUrl: "https://docs.siliconflow.cn/",
    purposes: ["llm", "embedding", "rerank"],
  },
  {
    id: "openai",
    label: "OpenAI",
    baseUrl: "https://api.openai.com/v1",
    models: ["gpt-4o-mini", "gpt-4o", "gpt-4.1-mini"],
    docsUrl: "https://platform.openai.com/docs/models",
    purposes: ["llm"],
  },
  {
    id: "deepseek",
    label: "DeepSeek",
    baseUrl: "https://api.deepseek.com",
    models: ["deepseek-chat", "deepseek-reasoner"],
    docsUrl: "https://api-docs.deepseek.com/",
    purposes: ["llm"],
  },
  {
    id: "openrouter",
    label: "OpenRouter",
    baseUrl: "https://openrouter.ai/api/v1",
    models: ["openai/gpt-4o-mini", "anthropic/claude-3.5-sonnet", "google/gemini-2.0-flash-001"],
    docsUrl: "https://openrouter.ai/docs",
    purposes: ["llm"],
  },
  {
    id: "custom",
    label: "Custom (OpenAI-compatible)",
    baseUrl: "",
    models: [],
    docsUrl: "https://docs.litellm.ai/docs/providers",
    purposes: ["llm"],
  },
];

const PURPOSE_META: Record<
  Purpose,
  { titleKey: string; duck: { zh: string; en: string }; docsExtra: string }
> = {
  llm: {
    titleKey: "LLM",
    duck: {
      zh: "小黄鸭：LLM 是「会说话的大脑」。你问一句，它先想再答。这里填的是对话模型的钥匙和地址；换了钥匙就等于换了大脑，别的（embedding/rerank）先别动。",
      en: "Rubber duck: the LLM is the talking brain. You ask; it thinks and answers. Put the chat-model key and base URL here. Do not change embedding/rerank unless you know why.",
    },
    docsExtra: "https://docs.litellm.ai/docs/providers",
  },
  embedding: {
    titleKey: "Embedding",
    duck: {
      zh: "小黄鸭：Embedding 是「把文字变成数字指纹」。检索时用同一套指纹才能对上号。平台默认 SiliconFlow；这里只允许和系统默认同一家，避免索引与查询两套向量对不上。",
      en: "Rubber duck: embeddings turn text into number fingerprints. Search only works if index and query use the same family. We lock this to SiliconFlow (system default) so vectors stay compatible.",
    },
    docsExtra: "https://docs.siliconflow.cn/cn/api-reference/embeddings/create-embeddings",
  },
  rerank: {
    titleKey: "Reranker",
    duck: {
      zh: "小黄鸭：Reranker 是「二审官」——先粗检索一堆，再精排谁最相关。也必须和默认同一家（SiliconFlow），否则精排模型看不懂粗检索的语义空间。",
      en: "Rubber duck: the reranker is the second-pass judge after rough retrieval. Stay on SiliconFlow so the judge speaks the same language as the index.",
    },
    docsExtra: "https://docs.siliconflow.cn/cn/api-reference/rerank/create-rerank",
  },
};

function secretFor(secrets: ProviderSecretRow[], purpose: Purpose) {
  return secrets.find((s) => s.purpose === purpose && !s.revoked_at) ?? null;
}

function PurposeCard({
  purpose,
  secrets,
  onSaved,
}: {
  purpose: Purpose;
  secrets: ProviderSecretRow[];
  onSaved: () => void;
}) {
  const { token } = useAuth();
  const { locale } = useUiPreferences();
  const allowed = PROVIDER_PRESETS.filter((p) => p.purposes.includes(purpose));
  const existing = secretFor(secrets, purpose);
  const defaultPreset =
    allowed.find((p) => p.id === (existing?.provider ?? "siliconflow")) ?? allowed[0];

  const [providerId, setProviderId] = useState(defaultPreset?.id ?? "siliconflow");
  const preset = allowed.find((p) => p.id === providerId) ?? allowed[0];
  const [baseUrl, setBaseUrl] = useState(existing?.base_url || preset?.baseUrl || "");
  const [model, setModel] = useState(existing?.model_hint || preset?.models[0] || "");
  const [apiKey, setApiKey] = useState("");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState("");
  const [notice, setNotice] = useState("");

  const meta = PURPOSE_META[purpose];
  const lockedToSilicon = purpose === "embedding" || purpose === "rerank";

  const onProviderChange = (id: string) => {
    if (lockedToSilicon && id !== "siliconflow") {
      return;
    }
    setProviderId(id);
    const next = allowed.find((p) => p.id === id);
    if (next) {
      setBaseUrl(next.baseUrl);
      if (next.models[0]) {
        setModel(next.models[0]);
      }
    }
  };

  const save = useCallback(async () => {
    if (!token || !apiKey.trim()) {
      return;
    }
    setBusy(true);
    setError("");
    setNotice("");
    try {
      await upsertProviderSecret(token, {
        purpose,
        provider: providerId === "custom" ? "custom" : providerId,
        api_key: apiKey.trim(),
        base_url: baseUrl.trim() || undefined,
        model_hint: model.trim() || undefined,
      });
      setApiKey("");
      setNotice(locale === "zh-CN" ? "已保存" : "Saved");
      onSaved();
    } catch (err) {
      setError(describeAuthError(locale === "zh-CN" ? "保存失败" : "Save failed", err, locale));
    } finally {
      setBusy(false);
    }
  }, [apiKey, baseUrl, locale, model, onSaved, providerId, purpose, token]);

  const revoke = useCallback(async () => {
    if (!token || !existing) {
      return;
    }
    setBusy(true);
    setError("");
    try {
      await revokeProviderSecret(token, existing.id);
      onSaved();
    } catch (err) {
      setError(describeAuthError(locale === "zh-CN" ? "撤销失败" : "Revoke failed", err, locale));
    } finally {
      setBusy(false);
    }
  }, [existing, locale, onSaved, token]);

  return (
    <section className={`app-inline-surface ${styles.planSection}`} data-testid={`provider-${purpose}`}>
      <div className={`app-inline-row ${styles.headerRow}`}>
        <div className={shared.headerText}>
          <h2 className={shared.flushTitle}>{meta.titleKey} Provider</h2>
          <p className={shared.mutedText}>
            {locale === "zh-CN" ? meta.duck.zh : meta.duck.en}
          </p>
          <p className={shared.mutedText}>
            <a className="app-link" href={meta.docsExtra} rel="noreferrer" target="_blank">
              {locale === "zh-CN" ? "文档" : "Docs"}
            </a>
            {preset?.docsUrl ? (
              <>
                {" · "}
                <a className="app-link" href={preset.docsUrl} rel="noreferrer" target="_blank">
                  {preset.label}
                </a>
              </>
            ) : null}
          </p>
        </div>
      </div>

      {error ? <p className="app-notice-banner">{error}</p> : null}
      {notice ? (
        <p className="app-inline-surface" role="status">
          {notice}
        </p>
      ) : null}

      <div className={`app-inline-surface ${styles.planCard}`}>
        {existing ? (
          <p className={shared.mutedText} data-testid={`provider-${purpose}-active`}>
            {locale === "zh-CN" ? "当前已配置：" : "Configured: "}
            <strong>
              {existing.provider}
              {existing.model_hint ? ` · ${existing.model_hint}` : ""}
            </strong>{" "}
            ({existing.key_fingerprint})
            <button className="app-link" disabled={busy} type="button" onClick={() => void revoke()}>
              {locale === "zh-CN" ? "撤销" : "Revoke"}
            </button>
          </p>
        ) : (
          <p className={shared.mutedText}>
            {locale === "zh-CN" ? "尚未配置自定义密钥，将使用平台默认。" : "No custom secret — platform default applies."}
          </p>
        )}

        <label className={shared.mutedText}>
          Provider
          <select
            className="app-input"
            disabled={lockedToSilicon}
            value={lockedToSilicon ? "siliconflow" : providerId}
            onChange={(e) => onProviderChange(e.target.value)}
          >
            {allowed.map((p) => (
              <option key={p.id} value={p.id}>
                {p.label}
                {lockedToSilicon && p.id !== "siliconflow" ? " (locked)" : ""}
              </option>
            ))}
          </select>
        </label>

        <label className={shared.mutedText}>
          Base URL
          <input
            className="app-input"
            value={baseUrl}
            onChange={(e) => setBaseUrl(e.target.value)}
            placeholder={preset?.baseUrl || "https://…/v1"}
          />
        </label>

        <label className={shared.mutedText}>
          Model
          {preset && preset.models.length > 0 ? (
            <select
              className="app-input"
              value={preset.models.includes(model) ? model : ""}
              onChange={(e) => {
                if (e.target.value) {
                  setModel(e.target.value);
                }
              }}
            >
              <option value="">
                {locale === "zh-CN" ? "— 选择或下方手填 —" : "— pick or type below —"}
              </option>
              {preset.models.map((m) => (
                <option key={m} value={m}>
                  {m}
                </option>
              ))}
            </select>
          ) : null}
          <input
            className="app-input"
            value={model}
            onChange={(e) => setModel(e.target.value)}
            placeholder={locale === "zh-CN" ? "模型名（可手填）" : "Model id (editable)"}
            list={`models-${purpose}`}
          />
          {preset && preset.models.length > 0 ? (
            <datalist id={`models-${purpose}`}>
              {preset.models.map((m) => (
                <option key={m} value={m} />
              ))}
            </datalist>
          ) : null}
        </label>

        <label className={shared.mutedText}>
          API Key
          <input
            className="app-input"
            type="password"
            value={apiKey}
            onChange={(e) => setApiKey(e.target.value)}
            placeholder="sk-…"
            autoComplete="off"
          />
        </label>

        <button
          type="button"
          className="app-button-primary"
          disabled={busy || !apiKey.trim()}
          onClick={() => void save()}
        >
          {busy ? "…" : locale === "zh-CN" ? "保存" : "Save"}
        </button>
      </div>
    </section>
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
    <div data-testid="settings-providers-panel">
      <section className={`app-inline-surface ${styles.planSection}`}>
        <div className={shared.headerText}>
          <h2 className={shared.flushTitle}>
            {locale === "zh-CN" ? "自定义 Provider" : "Custom providers"}
          </h2>
          <p className={shared.mutedText}>
            {locale === "zh-CN"
              ? "按 LiteLLM 兼容方式配置三路：对话 LLM、向量 Embedding、重排 Reranker。URL 已预填可改；模型可下拉也可手填。Embedding / Reranker 锁定 SiliconFlow（与系统默认一致），避免向量空间错位。"
              : "Configure three paths the LiteLLM way: chat LLM, embeddings, and reranker. Base URLs are prefilled but editable; models support dropdown + free text. Embedding and reranker stay on SiliconFlow (system default) so vector spaces match."}
          </p>
          <p className={shared.mutedText}>
            <a
              className="app-link"
              href="https://docs.litellm.ai/docs/providers"
              rel="noreferrer"
              target="_blank"
            >
              LiteLLM providers
            </a>
            {" · "}
            <a
              className="app-link"
              href="https://docs.siliconflow.cn/"
              rel="noreferrer"
              target="_blank"
            >
              SiliconFlow
            </a>
          </p>
        </div>
      </section>

      {secretsQuery.isLoading ? (
        <p className={shared.mutedText}>{locale === "zh-CN" ? "加载中…" : "Loading…"}</p>
      ) : null}

      <PurposeCard purpose="llm" secrets={secrets} onSaved={refresh} />
      <PurposeCard purpose="embedding" secrets={secrets} onSaved={refresh} />
      <PurposeCard purpose="rerank" secrets={secrets} onSaved={refresh} />
    </div>
  );
}
