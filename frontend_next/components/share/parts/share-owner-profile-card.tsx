"use client";

import Link from "next/link";
import { useState } from "react";
import { useMutation } from "@tanstack/react-query";

import { useAuth } from "../../../lib/auth/context";
import { formatUiMessage } from "../../../lib/i18n/messages";
import { updateProfile } from "../../../lib/settings/client";
import { useUiPreferences } from "../../../lib/ui-preferences";
import styles from "./share-control-bar.module.css";

/**
 * Owner-level public profile toggle (global, not per-workspace):
 * gates the public sharer page (/shared/u/[userId]) and the share-page
 * "more shares" entry points. Persisted via PUT /api/auth/profile —
 * the backend treats that endpoint as a full-object write, so the
 * current profile fields are always sent along with the flag.
 */
export function ShareOwnerProfileCard() {
  const auth = useAuth();
  const { locale } = useUiPreferences();
  const [actionError, setActionError] = useState("");
  const enabled = auth.user?.public_profile_enabled === true;

  const mutation = useMutation({
    mutationFn: async (next: boolean) => {
      if (!auth.token || !auth.user) {
        throw new Error(formatUiMessage(locale, "settings.profile.notAuthenticated"));
      }
      const response = await updateProfile(auth.token, {
        full_name: auth.user.full_name,
        bio: auth.user.bio ?? null,
        contact_url: auth.user.contact_url ?? null,
        public_profile_enabled: next,
      });
      if (!response.success || !response.data) {
        throw new Error(response.error ?? formatUiMessage(locale, "settings.saveError"));
      }
      return response.data.user;
    },
    onSuccess: (user) => {
      auth.updateUser(user);
      setActionError("");
    },
    onError: (error) => {
      setActionError(
        error instanceof Error
          ? error.message
          : formatUiMessage(locale, "settings.saveError"),
      );
    },
  });

  if (!auth.user) {
    return null;
  }

  return (
    <section className="app-surface-card" data-testid="owner-profile-card">
      <div className={styles.switchRow}>
        <div className={styles.switchLabelStack}>
          <span className={styles.switchLabel}>
            {formatUiMessage(locale, "shareCenter.ownerProfileTitle")}
          </span>
          <strong className={styles.switchState}>
            {enabled
              ? formatUiMessage(locale, "shareCenter.statusActive")
              : formatUiMessage(locale, "shareCenter.statusInactive")}
          </strong>
        </div>
        <button
          aria-checked={enabled}
          className={`app-button-ghost ${styles.switchTrack}`}
          data-testid="owner-profile-switch"
          disabled={mutation.isPending}
          role="switch"
          style={{
            background: enabled ? "hsl(var(--accent))" : "hsl(var(--muted))",
            justifyContent: enabled ? "flex-end" : "flex-start",
          }}
          type="button"
          onClick={() => mutation.mutate(!enabled)}
        >
          <span
            aria-hidden="true"
            className={styles.switchKnob}
            style={{
              background: enabled
                ? "hsl(var(--background))"
                : "hsl(var(--muted-foreground))",
            }}
          />
        </button>
      </div>
      <p className={styles.footnote}>
        {formatUiMessage(locale, "shareCenter.ownerProfileHint")}
      </p>
      {enabled ? (
        <Link
          className="app-link"
          data-testid="owner-profile-public-link"
          href={`/shared/u/${auth.user.id}`}
          target="_blank"
        >
          {formatUiMessage(locale, "shareCenter.ownerProfileView")}
        </Link>
      ) : null}
      {actionError ? <p className="app-notice-banner">{actionError}</p> : null}
    </section>
  );
}
