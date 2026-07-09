"use client";

import Link from "next/link";
import { type FormEvent, useEffect, useState } from "react";

import { useAuth } from "../../lib/auth/context";
import {
  createApiKey,
  listApiKeys,
  revokeApiKey,
  type ApiKeyRow,
  type CreateApiKeyRequest,
} from "../../lib/api-access/client";

type WorkspaceApiAccessSurfaceProps = {
  workspaceId: string;
};

const codeBlockStyle = {
  border: "1px solid hsl(var(--border))",
  borderRadius: "1rem",
  background: "hsl(var(--surface-muted))",
  margin: 0,
  overflowX: "auto" as const,
  padding: "1rem",
  whiteSpace: "pre-wrap" as const,
  wordBreak: "break-all" as const,
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

export function WorkspaceApiAccessSurface({ workspaceId }: WorkspaceApiAccessSurfaceProps) {
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

  return (
    <main className="app-page-shell">
      <div className="app-page-center" style={{ display: "grid", gap: "1.25rem", maxWidth: "56rem" }}>
        <header style={{ display: "grid", gap: "0.85rem" }}>
          <Link className="app-link app-link-muted" href={`/dashboard/${workspaceIdValue}`}>
            返回 Workspace
          </Link>
          <div style={{ display: "grid", gap: "0.5rem", maxWidth: "42rem" }}>
            <p
              style={{
                color: "hsl(var(--muted-foreground))",
                fontSize: "0.82rem",
                fontWeight: 700,
                letterSpacing: "0.08em",
                margin: 0,
                textTransform: "uppercase",
              }}
            >
              Workspace API
            </p>
            <div>
              <h1 className="app-page-title">API 访问</h1>
              <p className="app-page-subtitle">
                为这个 Workspace 创建 API 密钥，并查看面向开发者与 LLM agents 的接入说明。
              </p>
            </div>
            <p style={{ color: "hsl(var(--muted-foreground))", fontSize: "0.92rem", margin: 0 }}>
              在 API 路径中，这个 Workspace 对应 <code>workspace_id</code>。
            </p>
          </div>
        </header>

        {error ? <p className="app-notice-banner">{error}</p> : null}

        <div
          style={{
            display: "grid",
            gap: "1.25rem",
          }}
        >
          <section className="app-surface-card" style={{ display: "grid", gap: "1rem" }}>
            <div>
              <h2 style={{ margin: "0 0 0.5rem", fontSize: "1.25rem" }}>创建 API 密钥</h2>
              <p className="app-page-subtitle">按 Workspace 粒度创建密钥，控制索引能力和频率限制。</p>
            </div>
            <form onSubmit={handleCreate} style={{ display: "grid", gap: "0.9rem" }}>
              <label>
                <span className="app-form-label">密钥名称</span>
                <input
                  aria-label="密钥名称"
                  className="app-input"
                  value={nameDraft}
                  onChange={(event) => setNameDraft(event.target.value)}
                />
              </label>
              <fieldset style={{ border: 0, margin: 0, padding: 0 }}>
                <legend className="app-form-label">权限</legend>
                <div style={{ display: "grid", gap: "0.5rem", marginTop: "0.35rem" }}>
                  <label style={{ alignItems: "center", display: "flex", gap: "0.5rem" }}>
                    <input
                      checked={indexPermissionEnabled}
                      type="checkbox"
                      onChange={(event) => setIndexPermissionEnabled(event.target.checked)}
                    />
                    <span>索引（index）</span>
                  </label>
                  <label style={{ alignItems: "center", display: "flex", gap: "0.5rem" }}>
                    <input
                      checked={queryPermissionEnabled}
                      type="checkbox"
                      onChange={(event) => setQueryPermissionEnabled(event.target.checked)}
                    />
                    <span>查询（query）</span>
                  </label>
                </div>
              </fieldset>
              <p style={{ color: "hsl(var(--muted-foreground))", fontSize: "0.92rem", margin: 0 }}>
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
              <div className="app-inline-surface" style={{ display: "grid", gap: "0.75rem" }}>
                <div>
                  <strong>新密钥</strong>
                  <p style={{ color: "hsl(var(--muted-foreground))", margin: "0.35rem 0 0" }}>
                    明文只会返回这一次。
                  </p>
                </div>
                <pre style={codeBlockStyle}>{plaintextKey}</pre>
              </div>
            ) : null}
          </section>
        </div>

        <section className="app-surface-card" style={{ display: "grid", gap: "1rem" }}>
          <div style={{ alignItems: "center", display: "flex", gap: "0.75rem", justifyContent: "space-between" }}>
            <div>
              <h2 style={{ margin: "0 0 0.35rem", fontSize: "1.25rem" }}>已创建密钥</h2>
              <p className="app-page-subtitle">仅展示当前仍处于生效状态的 Workspace API 密钥。</p>
            </div>
            {loading ? <span style={{ color: "hsl(var(--muted-foreground))" }}>加载中...</span> : null}
          </div>

          {!loading && keys.length === 0 ? (
            <div className="app-inline-surface">
              <p style={{ color: "hsl(var(--muted-foreground))", margin: 0 }}>还没有 API 密钥。</p>
            </div>
          ) : null}

          <div style={{ display: "grid", gap: "0.75rem" }}>
            {keys.map((key) => (
              <div
                className="app-inline-surface"
                data-testid="api-key-item"
                key={key.id}
                style={{
                  alignItems: "start",
                  display: "flex",
                  gap: "1rem",
                  justifyContent: "space-between",
                }}
              >
                <div style={{ minWidth: 0 }}>
                  <div style={{ fontWeight: 600 }}>{key.name}</div>
                  <div style={{ color: "hsl(var(--muted-foreground))", fontSize: "0.92rem", marginTop: "0.25rem" }}>
                    {key.key_prefix} · {key.permissions.map(apiPermissionLabel).join(" / ")} · {key.rate_limit_rpm} RPM
                  </div>
                  <div style={{ color: "hsl(var(--muted-foreground))", fontSize: "0.92rem", marginTop: "0.35rem" }}>
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

        <section className="app-surface-card" style={{ display: "grid", gap: "1rem" }}>
          <div>
            <h2 style={{ margin: "0 0 0.5rem", fontSize: "1.25rem" }}>For LLM Agents</h2>
            <p className="app-page-subtitle">给要接入这个 Workspace 的 agent 看的入口卡。先理解边界，再读取稳定文档。</p>
          </div>
          <div
            className="app-inline-surface"
            style={{
              background: "linear-gradient(180deg, hsl(var(--surface)) 0%, hsl(var(--surface-muted)) 100%)",
              border: "1px solid hsl(var(--border))",
              display: "grid",
              gap: "1rem",
              padding: "1.1rem 1.15rem",
            }}
          >
            <div style={{ display: "grid", gap: "0.45rem" }}>
              <p
                style={{
                  color: "hsl(var(--muted-foreground))",
                  fontSize: "0.76rem",
                  fontWeight: 700,
                  letterSpacing: "0.08em",
                  margin: 0,
                  textTransform: "uppercase",
                }}
              >
                Agent onboarding
              </p>
              <div>
                <strong>推荐顺序</strong>
                <p style={{ color: "hsl(var(--muted-foreground))", margin: "0.35rem 0 0" }}>
                  如果你的 agent 要直连这个 Workspace，先读人类说明确认作用域，再读取稳定 agent 文档执行。
                </p>
              </div>
            </div>
            <div style={{ display: "grid", gap: "0.75rem" }}>
              <div
                style={{
                  alignItems: "start",
                  display: "grid",
                  gap: "0.65rem",
                  gridTemplateColumns: "2rem minmax(0, 1fr)",
                }}
              >
                <div
                  style={{
                    alignItems: "center",
                    background: "hsl(var(--foreground))",
                    borderRadius: "999px",
                    color: "hsl(var(--background))",
                    display: "grid",
                    fontSize: "0.88rem",
                    fontWeight: 700,
                    height: "2rem",
                    justifyItems: "center",
                    width: "2rem",
                  }}
                >
                  1
                </div>
                <div style={{ display: "grid", gap: "0.25rem", minWidth: 0 }}>
                  <strong>先读说明页</strong>
                  <p style={{ color: "hsl(var(--muted-foreground))", margin: 0 }}>
                    看清支持范围、认证方式，以及 Workspace 与 <code>workspace_id</code> 的映射。
                  </p>
                  <Link className="app-link" href="/help/api-access">
                    /help/api-access
                  </Link>
                </div>
              </div>
              <div
                style={{
                  alignItems: "start",
                  display: "grid",
                  gap: "0.65rem",
                  gridTemplateColumns: "2rem minmax(0, 1fr)",
                }}
              >
                <div
                  style={{
                    alignItems: "center",
                    background: "hsl(var(--surface-muted))",
                    border: "1px solid hsl(var(--border))",
                    borderRadius: "999px",
                    color: "hsl(var(--foreground))",
                    display: "grid",
                    fontSize: "0.88rem",
                    fontWeight: 700,
                    height: "2rem",
                    justifyItems: "center",
                    width: "2rem",
                  }}
                >
                  2
                </div>
                <div style={{ display: "grid", gap: "0.25rem", minWidth: 0 }}>
                  <strong>再读稳定 agent 文档</strong>
                  <p style={{ color: "hsl(var(--muted-foreground))", margin: 0 }}>
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
      </div>
    </main>
  );
}
