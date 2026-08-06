"use client";

import { useState } from "react";

import { useAuth } from "../../lib/auth/context";
import { request } from "../../lib/http/request";
import { formatUiMessage } from "../../lib/i18n/messages";
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
      setError(formatUiMessage(locale, "admin.broadcast.signInRequired"));
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
        formatUiMessage(locale, "admin.broadcast.success", {
          count: result.created,
        }),
      );
      setTitle("");
      setBody("");
    } catch (e) {
      setError(
        e instanceof Error
          ? e.message
          : formatUiMessage(locale, "admin.broadcast.error"),
      );
    } finally {
      setBusy(false);
    }
  }

  return (
    <section className={styles.headingBlock} data-testid="admin-broadcast-surface">
      <div className={styles.headingBlock}>
        <h1 className={styles.headingTitle}>
          {formatUiMessage(locale, "admin.broadcast.title")}
        </h1>
        <p className={styles.headingSubtitle}>
          {formatUiMessage(locale, "admin.broadcast.subtitle")}
        </p>
      </div>
      <form
        className="app-inline-surface"
        style={{ display: "grid", gap: "0.75rem", padding: "1rem", maxWidth: "36rem" }}
        onSubmit={(e) => void handleSubmit(e)}
      >
        <label>
          <span className="app-form-label">
            {formatUiMessage(locale, "admin.broadcast.titleLabel")}
          </span>
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
          <span className="app-form-label">
            {formatUiMessage(locale, "admin.broadcast.bodyLabel")}
          </span>
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
            ? formatUiMessage(locale, "admin.broadcast.sending")
            : formatUiMessage(locale, "admin.broadcast.submit")}
        </button>
      </form>
    </section>
  );
}
