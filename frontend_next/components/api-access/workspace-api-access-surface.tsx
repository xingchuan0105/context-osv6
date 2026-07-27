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

import styles from "./workspace-api-access-surface.module.css";

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
          为这个 Workspace 创建 API 密钥。完整说明与 agent 文档可在完整页面查看。
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

      {embedded ? null : (
        <section className={`app-surface-card ${styles.card}`}>
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
      )}
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
