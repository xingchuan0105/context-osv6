"use client";

import Link from "next/link";
import { type FormEvent, useEffect, useMemo, useState } from "react";

import { useAuth } from "../../lib/auth/context";
import {
  createApiKey,
  listApiKeys,
  revokeApiKey,
  type ApiKeyRow,
  type CreateApiKeyRequest,
} from "../../lib/api-access/client";
import {
  AGENT_DOCS_PATH,
  HUMAN_DOCS_PATH,
  absoluteDocUrl,
  buildAgentPack,
} from "../../lib/api-access/agent-pack";
import { getApiBaseUrl } from "../../lib/http/request";
import { formatUiMessage } from "../../lib/i18n/messages";
import type { UiLocale } from "../../lib/i18n/config";
import { isTauri } from "../../lib/runtime/tauri-ipc";
import { useUiPreferences } from "../../lib/ui-preferences";

import styles from "./workspace-api-access-surface.module.css";

const LOCAL_DESKTOP_API_BASE = "http://127.0.0.1:18080";

type WorkspaceApiAccessSurfaceProps = {
  workspaceId: string;
};

function apiPermissionLabel(permission: string, locale: UiLocale) {
  switch (permission) {
    case "index":
      return formatUiMessage(locale, "apiAccess.permShortIndex");
    case "query":
      return formatUiMessage(locale, "apiAccess.permShortQuery");
    default:
      return permission;
  }
}

function apiKeyStatusLabel(isActive: boolean, locale: UiLocale) {
  return isActive
    ? formatUiMessage(locale, "apiAccess.statusActive")
    : formatUiMessage(locale, "apiAccess.statusRevoked");
}

function errorMessage(error: unknown, fallback: string, locale: UiLocale) {
  if (error instanceof Error && error.message.trim()) {
    return formatUiMessage(locale, "apiAccess.errWithDetail", {
      fallback,
      detail: error.message,
    });
  }
  return fallback;
}

const workspaceIdPattern = /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/i;

function getWorkspaceIdValidationError(workspaceId: string, locale: UiLocale) {
  if (!workspaceId) {
    return formatUiMessage(locale, "apiAccess.errMissingWorkspaceId");
  }
  if (!workspaceIdPattern.test(workspaceId)) {
    return formatUiMessage(locale, "apiAccess.errInvalidWorkspaceId", { id: workspaceId });
  }
  return "";
}

/** Agent-facing public base (HTTP MCP / stdio wrapper). Desktop defaults to local stack. */
function resolveAgentApiBase(): string {
  const configured = getApiBaseUrl().trim();
  if (configured) {
    return configured.replace(/\/$/, "");
  }
  if (typeof window !== "undefined" && isTauri()) {
    return LOCAL_DESKTOP_API_BASE;
  }
  if (typeof window !== "undefined" && window.location?.origin) {
    return window.location.origin.replace(/\/$/, "");
  }
  return LOCAL_DESKTOP_API_BASE;
}

function resolveDocsOrigin(apiBase: string): string {
  if (typeof window !== "undefined" && window.location?.origin) {
    return window.location.origin.replace(/\/$/, "");
  }
  return apiBase.replace(/\/$/, "");
}

async function copyText(text: string): Promise<boolean> {
  try {
    if (typeof navigator !== "undefined" && navigator.clipboard?.writeText) {
      await navigator.clipboard.writeText(text);
      return true;
    }
  } catch {
    // fall through
  }
  return false;
}

export function WorkspaceApiAccessSurface({
  workspaceId,
}: WorkspaceApiAccessSurfaceProps) {
  const auth = useAuth();
  const { locale } = useUiPreferences();
  const workspaceIdValue = typeof workspaceId === "string" ? workspaceId.trim() : "";
  const workspaceIdValidationError = getWorkspaceIdValidationError(workspaceIdValue, locale);
  const [keys, setKeys] = useState<ApiKeyRow[]>([]);
  const [loading, setLoading] = useState(true);
  const [submitting, setSubmitting] = useState(false);
  const [revokingKeyId, setRevokingKeyId] = useState<string | null>(null);
  const [error, setError] = useState("");
  const [nameDraft, setNameDraft] = useState("");
  const [indexPermissionEnabled, setIndexPermissionEnabled] = useState(true);
  const [queryPermissionEnabled, setQueryPermissionEnabled] = useState(true);
  const [rateLimitDraft, setRateLimitDraft] = useState("60");
  const [expiresAtDraft, setExpiresAtDraft] = useState("");
  const [plaintextKey, setPlaintextKey] = useState("");
  const [copyFeedback, setCopyFeedback] = useState("");
  const [agentUserToken, setAgentUserToken] = useState("");
  const [mintingToken, setMintingToken] = useState(false);
  const [agentApiBase, setAgentApiBase] = useState(LOCAL_DESKTOP_API_BASE);
  const [docsOrigin, setDocsOrigin] = useState(LOCAL_DESKTOP_API_BASE);
  const [advancedOpen, setAdvancedOpen] = useState(false);

  const desktopLocal =
    agentApiBase.includes("127.0.0.1") || agentApiBase.includes("localhost");
  const hasPackKey = Boolean(plaintextKey.trim());
  const permissions = useMemo(
    () =>
      [indexPermissionEnabled ? "index" : null, queryPermissionEnabled ? "query" : null].filter(
        (p): p is string => p !== null,
      ),
    [indexPermissionEnabled, queryPermissionEnabled],
  );

  const agentPack = useMemo(
    () =>
      buildAgentPack({
        workspaceId: workspaceIdValue,
        apiBase: agentApiBase,
        apiKey: plaintextKey || null,
        permissions,
        docsOrigin,
        runtime: desktopLocal ? "desktop-local" : "cloud",
      }),
    [workspaceIdValue, agentApiBase, plaintextKey, permissions, docsOrigin, desktopLocal],
  );

  const docsHumanUrl = absoluteDocUrl(docsOrigin, HUMAN_DOCS_PATH);
  const docsAgentUrl = absoluteDocUrl(docsOrigin, AGENT_DOCS_PATH);
  const mcpEndpoint = `${agentApiBase.replace(/\/$/, "")}/api/v1/mcp`;

  async function handleCopy(label: string, value: string) {
    const ok = await copyText(value);
    setCopyFeedback(
      ok
        ? formatUiMessage(locale, "apiAccess.copied", { label })
        : formatUiMessage(locale, "apiAccess.copyFailed", { label }),
    );
    window.setTimeout(() => setCopyFeedback(""), 2500);
  }

  async function handleCopyPack() {
    if (!hasPackKey) {
      return;
    }
    const ok = await copyText(agentPack);
    setCopyFeedback(
      ok
        ? formatUiMessage(locale, "apiAccess.packCopied")
        : formatUiMessage(locale, "apiAccess.copyFailed", {
            label: formatUiMessage(locale, "apiAccess.packPreviewTitle"),
          }),
    );
    window.setTimeout(() => setCopyFeedback(""), 4000);
  }

  async function handleMintAgentToken() {
    if (!auth.token) {
      setError(formatUiMessage(locale, "apiAccess.errSessionMint"));
      return;
    }
    setMintingToken(true);
    setError("");
    try {
      const { request } = await import("../../lib/http/request");
      const payload = await request<{
        success?: boolean;
        data?: { token?: string; expires_at?: string; ttl_minutes?: number };
        error?: string;
        message?: string;
      }>(
        "/api/auth/agent-token",
        { method: "POST", body: JSON.stringify({ ttl_minutes: 120 }) },
        auth.token,
      );
      const token = payload.data?.token?.trim() ?? "";
      if (!token) {
        throw new Error(
          payload.message ?? payload.error ?? formatUiMessage(locale, "apiAccess.errNoToken"),
        );
      }
      setAgentUserToken(token);
      setCopyFeedback(
        payload.data?.expires_at
          ? formatUiMessage(locale, "apiAccess.mintedWithExpiry", {
              minutes: String(payload.data.ttl_minutes ?? 120),
              expires: payload.data.expires_at,
            })
          : formatUiMessage(locale, "apiAccess.minted"),
      );
      window.setTimeout(() => setCopyFeedback(""), 4000);
    } catch (mintError) {
      setError(
        errorMessage(mintError, formatUiMessage(locale, "apiAccess.errMintToken"), locale),
      );
    } finally {
      setMintingToken(false);
    }
  }

  useEffect(() => {
    const base = resolveAgentApiBase();
    setAgentApiBase(base);
    setDocsOrigin(resolveDocsOrigin(base));
  }, []);

  useEffect(() => {
    let cancelled = false;

    async function loadApiKeys() {
      if (workspaceIdValidationError) {
        setLoading(false);
        setKeys([]);
        setPlaintextKey("");
        setError(workspaceIdValidationError);
        return;
      }

      if (!auth.token) {
        setLoading(false);
        setKeys([]);
        setPlaintextKey("");
        setError("");
        return;
      }

      setLoading(true);
      setError("");

      try {
        const response = await listApiKeys(auth.token, workspaceIdValue);
        if (!cancelled) {
          setKeys(response.api_keys);
        }
      } catch (loadError) {
        if (!cancelled) {
          setError(
            errorMessage(loadError, formatUiMessage(locale, "apiAccess.errLoadKeys"), locale),
          );
        }
      } finally {
        if (!cancelled) {
          setLoading(false);
        }
      }
    }

    void loadApiKeys();

    return () => {
      cancelled = true;
    };
  }, [auth.token, locale, workspaceIdValidationError, workspaceIdValue]);

  async function handleCreate(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();

    if (workspaceIdValidationError) {
      setError(workspaceIdValidationError);
      return;
    }

    if (!auth.token) {
      setError(formatUiMessage(locale, "apiAccess.errSession"));
      return;
    }

    const trimmedName = nameDraft.trim();
    if (!trimmedName) {
      setError(formatUiMessage(locale, "apiAccess.errNameRequired"));
      return;
    }

    const parsedRateLimit = Number.parseInt(rateLimitDraft.trim(), 10);
    if (!Number.isFinite(parsedRateLimit) || parsedRateLimit <= 0) {
      setError(formatUiMessage(locale, "apiAccess.errRateLimit"));
      return;
    }

    if (permissions.length === 0) {
      setError(formatUiMessage(locale, "apiAccess.errPermissions"));
      return;
    }

    const requestBody: CreateApiKeyRequest = {
      name: trimmedName,
      permissions,
      rate_limit_rpm: parsedRateLimit,
    };
    const trimmedExpiresAt = expiresAtDraft.trim();
    if (trimmedExpiresAt) {
      requestBody.expires_at = trimmedExpiresAt;
    }

    setSubmitting(true);
    setError("");

    try {
      const response = await createApiKey(auth.token, workspaceIdValue, requestBody);
      const key = response.plaintext_key;
      setPlaintextKey(key);
      setKeys((current) => [
        response.api_key,
        ...current.filter((row) => row.id !== response.api_key.id),
      ]);
      setNameDraft("");
      setExpiresAtDraft("");

      // Auto-copy full pack with the new key injected (design §12 decision 2).
      const pack = buildAgentPack({
        workspaceId: workspaceIdValue,
        apiBase: agentApiBase,
        apiKey: key,
        permissions: response.api_key.permissions?.length
          ? response.api_key.permissions
          : permissions,
        docsOrigin,
        runtime: desktopLocal ? "desktop-local" : "cloud",
      });
      const ok = await copyText(pack);
      setCopyFeedback(
        ok
          ? formatUiMessage(locale, "apiAccess.packCopied")
          : formatUiMessage(locale, "apiAccess.copied", {
              label: formatUiMessage(locale, "apiAccess.newKeyTitle"),
            }),
      );
      window.setTimeout(() => setCopyFeedback(""), 4000);
    } catch (createError) {
      setError(
        errorMessage(createError, formatUiMessage(locale, "apiAccess.errCreateKey"), locale),
      );
    } finally {
      setSubmitting(false);
    }
  }

  async function handleRevoke(keyId: string) {
    if (workspaceIdValidationError) {
      setError(workspaceIdValidationError);
      return;
    }
    if (!auth.token) {
      setError(formatUiMessage(locale, "apiAccess.errSession"));
      return;
    }

    setRevokingKeyId(keyId);
    setError("");

    try {
      await revokeApiKey(auth.token, workspaceIdValue, keyId);
      setKeys((current) => current.filter((key) => key.id !== keyId));
    } catch (revokeError) {
      setError(
        errorMessage(revokeError, formatUiMessage(locale, "apiAccess.errRevokeKey"), locale),
      );
    } finally {
      setRevokingKeyId(null);
    }
  }

  return (
    <div className={styles.container}>
      {error ? <p className="app-notice-banner">{error}</p> : null}
      {copyFeedback ? <p className={styles.copyFeedback}>{copyFeedback}</p> : null}

      {/* ① Connect / Agent pack — primary */}
      <section className={`app-surface-card ${styles.card}`} data-testid="api-access-agent-setup-card">
        <div>
          <h2 className={styles.cardTitle}>{formatUiMessage(locale, "apiAccess.agentTitle")}</h2>
          <p className="app-page-subtitle">
            {formatUiMessage(locale, "apiAccess.agentSubtitle")}{" "}
            {desktopLocal
              ? formatUiMessage(locale, "apiAccess.agentDesktopHint")
              : formatUiMessage(locale, "apiAccess.agentCloudHint")}
          </p>
        </div>

        <p className={hasPackKey ? styles.packStatusReady : styles.packStatusNeed}>
          {hasPackKey
            ? formatUiMessage(locale, "apiAccess.packReady")
            : formatUiMessage(locale, "apiAccess.packNeedKey")}
        </p>

        <div className="app-button-row">
          <button
            className="app-button-primary"
            type="button"
            data-testid="copy-agent-pack"
            disabled={!hasPackKey || !workspaceIdValue}
            title={
              hasPackKey
                ? formatUiMessage(locale, "apiAccess.copyPack")
                : formatUiMessage(locale, "apiAccess.copyPackDisabled")
            }
            onClick={() => void handleCopyPack()}
          >
            {hasPackKey
              ? formatUiMessage(locale, "apiAccess.copyPack")
              : formatUiMessage(locale, "apiAccess.copyPackDisabled")}
          </button>
        </div>

        <details className={styles.packDetails} open={hasPackKey}>
          <summary className={styles.packSummary}>
            {formatUiMessage(locale, "apiAccess.packPreviewTitle")}
          </summary>
          <pre className={styles.codeBlock} data-testid="agent-pack-preview">
            {agentPack}
          </pre>
        </details>
      </section>

      {/* ② Create key */}
      <section className={`app-surface-card ${styles.card}`}>
        <div>
          <h2 className={styles.cardTitle}>{formatUiMessage(locale, "apiAccess.createTitle")}</h2>
          <p className="app-page-subtitle">{formatUiMessage(locale, "apiAccess.createSubtitle")}</p>
        </div>
        <form className={styles.form} onSubmit={handleCreate}>
          <label>
            <span className="app-form-label">{formatUiMessage(locale, "apiAccess.nameLabel")}</span>
            <input
              aria-label={formatUiMessage(locale, "apiAccess.nameLabel")}
              className="app-input"
              value={nameDraft}
              onChange={(event) => setNameDraft(event.target.value)}
            />
          </label>
          <fieldset className={styles.fieldset}>
            <legend className="app-form-label">
              {formatUiMessage(locale, "apiAccess.permissionsLabel")}
            </legend>
            <div className={styles.checkboxGroup}>
              <label className={styles.checkboxLabel}>
                <input
                  checked={indexPermissionEnabled}
                  type="checkbox"
                  onChange={(event) => setIndexPermissionEnabled(event.target.checked)}
                />
                <span>{formatUiMessage(locale, "apiAccess.permIndex")}</span>
              </label>
              <label className={styles.checkboxLabel}>
                <input
                  checked={queryPermissionEnabled}
                  type="checkbox"
                  onChange={(event) => setQueryPermissionEnabled(event.target.checked)}
                />
                <span>{formatUiMessage(locale, "apiAccess.permQuery")}</span>
              </label>
            </div>
          </fieldset>
          <p className={styles.note}>{formatUiMessage(locale, "apiAccess.permNote")}</p>
          <details className={styles.advancedDetails}>
            <summary>{formatUiMessage(locale, "apiAccess.advancedTitle")}</summary>
            <div className={styles.form} style={{ marginTop: "0.75rem" }}>
              <label>
                <span className="app-form-label">
                  {formatUiMessage(locale, "apiAccess.rateLimitLabel")}
                </span>
                <input
                  aria-label={formatUiMessage(locale, "apiAccess.rateLimitLabel")}
                  className="app-input"
                  inputMode="numeric"
                  type="number"
                  value={rateLimitDraft}
                  onChange={(event) => setRateLimitDraft(event.target.value)}
                />
              </label>
              <label>
                <span className="app-form-label">
                  {formatUiMessage(locale, "apiAccess.expiresLabel")}
                </span>
                <input
                  aria-label={formatUiMessage(locale, "apiAccess.expiresLabel")}
                  className="app-input"
                  placeholder="2026-03-31T18:00:00Z"
                  value={expiresAtDraft}
                  onChange={(event) => setExpiresAtDraft(event.target.value)}
                />
              </label>
            </div>
          </details>
          <div className="app-button-row">
            <button className="app-button-primary" disabled={submitting} type="submit">
              {submitting
                ? formatUiMessage(locale, "apiAccess.creating")
                : formatUiMessage(locale, "apiAccess.createAction")}
            </button>
          </div>
        </form>

        {plaintextKey ? (
          <div className={`app-inline-surface ${styles.stack}`}>
            <div>
              <strong>{formatUiMessage(locale, "apiAccess.newKeyTitle")}</strong>
              <p className={styles.mutedTextSpaced}>
                {formatUiMessage(locale, "apiAccess.newKeyOnce")}
              </p>
            </div>
            <pre className={styles.codeBlock}>{plaintextKey}</pre>
          </div>
        ) : null}
      </section>

      {/* Key list */}
      <section className={`app-surface-card ${styles.card}`}>
        <div className={styles.sectionHeader}>
          <div>
            <h2 className={styles.cardTitleCompact}>
              {formatUiMessage(locale, "apiAccess.listTitle")}
            </h2>
            <p className="app-page-subtitle">{formatUiMessage(locale, "apiAccess.listSubtitle")}</p>
          </div>
          {loading ? (
            <span className={styles.muted}>{formatUiMessage(locale, "apiAccess.loading")}</span>
          ) : null}
        </div>

        {!loading && keys.length === 0 ? (
          <div className="app-inline-surface">
            <p className={styles.mutedText}>{formatUiMessage(locale, "apiAccess.empty")}</p>
          </div>
        ) : null}

        <div className={styles.stack}>
          {keys.map((key) => (
            <div
              className={`app-inline-surface ${styles.keyItem}`}
              data-testid="api-key-item"
              key={key.id}
            >
              <div className={styles.keyItemBody}>
                <div className={styles.keyName}>{key.name}</div>
                <div className={styles.keyMeta}>
                  {key.key_prefix} ·{" "}
                  {key.permissions.map((p) => apiPermissionLabel(p, locale)).join(" / ")} ·{" "}
                  {key.rate_limit_rpm} RPM
                </div>
                <div className={styles.keyMetaSpaced}>
                  {apiKeyStatusLabel(key.is_active, locale)} ·{" "}
                  {formatUiMessage(locale, "apiAccess.metaExpires", {
                    value: key.expires_at ?? formatUiMessage(locale, "apiAccess.never"),
                  })}{" "}
                  ·{" "}
                  {formatUiMessage(locale, "apiAccess.metaLastUsed", {
                    value: key.last_used_at ?? formatUiMessage(locale, "apiAccess.neverUsed"),
                  })}
                </div>
              </div>
              <button
                className="app-button-secondary"
                disabled={revokingKeyId === key.id}
                type="button"
                onClick={() => void handleRevoke(key.id)}
              >
                {revokingKeyId === key.id
                  ? formatUiMessage(locale, "apiAccess.revoking")
                  : formatUiMessage(locale, "apiAccess.revoke")}
              </button>
            </div>
          ))}
        </div>
      </section>

      {/* ③ Docs */}
      <section className={`app-surface-card ${styles.card}`} data-testid="api-access-docs-card">
        <div>
          <h2 className={styles.cardTitle}>{formatUiMessage(locale, "apiAccess.docsTitle")}</h2>
          <p className="app-page-subtitle">{formatUiMessage(locale, "apiAccess.docsSubtitle")}</p>
        </div>
        <div className={styles.docsGrid}>
          <div className={`app-inline-surface ${styles.copyRow}`}>
            <div className={styles.copyBody}>
              <span className={styles.overlineSmall}>
                {formatUiMessage(locale, "apiAccess.docsHumanLabel")}
              </span>
              <Link className="app-link" href={HUMAN_DOCS_PATH}>
                {docsHumanUrl}
              </Link>
            </div>
            <button
              className="app-button-secondary"
              type="button"
              onClick={() => void handleCopy("docs_human", docsHumanUrl)}
            >
              {formatUiMessage(locale, "apiAccess.copy")}
            </button>
          </div>
          <div className={`app-inline-surface ${styles.copyRow}`}>
            <div className={styles.copyBody}>
              <span className={styles.overlineSmall}>
                {formatUiMessage(locale, "apiAccess.docsAgentLabel")}
              </span>
              <Link className="app-link" href={AGENT_DOCS_PATH} data-testid="agent-docs-link">
                {docsAgentUrl}
              </Link>
            </div>
            <button
              className="app-button-secondary"
              type="button"
              onClick={() => void handleCopy("docs_agent", docsAgentUrl)}
            >
              {formatUiMessage(locale, "apiAccess.copy")}
            </button>
          </div>
        </div>
      </section>

      {/* ④ Advanced */}
      <section className={`app-surface-card ${styles.card}`}>
        <button
          type="button"
          className={styles.advancedToggle}
          aria-expanded={advancedOpen}
          onClick={() => setAdvancedOpen((v) => !v)}
        >
          <strong>{formatUiMessage(locale, "apiAccess.advancedTitle")}</strong>
          <span className={styles.mutedText}>
            {formatUiMessage(locale, "apiAccess.advancedHint")}
          </span>
        </button>
        {advancedOpen ? (
          <div className={styles.stack}>
            <div className={styles.copyGrid}>
              <div className={`app-inline-surface ${styles.copyRow}`}>
                <div className={styles.copyBody}>
                  <span className={styles.overlineSmall}>workspace_id</span>
                  <code className={styles.copyValue}>{workspaceIdValue || "—"}</code>
                </div>
                <button
                  className="app-button-secondary"
                  type="button"
                  disabled={!workspaceIdValue}
                  onClick={() => void handleCopy("workspace_id", workspaceIdValue)}
                >
                  {formatUiMessage(locale, "apiAccess.copy")}
                </button>
              </div>
              <div className={`app-inline-surface ${styles.copyRow}`}>
                <div className={styles.copyBody}>
                  <span className={styles.overlineSmall}>API base URL</span>
                  <code className={styles.copyValue}>{agentApiBase}</code>
                </div>
                <button
                  className="app-button-secondary"
                  type="button"
                  onClick={() => void handleCopy("API base URL", agentApiBase)}
                >
                  {formatUiMessage(locale, "apiAccess.copy")}
                </button>
              </div>
              <div className={`app-inline-surface ${styles.copyRow}`}>
                <div className={styles.copyBody}>
                  <span className={styles.overlineSmall}>HTTP MCP</span>
                  <code className={styles.copyValue}>{mcpEndpoint}</code>
                </div>
                <button
                  className="app-button-secondary"
                  type="button"
                  onClick={() => void handleCopy("HTTP MCP", mcpEndpoint)}
                >
                  {formatUiMessage(locale, "apiAccess.copy")}
                </button>
              </div>
            </div>

            <div className={`app-inline-surface ${styles.stack}`}>
              <div className={styles.sectionHeader}>
                <div>
                  <strong>{formatUiMessage(locale, "apiAccess.agentTokenTitle")}</strong>
                  <p className={styles.mutedTextSpaced}>
                    {formatUiMessage(locale, "apiAccess.agentTokenHint")}
                  </p>
                </div>
                <button
                  className="app-button-secondary"
                  type="button"
                  disabled={mintingToken || !auth.token}
                  onClick={() => void handleMintAgentToken()}
                >
                  {mintingToken
                    ? formatUiMessage(locale, "apiAccess.minting")
                    : formatUiMessage(locale, "apiAccess.mintToken")}
                </button>
              </div>
              {agentUserToken ? (
                <>
                  <pre className={styles.codeBlock} data-testid="agent-user-token">
                    {agentUserToken}
                  </pre>
                  <button
                    className="app-button-secondary"
                    type="button"
                    onClick={() => void handleCopy("agent token", agentUserToken)}
                  >
                    {formatUiMessage(locale, "apiAccess.copyToken")}
                  </button>
                </>
              ) : null}
              <p className={styles.note}>{formatUiMessage(locale, "apiAccess.agentProbeNote")}</p>
            </div>
          </div>
        ) : null}
      </section>
    </div>
  );
}
