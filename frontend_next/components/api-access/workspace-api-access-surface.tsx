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
import { getApiBaseUrl } from "../../lib/http/request";
import { isTauri } from "../../lib/runtime/tauri-ipc";

import styles from "./workspace-api-access-surface.module.css";

const LOCAL_DESKTOP_API_BASE = "http://127.0.0.1:18080";

type WorkspaceApiAccessSurfaceProps = {
  workspaceId: string;
  /** When true, drop page chrome so the surface can sit inside a modal. */
  embedded?: boolean;
};

function apiPermissionLabel(permission: string) {
  switch (permission) {
    case "index":
      return "索引";
    case "query":
      return "查询";
    default:
      return permission;
  }
}

function apiKeyStatusLabel(isActive: boolean) {
  return isActive ? "生效中" : "已撤销";
}

function errorMessage(error: unknown, fallback: string) {
  if (error instanceof Error && error.message.trim()) {
    return `${fallback}：${error.message}`;
  }

  return fallback;
}

const workspaceIdPattern = /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/i;

function getWorkspaceIdValidationError(workspaceId: string) {
  if (!workspaceId) {
    return "Workspace ID 缺失，未发起 API 请求。请检查路由参数是否正确。";
  }

  if (!workspaceIdPattern.test(workspaceId)) {
    return `Workspace ID 无效（${workspaceId} 不是 UUID），未发起 API 请求。请检查路由参数是否正确。`;
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

function buildAgentMcpSnippet(apiBase: string): string {
  return JSON.stringify(
    {
      mcpServers: {
        "context-os": {
          command: "context-os-mcp",
          env: {
            CONTEXT_OS_API_BASE: apiBase,
            CONTEXT_OS_API_KEY: "<paste_workspace_api_key>",
          },
        },
      },
    },
    null,
    2,
  );
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
  embedded = false,
}: WorkspaceApiAccessSurfaceProps) {
  const auth = useAuth();
  const workspaceIdValue = typeof workspaceId === "string" ? workspaceId.trim() : "";
  const workspaceIdValidationError = getWorkspaceIdValidationError(workspaceIdValue);
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
  // Resolve after mount so desktop (Tauri) base is correct and SSR/hydration stay stable.
  const [agentApiBase, setAgentApiBase] = useState(LOCAL_DESKTOP_API_BASE);
  const mcpSnippet = useMemo(() => buildAgentMcpSnippet(agentApiBase), [agentApiBase]);
  const mcpEndpoint = `${agentApiBase}/api/v1/mcp`;
  const desktopLocal =
    agentApiBase.includes("127.0.0.1") || agentApiBase.includes("localhost");

  async function handleCopy(label: string, value: string) {
    const ok = await copyText(value);
    setCopyFeedback(ok ? `已复制：${label}` : `复制失败，请手动选择 ${label}`);
    window.setTimeout(() => setCopyFeedback(""), 2500);
  }

  async function handleMintAgentToken() {
    if (!auth.token) {
      setError("登录状态失效，请重新登录后再签发 agent token。");
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
      }>("/api/auth/agent-token", { method: "POST", body: JSON.stringify({ ttl_minutes: 120 }) }, auth.token);
      const token = payload.data?.token?.trim() ?? "";
      if (!token) {
        throw new Error(payload.message ?? payload.error ?? "未返回 token");
      }
      setAgentUserToken(token);
      setCopyFeedback(
        payload.data?.expires_at
          ? `已签发 agent token（约 ${payload.data.ttl_minutes ?? 120} 分钟，至 ${payload.data.expires_at}）`
          : "已签发 agent token",
      );
      window.setTimeout(() => setCopyFeedback(""), 4000);
    } catch (mintError) {
      setError(errorMessage(mintError, "签发 agent token 失败"));
    } finally {
      setMintingToken(false);
    }
  }

  useEffect(() => {
    setAgentApiBase(resolveAgentApiBase());
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
          setError(errorMessage(loadError, "加载 API 密钥失败"));
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
  }, [auth.token, workspaceIdValidationError, workspaceIdValue]);

  async function handleCreate(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();

    if (workspaceIdValidationError) {
      setError(workspaceIdValidationError);
      return;
    }

    if (!auth.token) {
      setError("登录状态失效，请重新登录。");
      return;
    }

    const trimmedName = nameDraft.trim();

    if (!trimmedName) {
      setError("请输入密钥名称。");
      return;
    }

    const parsedRateLimit = Number.parseInt(rateLimitDraft.trim(), 10);

    if (!Number.isFinite(parsedRateLimit) || parsedRateLimit <= 0) {
      setError("速率限制必须是正整数。");
      return;
    }

    const permissions = [
      indexPermissionEnabled ? "index" : null,
      queryPermissionEnabled ? "query" : null,
    ].filter((permission): permission is string => permission !== null);

    if (permissions.length === 0) {
      setError("请至少选择一种权限。");
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
      setPlaintextKey(response.plaintext_key);
      setKeys((current) => [response.api_key, ...current.filter((key) => key.id !== response.api_key.id)]);
      setNameDraft("");
      setExpiresAtDraft("");
    } catch (createError) {
      setError(errorMessage(createError, "创建 API 密钥失败"));
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
      setError("登录状态失效，请重新登录。");
      return;
    }

    setRevokingKeyId(keyId);
    setError("");

    try {
      await revokeApiKey(auth.token, workspaceIdValue, keyId);
      setKeys((current) => current.filter((key) => key.id !== keyId));
    } catch (revokeError) {
      setError(errorMessage(revokeError, "撤销 API 密钥失败"));
    } finally {
      setRevokingKeyId(null);
    }
  }

  const body = (
    <>
      {embedded ? null : (
        <header className={styles.header}>
          <Link className="app-link app-link-muted" href={`/dashboard/${workspaceIdValue}`}>
            返回 Workspace
          </Link>
          <div className={styles.intro}>
            <p className={styles.overline}>Workspace API</p>
            <div>
              <h1 className="app-page-title">API 访问</h1>
              <p className="app-page-subtitle">
                为这个 Workspace 创建 API 密钥，并查看面向开发者与 LLM agents 的接入说明。
              </p>
            </div>
            <p className={styles.note}>
              在 API 路径中，这个 Workspace 对应 <code>workspace_id</code>。
            </p>
          </div>
        </header>
      )}

      {embedded ? (
        <p className="app-page-subtitle" style={{ marginTop: 0 }}>
          为这个 Workspace 创建 API 密钥。下方提供人类说明与 agent 稳定文档入口。
        </p>
      ) : null}

      {error ? <p className="app-notice-banner">{error}</p> : null}

      <div className={styles.stackLarge}>
        <section className={`app-surface-card ${styles.card}`}>
          <div>
            <h2 className={styles.cardTitle}>创建 API 密钥</h2>
            <p className="app-page-subtitle">按 Workspace 粒度创建密钥，控制索引能力和频率限制。</p>
          </div>
          <form className={styles.form} onSubmit={handleCreate}>
            <label>
              <span className="app-form-label">密钥名称</span>
              <input
                aria-label="密钥名称"
                className="app-input"
                value={nameDraft}
                onChange={(event) => setNameDraft(event.target.value)}
              />
            </label>
            <fieldset className={styles.fieldset}>
              <legend className="app-form-label">权限</legend>
              <div className={styles.checkboxGroup}>
                <label className={styles.checkboxLabel}>
                  <input
                    checked={indexPermissionEnabled}
                    type="checkbox"
                    onChange={(event) => setIndexPermissionEnabled(event.target.checked)}
                  />
                  <span>索引（index）</span>
                </label>
                <label className={styles.checkboxLabel}>
                  <input
                    checked={queryPermissionEnabled}
                    type="checkbox"
                    onChange={(event) => setQueryPermissionEnabled(event.target.checked)}
                  />
                  <span>查询（query）</span>
                </label>
              </div>
            </fieldset>
            <p className={styles.note}>
              API 密钥只支持资料管理与 RAG 查询，聊天和搜索代理默认不可用。
            </p>
            <label>
              <span className="app-form-label">速率限制（RPM）</span>
              <input
                aria-label="速率限制（RPM）"
                className="app-input"
                inputMode="numeric"
                type="number"
                value={rateLimitDraft}
                onChange={(event) => setRateLimitDraft(event.target.value)}
              />
            </label>
            <label>
              <span className="app-form-label">过期时间 RFC3339（可选）</span>
              <input
                aria-label="过期时间 RFC3339（可选）"
                className="app-input"
                placeholder="2026-03-31T18:00:00Z"
                value={expiresAtDraft}
                onChange={(event) => setExpiresAtDraft(event.target.value)}
              />
            </label>
            <div className="app-button-row">
              <button className="app-button-primary" disabled={submitting} type="submit">
                {submitting ? "创建中..." : "创建密钥"}
              </button>
            </div>
          </form>

          {plaintextKey ? (
            <div className={`app-inline-surface ${styles.stack}`}>
              <div>
                <strong>新密钥</strong>
                <p className={styles.mutedTextSpaced}>
                  明文只会返回这一次。
                </p>
              </div>
              <pre className={styles.codeBlock}>{plaintextKey}</pre>
            </div>
          ) : null}
        </section>
      </div>

      <section className={`app-surface-card ${styles.card}`}>
        <div className={styles.sectionHeader}>
          <div>
            <h2 className={styles.cardTitleCompact}>已创建密钥</h2>
            <p className="app-page-subtitle">仅展示当前仍处于生效状态的 Workspace API 密钥。</p>
          </div>
          {loading ? <span className={styles.muted}>加载中...</span> : null}
        </div>

        {!loading && keys.length === 0 ? (
          <div className="app-inline-surface">
            <p className={styles.mutedText}>还没有 API 密钥。</p>
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
                  {key.key_prefix} · {key.permissions.map(apiPermissionLabel).join(" / ")} · {key.rate_limit_rpm} RPM
                </div>
                <div className={styles.keyMetaSpaced}>
                  {apiKeyStatusLabel(key.is_active)} · 过期时间 {key.expires_at ?? "永不"} · 最近使用{" "}
                  {key.last_used_at ?? "从未"}
                </div>
              </div>
              <button
                className="app-button-secondary"
                disabled={revokingKeyId === key.id}
                type="button"
                onClick={() => void handleRevoke(key.id)}
              >
                {revokingKeyId === key.id ? "撤销中..." : "撤销"}
              </button>
            </div>
          ))}
        </div>
      </section>

      {/* Agent setup: copy base URL / workspace_id / MCP snippet (P0 local agent access). */}
      <section className={`app-surface-card ${styles.card}`} data-testid="api-access-agent-setup-card">
        <div>
          <h2 className={styles.cardTitle}>给 Agent 用</h2>
          <p className="app-page-subtitle">
            把本工作区交给 Claude Code / Codex / Cursor：先创建密钥（上方），再复制下列字段与 MCP 配置。
            {desktopLocal
              ? " 本机客户端默认 API 为 127.0.0.1:18080；请先确认客户端栈已启动。"
              : " 云端与本机使用同一 MCP 工具表，仅 base URL 不同。"}
          </p>
        </div>

        {copyFeedback ? <p className={styles.copyFeedback}>{copyFeedback}</p> : null}

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
              复制
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
              复制
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
              复制
            </button>
          </div>
        </div>

        <div className={`app-inline-surface ${styles.stack}`}>
          <div className={styles.sectionHeader}>
            <div>
              <strong>stdio MCP 配置片段</strong>
              <p className={styles.mutedTextSpaced}>
                粘贴到 Claude Code / Cursor 的 MCP 配置；将 <code>command</code> 换成本机{" "}
                <code>context-os-mcp</code> 路径，密钥用上方一次性明文替换。
              </p>
            </div>
            <button
              className="app-button-secondary"
              type="button"
              onClick={() => void handleCopy("MCP 配置", mcpSnippet)}
            >
              复制配置
            </button>
          </div>
          <pre className={styles.codeBlock} data-testid="agent-mcp-snippet">
            {mcpSnippet}
          </pre>
          <div className={`app-inline-surface ${styles.stack}`}>
            <div className={styles.sectionHeader}>
              <div>
                <strong>用户态 agent token（建库）</strong>
                <p className={styles.mutedTextSpaced}>
                  短时用户 JWT（默认 120 分钟）。export 为 <code>CONTEXT_OS_USER_TOKEN</code> 后可用{" "}
                  <code>context-os workspace create</code>；工作区密钥仍不能建库。分享仍走 UI。
                </p>
              </div>
              <button
                className="app-button-secondary"
                type="button"
                disabled={mintingToken || !auth.token}
                onClick={() => void handleMintAgentToken()}
              >
                {mintingToken ? "签发中..." : "签发 2h token"}
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
                  复制 token
                </button>
              </>
            ) : null}
          </div>

          <p className={styles.note}>
            探活：<code>context-os status</code>。本机桌面可 <code>context-os auth from-desktop --save</code>{" "}
            写入 <code>~/.config/context-os/user.token</code>（CLI/MCP 自动加载）。脚本：{" "}
            <code>context-os ingest</code> / <code>ask</code> / <code>share enable</code>（需 user
            token）。工具参数 <code>workspace_id</code> 须与本页一致。
          </p>
        </div>
      </section>

      {/* W2 #12: docs card stays visible in modal (embedded) and full page. */}
      <section className={`app-surface-card ${styles.card}`} data-testid="api-access-docs-card">
        <div>
          <h2 className={styles.cardTitle}>For LLM Agents</h2>
          <p className="app-page-subtitle">给要接入这个 Workspace 的 agent 看的入口卡。先理解边界，再读取稳定文档。</p>
        </div>
        <div className={`app-inline-surface ${styles.agentCard}`}>
          <div className={styles.agentIntro}>
            <p className={styles.overlineSmall}>Agent onboarding</p>
            <div>
              <strong>推荐顺序</strong>
              <p className={styles.mutedTextSpaced}>
                如果你的 agent 要直连这个 Workspace，先读人类说明确认作用域，再读取稳定 agent 文档执行。
              </p>
            </div>
          </div>
          <div className={styles.stack}>
            <div className={styles.step}>
              <div className={styles.stepBadge}>1</div>
              <div className={styles.stepBody}>
                <strong>先读说明页</strong>
                <p className={styles.mutedText}>
                  看清支持范围、认证方式，以及 Workspace 与 <code>workspace_id</code> 的映射。
                </p>
                <Link className="app-link" href="/help/api-access">
                  /help/api-access
                </Link>
              </div>
            </div>
            <div className={styles.step}>
              <div className={styles.stepBadgeMuted}>2</div>
              <div className={styles.stepBody}>
                <strong>再读稳定 agent 文档</strong>
                <p className={styles.mutedText}>
                  这份链接适合 agent 直接抓取，内容更短，也更适合程序化读取。
                </p>
                <Link className="app-link" href="/docs/api-access-for-agents.md">
                  /docs/api-access-for-agents.md
                </Link>
              </div>
            </div>
          </div>
        </div>
      </section>
    </>
  );

  if (embedded) {
    return <div className={styles.container}>{body}</div>;
  }

  return (
    <main className="app-page-shell">
      <div className={`app-page-center ${styles.container}`}>{body}</div>
    </main>
  );
}
