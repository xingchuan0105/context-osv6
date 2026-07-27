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
  ensureLocalProduct,
  ensureLocalSession,
  ensureLocalStack,
  getClientRuntimeConfig,
  getDockerStatus,
  getLlmConfig,
  getLocalProductStatus,
  getLocalSession,
  getLocalStackStatus,
  setLlmConfig,
  stopLocalProduct,
  stopLocalStack,
  testLlmConnection,
  type ClientRuntimeConfig,
  type DockerStatus,
  type LocalLlmConfig,
  type LocalProductStatus,
  type LocalSessionStatus,
  type LocalStackStatus,
} from "@/lib/desktop/tauri-llm";
import { useAuth } from "@/lib/auth/context";
import { APP_PATHS, appAbsoluteUrl } from "@/lib/site-map";

type DrawerTab = "llm" | "embedding" | "stack" | "license" | "diagnostic";

type DesktopSettingsDrawerProps = {
  open: boolean;
  onClose: () => void;
};

export function DesktopSettingsDrawer({ open, onClose }: DesktopSettingsDrawerProps) {
  const { completeAuth, user: authUser, isAuthenticated } = useAuth();
  const [tab, setTab] = useState<DrawerTab>("llm");
  const [config, setConfig] = useState<LocalLlmConfig | null>(null);
  const [provider, setProvider] = useState("zhipu");
  const [apiKey, setApiKey] = useState("");
  const [baseUrl, setBaseUrl] = useState("");
  const [model, setModel] = useState("");
  const [embBaseUrl, setEmbBaseUrl] = useState("");
  const [embApiKey, setEmbApiKey] = useState("");
  const [embModel, setEmbModel] = useState("");
  const [embDims, setEmbDims] = useState("");
  const [licenseLabel, setLicenseLabel] = useState("");
  const [licenseDetail, setLicenseDetail] = useState("");
  const [stack, setStack] = useState<LocalStackStatus | null>(null);
  const [product, setProduct] = useState<LocalProductStatus | null>(null);
  const [localSession, setLocalSession] = useState<LocalSessionStatus | null>(null);
  const [docker, setDocker] = useState<DockerStatus | null>(null);
  const [runtimeConfig, setRuntimeConfig] = useState<ClientRuntimeConfig | null>(null);
  const [stackBusy, setStackBusy] = useState(false);
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
        if (saved.embedding) {
          setEmbBaseUrl(saved.embedding.base_url);
          setEmbApiKey(saved.embedding.api_key);
          setEmbModel(saved.embedding.model);
          setEmbDims(saved.embedding.dimensions != null ? String(saved.embedding.dimensions) : "");
        }
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

    void getLocalStackStatus()
      .then(setStack)
      .catch(() => setStack(null));

    void getClientRuntimeConfig()
      .then(setRuntimeConfig)
      .catch(() => setRuntimeConfig(null));

    void getLocalProductStatus()
      .then(setProduct)
      .catch(() => setProduct(null));

    void getLocalSession()
      .then(setLocalSession)
      .catch(() => setLocalSession(null));

    void getDockerStatus()
      .then(setDocker)
      .catch(() => setDocker(null));
  }, [open]);

  async function refreshStack() {
    setLoading(true);
    setError("");
    try {
      const [nextStack, nextConfig, nextProduct, nextDocker] = await Promise.all([
        getLocalStackStatus(),
        getClientRuntimeConfig(),
        getLocalProductStatus(),
        getDockerStatus(),
      ]);
      setStack(nextStack);
      setRuntimeConfig(nextConfig);
      setProduct(nextProduct);
      setDocker(nextDocker);
      if (nextStack.docker) {
        setDocker(nextStack.docker);
      }
      const parts = [
        nextDocker.overall_ok ? "Docker 就绪" : "Docker 未就绪",
        nextStack.overall_ok ? "数据栈就绪" : "数据栈未全就绪",
        nextProduct.api_ok ? "API 就绪" : "API 未就绪",
      ];
      setMessage(parts.join(" · "));
    } catch (probeError) {
      setError(probeError instanceof Error ? probeError.message : "探测失败");
    } finally {
      setLoading(false);
    }
  }

  async function handleEnsureStack() {
    setStackBusy(true);
    setError("");
    setMessage("");
    try {
      const result = await ensureLocalStack();
      setStack(result.status);
      setRuntimeConfig(result.config);
      if (result.ok) {
        setMessage(result.message);
      } else {
        setError(result.message || result.stderr || "启动失败");
      }
    } catch (ensureError) {
      setError(ensureError instanceof Error ? ensureError.message : "启动本机栈失败");
    } finally {
      setStackBusy(false);
    }
  }

  async function handleStopStack() {
    setStackBusy(true);
    setError("");
    setMessage("");
    try {
      const result = await stopLocalStack();
      setStack(result.status);
      setRuntimeConfig(result.config);
      if (result.ok) {
        setMessage(result.message);
      } else {
        setError(result.message || result.stderr || "停止失败");
      }
    } catch (stopError) {
      setError(stopError instanceof Error ? stopError.message : "停止本机栈失败");
    } finally {
      setStackBusy(false);
    }
  }

  async function handleEnsureProduct() {
    setStackBusy(true);
    setError("");
    setMessage("");
    try {
      const result = await ensureLocalProduct();
      setProduct(result.status);
      if (result.ok) {
        setMessage(result.message);
      } else {
        setError(result.message || result.stderr || "启动本机产品进程失败");
      }
      try {
        setStack(await getLocalStackStatus());
        setRuntimeConfig(await getClientRuntimeConfig());
      } catch {
        /* ignore */
      }
    } catch (productError) {
      setError(productError instanceof Error ? productError.message : "启动本机产品进程失败");
    } finally {
      setStackBusy(false);
    }
  }

  async function handleStopProduct() {
    setStackBusy(true);
    setError("");
    setMessage("");
    try {
      const result = await stopLocalProduct();
      setProduct(result.status);
      if (result.ok) {
        setMessage(result.message);
      } else {
        setError(result.message || result.stderr || "停止产品进程失败");
      }
    } catch (stopError) {
      setError(stopError instanceof Error ? stopError.message : "停止产品进程失败");
    } finally {
      setStackBusy(false);
    }
  }

  async function handleEnsureSession() {
    setStackBusy(true);
    setError("");
    setMessage("");
    try {
      const session = await ensureLocalSession();
      setLocalSession(session);
      if (session.ready && session.token && session.user) {
        completeAuth({
          token: session.token,
          user: {
            id: session.user.id,
            email: session.user.email,
            full_name: session.user.full_name,
          },
          reset_ticket: null,
        });
        setMessage(session.message);
      } else {
        setError(session.message || "本机会话未就绪");
      }
    } catch (sessionError) {
      setError(sessionError instanceof Error ? sessionError.message : "本机会话失败");
    } finally {
      setStackBusy(false);
    }
  }

  if (!open) {
    return null;
  }

  function buildDraftConfig(): LocalLlmConfig {
    const embedding =
      embBaseUrl.trim() || embModel.trim()
        ? {
            base_url: embBaseUrl.trim() || baseUrl,
            api_key: embApiKey.trim() || apiKey,
            model: embModel.trim() || "text-embedding-3-small",
            dimensions: embDims.trim() ? Number(embDims) : null,
          }
        : config?.embedding ?? null;

    return {
      provider,
      base_url: baseUrl,
      api_key: apiKey,
      model,
      timeout_ms: config?.timeout_ms ?? 30_000,
      embedding,
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
      setMessage("配置已保存");
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

  const tabs: { id: DrawerTab; label: string }[] = [
    { id: "llm", label: "LLM" },
    { id: "embedding", label: "Embedding" },
    { id: "stack", label: "本机数据栈" },
    { id: "license", label: "授权" },
    { id: "diagnostic", label: "诊断" },
  ];

  return (
    <div className={styles.drawerOverlay} role="presentation" onClick={onClose}>
      <aside
        className={styles.drawerPanel}
        role="dialog"
        aria-label="客户端设置"
        onClick={(event) => event.stopPropagation()}
      >
        <header className={styles.drawerHeader}>
          <h2 className={styles.drawerTitle}>客户端设置</h2>
          <button type="button" className="app-button-ghost" onClick={onClose}>
            关闭
          </button>
        </header>

        <div className={styles.drawerTabs}>
          {tabs.map((item) => (
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
              <button type="button" className="app-button-secondary" disabled={loading} onClick={() => void handleTestLlm()}>
                测试连接
              </button>
              <button type="button" className="app-button-primary" disabled={loading} onClick={() => void handleSaveLlm()}>
                保存
              </button>
            </div>
            <p className={styles.subtitle}>
              <Link href="/setup" onClick={onClose}>
                打开完整引导（/setup）
              </Link>
            </p>
          </div>
        ) : null}

        {tab === "embedding" ? (
          <div className={styles.drawerSection}>
            <p className={styles.subtitle}>用于本机文档向量化；可与 LLM 共用 Key，或单独填写。</p>
            <label className="app-form-label" htmlFor="emb-base">
              Embedding Base URL
            </label>
            <input
              id="emb-base"
              className="app-input"
              value={embBaseUrl}
              placeholder={baseUrl || "https://…"}
              onChange={(event) => setEmbBaseUrl(event.target.value)}
            />
            <label className="app-form-label" htmlFor="emb-key">
              Embedding API Key
            </label>
            <input
              id="emb-key"
              className="app-input"
              type="password"
              value={embApiKey}
              onChange={(event) => setEmbApiKey(event.target.value)}
            />
            <label className="app-form-label" htmlFor="emb-model">
              Embedding Model
            </label>
            <input
              id="emb-model"
              className="app-input"
              value={embModel}
              placeholder="text-embedding-3-small / embedding-2"
              onChange={(event) => setEmbModel(event.target.value)}
            />
            <label className="app-form-label" htmlFor="emb-dims">
              Dimensions（可选）
            </label>
            <input
              id="emb-dims"
              className="app-input"
              value={embDims}
              placeholder="1024"
              onChange={(event) => setEmbDims(event.target.value)}
            />
            <button type="button" className="app-button-primary" disabled={loading} onClick={() => void handleSaveLlm()}>
              保存 Embedding 配置
            </button>
          </div>
        ) : null}

        {tab === "stack" ? (
          <div className={styles.drawerSection}>
            <p className={styles.subtitle}>
              本机数据面：PostgreSQL、Redis、完整 Milvus。依赖 Docker（Windows 请使用 Docker
              Desktop）。一键「启动并迁移」会拉起 compose 栈、写出 client.env，并执行 migrations。
            </p>

            <div className={styles.dockerStatusCard}>
              <p className={styles.flushParagraph}>
                <strong>Docker</strong> —{" "}
                <span className={docker?.overall_ok ? styles.statusActive : styles.statusError}>
                  {docker
                    ? docker.overall_ok
                      ? "就绪"
                      : !docker.cli_ok
                        ? "未安装"
                        : !docker.daemon_ok
                          ? "引擎未运行"
                          : "compose 不可用"
                    : "未探测"}
                </span>
              </p>
              {docker ? <p className={styles.subtitle}>{docker.detail}</p> : null}
              {docker && !docker.overall_ok ? (
                <>
                  <p className={styles.subtitle}>
                    {docker.install_hint}
                  </p>
                  <div className={`app-button-row ${styles.buttonRowWrap}`}>
                    <button
                      type="button"
                      className="app-button-primary"
                      onClick={() => void openInBrowser(docker.install_url)}
                    >
                      打开 Docker 安装指南
                    </button>
                    <button
                      type="button"
                      className="app-button-secondary"
                      disabled={loading}
                      onClick={() => void refreshStack()}
                    >
                      重新检测 Docker
                    </button>
                  </div>
                </>
              ) : null}
            </div>

            {stack ? (
              <ul className={styles.serviceList}>
                {stack.services.map((s) => (
                  <li key={s.id}>
                    <strong>{s.label}</strong> {s.endpoint} —{" "}
                    <span className={s.ok ? styles.statusActive : styles.statusError}>
                      {s.ok ? "OK" : "DOWN"}
                    </span>
                    <div className={styles.subtitle}>{s.detail}</div>
                  </li>
                ))}
              </ul>
            ) : (
              <p className={styles.subtitle}>尚未探测（仅桌面运行时可用）</p>
            )}
            <div className={`app-button-row ${styles.stackActions}`}>
              <button
                type="button"
                className="app-button-primary"
                disabled={loading || stackBusy || (docker != null && !docker.overall_ok)}
                onClick={() => void handleEnsureStack()}
                title={
                  docker && !docker.overall_ok
                    ? "请先安装并启动 Docker Desktop"
                    : undefined
                }
              >
                {stackBusy ? "处理中…" : "启动并迁移"}
              </button>
              <button
                type="button"
                className="app-button-secondary"
                disabled={loading || stackBusy}
                onClick={() => void refreshStack()}
              >
                重新探测
              </button>
              <button
                type="button"
                className="app-button-secondary"
                disabled={loading || stackBusy}
                onClick={() => void handleStopStack()}
              >
                停止栈
              </button>
            </div>
            {runtimeConfig ? (
              <div className={styles.runtimeConfig}>
                <p className={styles.subtitle}>
                  <strong>运行时连接</strong>
                  {runtimeConfig.env_file_exists ? " · client.env 已生成" : " · client.env 未生成"}
                </p>
                <code className={`${styles.subtitle} ${styles.codeBlock}`}>
                  DATABASE_URL={runtimeConfig.database_url}
                </code>
                <code className={`${styles.subtitle} ${styles.codeBlock}`}>
                  REDIS_URL={runtimeConfig.redis_url}
                </code>
                <code className={`${styles.subtitle} ${styles.codeBlock}`}>
                  MILVUS_URL={runtimeConfig.milvus_url}
                </code>
                {runtimeConfig.env_file_path ? (
                  <p className={styles.subtitle}>
                    env 文件：{runtimeConfig.env_file_path}
                  </p>
                ) : null}
                <p className={styles.subtitle}>
                  {runtimeConfig.note}
                </p>
              </div>
            ) : null}

            <div className={styles.productSection}>
              <p className={styles.subtitle}>
                <strong>本机产品进程</strong>（avrag-api + worker，默认 :18080）。先起数据栈，再启动产品；REST 经桌面{" "}
                <code>api_call</code> 代理。
              </p>
              {product ? (
                <ul className={styles.productList}>
                  <li>
                    <strong>API</strong> {product.api_base_url} —{" "}
                    <span className={product.api_ok ? styles.statusActive : styles.statusError}>
                      {product.api_ok ? "OK" : "DOWN"}
                    </span>
                    <div className={styles.subtitle}>{product.health_detail}</div>
                  </li>
                  <li>
                    <strong>Worker</strong> —{" "}
                    <span className={product.worker_ok ? styles.statusActive : styles.statusError}>
                      {product.worker_ok ? "OK" : "DOWN"}
                    </span>
                    <div className={styles.subtitle}>{product.worker_detail}</div>
                  </li>
                </ul>
              ) : (
                <p className={styles.subtitle}>尚未探测产品进程</p>
              )}
              <div className={`app-button-row ${styles.stackActions}`}>
                <button
                  type="button"
                  className="app-button-primary"
                  disabled={loading || stackBusy}
                  onClick={() => void handleEnsureProduct()}
                >
                  {stackBusy ? "处理中…" : "启动产品进程"}
                </button>
                <button
                  type="button"
                  className="app-button-secondary"
                  disabled={loading || stackBusy}
                  onClick={() => void handleStopProduct()}
                >
                  停止产品进程
                </button>
              </div>
              {product?.log_dir ? (
                <p className={`${styles.subtitle} ${styles.logDirNote}`}>
                  日志：{product.log_dir}
                </p>
              ) : null}

              <div className={styles.sessionSection}>
                <p className={styles.subtitle}>
                  <strong>本机个人账户</strong>（B2C · 无云登录）
                  {isAuthenticated && authUser ? ` · ${authUser.email}` : ""}
                </p>
                {localSession ? (
                  <p className={`${styles.subtitle} ${styles.sessionStatus}`}>
                    {localSession.ready ? "会话就绪" : "会话未就绪"} — {localSession.message}
                  </p>
                ) : null}
                <div className={`app-button-row ${styles.sessionActions}`}>
                  <button
                    type="button"
                    className="app-button-secondary"
                    disabled={loading || stackBusy}
                    onClick={() => void handleEnsureSession()}
                  >
                    刷新本机会话
                  </button>
                </div>
              </div>
            </div>

            <p className={`${styles.subtitle} ${styles.cliHint}`}>
              CLI 栈：<code>{stack?.compose_hint ?? "bash scripts/desktop-local-stack.sh ensure"}</code>
              <br />
              CLI 产品：
              <code>{product?.compose_hint ?? "bash scripts/desktop-local-product.sh ensure"}</code>
            </p>
          </div>
        ) : null}

        {tab === "license" ? (
          <div className={styles.drawerSection}>
            <p>
              <strong>{licenseLabel}</strong>
            </p>
            {licenseDetail ? <p className={styles.subtitle}>{licenseDetail}</p> : null}
            <div className="app-button-row">
              <Link href="/activate" className="app-button-secondary" onClick={onClose}>
                欢迎 / 激活页
              </Link>
              <button
                type="button"
                className="app-button-secondary"
                onClick={() => void openInBrowser(appAbsoluteUrl(APP_PATHS.desktopBuy))}
              >
                购买授权
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
