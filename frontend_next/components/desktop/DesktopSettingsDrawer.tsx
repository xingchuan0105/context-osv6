"use client";

import Link from "next/link";
import { useEffect, useState } from "react";

import styles from "./desktop.module.css";
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
  getLocalProductStatus,
  getLocalSession,
  getLocalStackStatus,
  stopLocalProduct,
  stopLocalStack,
  type ClientRuntimeConfig,
  type DockerStatus,
  type LocalProductStatus,
  type LocalSessionStatus,
  type LocalStackStatus,
} from "@/lib/desktop/tauri-local";
import { useAuth } from "@/lib/auth/context";
import { APP_PATHS, appAbsoluteUrl } from "@/lib/site-map";

type DrawerTab = "stack" | "license";

type DesktopSettingsDrawerProps = {
  open: boolean;
  onClose: () => void;
};

export function DesktopSettingsDrawer({ open, onClose }: DesktopSettingsDrawerProps) {
  const { completeAuth, user: authUser, isAuthenticated } = useAuth();
  const [tab, setTab] = useState<DrawerTab>("stack");
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

  const tabs: { id: DrawerTab; label: string }[] = [
    { id: "stack", label: "本机数据栈" },
    { id: "license", label: "关于" },
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

        <p className={styles.subtitle}>
          <Link href="/settings?tab=providers" onClick={onClose}>
            模型 Provider →
          </Link>
        </p>

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

        {tab === "stack" ? (
          <div className={styles.drawerSection}>
            <p className={styles.subtitle}>
              本机数据面（无 Docker）：PostgreSQL + pgvector（控制面与 Vector Graph RAG）+ Redis。
              默认用系统 <code>pg_ctl</code> / <code>redis-server</code>；一键「启动并迁移」写出
              client.env（RETRIEVAL_BACKEND=pgvector）并执行 migrations。Docker 仅在未安装本机
              PG/Redis 时可选回退（STACK_MODE=docker）。
            </p>

            <div className={styles.dockerStatusCard}>
              <p className={styles.flushParagraph}>
                <strong>运行时</strong> — 优先 <span className={styles.statusActive}>native</span>
                {docker ? (
                  <>
                    {" "}
                    · Docker{" "}
                    <span className={docker.overall_ok ? styles.statusActive : styles.statusError}>
                      {docker.overall_ok
                        ? "可选回退可用"
                        : !docker.cli_ok
                          ? "未安装（不需要）"
                          : !docker.daemon_ok
                            ? "引擎未运行（不需要）"
                            : "compose 不可用"}
                    </span>
                  </>
                ) : null}
              </p>
              {docker?.install_hint ? (
                <p className={styles.subtitle}>{docker.install_hint}</p>
              ) : null}
              {docker && !docker.overall_ok ? (
                <div className={`app-button-row ${styles.buttonRowWrap}`}>
                  <button
                    type="button"
                    className="app-button-secondary"
                    onClick={() => void openInBrowser(docker.install_url)}
                  >
                    本机 PG 安装说明
                  </button>
                  <button
                    type="button"
                    className="app-button-secondary"
                    disabled={loading}
                    onClick={() => void refreshStack()}
                  >
                    重新探测
                  </button>
                </div>
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
                disabled={loading || stackBusy}
                onClick={() => void handleEnsureStack()}
                title="优先使用本机 PostgreSQL + Redis（无需 Docker）"
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
                  RETRIEVAL_BACKEND={runtimeConfig.retrieval_backend ?? "pgvector"}
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
              <strong>客户端免费</strong>
            </p>
            <p className={styles.subtitle}>
              无需激活码。本机使用自备模型 Key；云端分享名额与钱包见定价页。
              {licenseLabel ? `（本机状态：${licenseLabel}${licenseDetail ? ` · ${licenseDetail}` : ""}）` : ""}
            </p>
            <div className="app-button-row">
              <button
                type="button"
                className="app-button-secondary"
                onClick={() => void openInBrowser(appAbsoluteUrl(APP_PATHS.desktop))}
              >
                下载 / 客户端页
              </button>
              <button
                type="button"
                className="app-button-secondary"
                onClick={() => void openInBrowser(appAbsoluteUrl(APP_PATHS.pricing))}
              >
                云端定价
              </button>
            </div>
          </div>
        ) : null}
      </aside>
    </div>
  );
}
