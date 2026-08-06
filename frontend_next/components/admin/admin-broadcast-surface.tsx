"use client";

import { useState } from "react";

import { useAuth } from "../../lib/auth/context";
import { request } from "../../lib/http/request";
import { useUiPreferences } from "../../lib/ui-preferences";
import styles from "./admin-shared-ui.module.css";

/**
 * Minimal admin broadcast form (W4 #11).
 * POST /api/v1/admin/notifications/broadcast
 */
export function AdminBroadcastSurface() {
  const { token } = useAuth();
  const { locale } = useUiPreferences();
  const [title, setTitle] = useState("");
  const [body, setBody] = useState("");
  const [busy, setBusy] = useState(false);
  const [message, setMessage] = useState("");
  const [error, setError] = useState("");

  async function handleSubmit(event: React.FormEvent) {
    event.preventDefault();
    if (!token) {
      setError(locale === "zh-CN" ? "请先登录" : "Sign in required");
      return;
    }
    setBusy(true);
    setError("");
    setMessage("");
    try {
      const result = await request<{ created: number }>(
        "/api/v1/admin/notifications/broadcast",
        {
          method: "POST",
          body: JSON.stringify({
            event_type: "admin.broadcast",
            title: title.trim(),
            body: body.trim(),
            data: {},
          }),
        },
        token,
      );
      setMessage(
        locale === "zh-CN"
          ? `已发送 ${result.created} 条通知`
          : `Sent ${result.created} notifications`,
      );
      setTitle("");
      setBody("");
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    } finally {
      setBusy(false);
    }
  }

  return (
    <section className={styles.headingBlock} data-testid="admin-broadcast-surface">
      <div className={styles.headingBlock}>
        <h1 className={styles.headingTitle}>
          {locale === "zh-CN" ? "官方广播" : "Broadcast"}
        </h1>
        <p className={styles.headingSubtitle}>
          {locale === "zh-CN"
            ? "向全部用户推送一条应用内通知（账户级，非 Workspace）。"
            : "Push one in-app notification to all users (account-level, not workspace)."}
        </p>
      </div>
      <form
        className="app-inline-surface"
        style={{ display: "grid", gap: "0.75rem", padding: "1rem", maxWidth: "36rem" }}
        onSubmit={(e) => void handleSubmit(e)}
      >
        <label>
          <span className="app-form-label">{locale === "zh-CN" ? "标题" : "Title"}</span>
          <input
            className="app-input"
            data-testid="admin-broadcast-title"
            maxLength={120}
            required
            value={title}
            onChange={(e) => setTitle(e.target.value)}
          />
        </label>
        <label>
          <span className="app-form-label">{locale === "zh-CN" ? "正文" : "Body"}</span>
          <textarea
            className="app-input"
            data-testid="admin-broadcast-body"
            maxLength={2000}
            required
            rows={5}
            value={body}
            onChange={(e) => setBody(e.target.value)}
          />
        </label>
        {error ? <p className="app-notice-banner">{error}</p> : null}
        {message ? (
          <p className="app-notice-banner" data-testid="admin-broadcast-result">
            {message}
          </p>
        ) : null}
        <button
          className="app-button-primary"
          data-testid="admin-broadcast-submit"
          disabled={busy || !title.trim() || !body.trim()}
          type="submit"
        >
          {busy
            ? locale === "zh-CN"
              ? "发送中…"
              : "Sending…"
            : locale === "zh-CN"
              ? "广播发送"
              : "Send broadcast"}
        </button>
      </form>
    </section>
  );
}
