"use client";

import { useEffect, useRef, useState } from "react";
import { useMutation } from "@tanstack/react-query";
import { useForm } from "react-hook-form";
import { z } from "zod";

import { describeAuthError } from "../../lib/auth/errors";
import { useAuth } from "../../lib/auth/context";
import { buildApiUrl } from "../../lib/http/request";
import { formatUiMessage } from "../../lib/i18n/messages";
import {
  deleteProfileMedia,
  updateProfile,
  uploadProfileMedia,
} from "../../lib/settings/client";
import { useUiPreferences } from "../../lib/ui-preferences";
import { applyZodErrors, bannerStyle, type ProfileFormValues } from "./settings-shared";
import styles from "./settings-profile-panel.module.css";
import shared from "./settings-ui-shared.module.css";

function mediaSrc(path: string | null | undefined) {
  if (!path?.trim()) {
    return null;
  }
  if (path.startsWith("http://") || path.startsWith("https://")) {
    return path;
  }
  return buildApiUrl(path);
}

export function ProfilePanel() {
  const auth = useAuth();
  const { locale } = useUiPreferences();
  const avatarInputRef = useRef<HTMLInputElement | null>(null);
  const bannerInputRef = useRef<HTMLInputElement | null>(null);
  const profileForm = useForm<ProfileFormValues>({
    defaultValues: {
      fullName: auth.user?.full_name ?? "",
      bio: auth.user?.bio ?? "",
      contactUrl: auth.user?.contact_url ?? "",
    },
  });
  const [banner, setBanner] = useState("");
  const [actionError, setActionError] = useState("");
  const profileMutation = useMutation({
    mutationFn: async (values: ProfileFormValues) => {
      if (!auth.token) {
        throw new Error(formatUiMessage(locale, "settings.profile.notAuthenticated"));
      }

      const response = await updateProfile(auth.token, {
        full_name: values.fullName || null,
        bio: values.bio || null,
        contact_url: values.contactUrl || null,
      });

      if (!response.success || !response.data) {
        throw new Error(
          response.error ?? formatUiMessage(locale, "settings.saveError"),
        );
      }

      return response.data.user;
    },
    onSuccess: (user) => {
      auth.updateUser(user);
      setBanner(formatUiMessage(locale, "settings.saveSuccess"));
    },
  });
  const mediaMutation = useMutation({
    mutationFn: async (input: {
      kind: "avatar" | "banner";
      action: "upload" | "remove";
      file?: File;
    }) => {
      if (!auth.token) {
        throw new Error(formatUiMessage(locale, "settings.profile.notAuthenticated"));
      }
      if (input.action === "remove") {
        const response = await deleteProfileMedia(auth.token, input.kind);
        if (!response.success || !response.data) {
          throw new Error(response.error ?? formatUiMessage(locale, "settings.saveError"));
        }
        return response.data.user;
      }
      if (!input.file) {
        throw new Error(formatUiMessage(locale, "settings.saveError"));
      }
      const response = await uploadProfileMedia(
        auth.token,
        input.kind,
        input.file,
        input.file.type || "image/jpeg",
      );
      if (!response.success || !response.data) {
        throw new Error(response.error ?? formatUiMessage(locale, "settings.saveError"));
      }
      return response.data.user;
    },
    onSuccess: (user) => {
      auth.updateUser(user);
      setBanner(formatUiMessage(locale, "settings.saveSuccess"));
    },
  });

  useEffect(() => {
    profileForm.reset({
      fullName: auth.user?.full_name ?? "",
      bio: auth.user?.bio ?? "",
      contactUrl: auth.user?.contact_url ?? "",
    });
  }, [auth.user?.full_name, auth.user?.bio, auth.user?.contact_url, profileForm]);

  const profileSchema = z.object({
    fullName: z.string().trim().max(120, {
      message: formatUiMessage(locale, "settings.profile.nameTooLong"),
    }),
    bio: z.string().trim().max(500, {
      message: formatUiMessage(locale, "settings.profile.bioTooLong"),
    }),
    contactUrl: z
      .string()
      .trim()
      .max(500)
      .refine(
        (value) =>
          !value ||
          value.toLowerCase().startsWith("https://") ||
          value.toLowerCase().startsWith("http://"),
        { message: formatUiMessage(locale, "settings.profile.invalidContactUrl") },
      ),
  });

  async function handleSubmit(values: ProfileFormValues) {
    setBanner("");
    setActionError("");
    profileForm.clearErrors();

    const parsed = profileSchema.safeParse(values);

    if (!parsed.success) {
      applyZodErrors(parsed.error, profileForm.setError);
      return;
    }

    try {
      await profileMutation.mutateAsync(parsed.data);
    } catch (error) {
      setActionError(
        describeAuthError(
          formatUiMessage(locale, "settings.saveError"),
          error,
        ),
      );
    }
  }

  async function handleMediaChange(kind: "avatar" | "banner", file: File | null) {
    setBanner("");
    setActionError("");
    if (!file) {
      return;
    }
    try {
      await mediaMutation.mutateAsync({ kind, action: "upload", file });
    } catch (error) {
      setActionError(
        describeAuthError(formatUiMessage(locale, "settings.saveError"), error),
      );
    }
  }

  async function handleMediaRemove(kind: "avatar" | "banner") {
    setBanner("");
    setActionError("");
    try {
      await mediaMutation.mutateAsync({ kind, action: "remove" });
    } catch (error) {
      setActionError(
        describeAuthError(formatUiMessage(locale, "settings.saveError"), error),
      );
    }
  }

  const avatarUrl = mediaSrc(auth.user?.avatar_url);
  const bannerUrl = mediaSrc(auth.user?.banner_url);
  const busy = profileMutation.isPending || mediaMutation.isPending;

  return (
    <section className={shared.section}>
      <section className={`app-inline-surface ${shared.section}`}>
        <div className={shared.headerText}>
          <h2 className={shared.flushTitle}>
            {formatUiMessage(locale, "settings.profile.sectionTitle")}
          </h2>
          <p className={shared.mutedText}>
            {formatUiMessage(locale, "settings.profile.sectionSubtitle")}
          </p>
        </div>

        <div className={styles.mediaPreview} data-testid="settings-profile-card-preview">
          <div
            className={styles.bannerPreview}
            style={bannerUrl ? { backgroundImage: `url(${bannerUrl})` } : undefined}
          />
          <div className={styles.avatarRow}>
            <div
              className={styles.avatarPreview}
              style={avatarUrl ? { backgroundImage: `url(${avatarUrl})` } : undefined}
              aria-hidden={!avatarUrl}
            >
              {!avatarUrl
                ? (auth.user?.full_name?.trim() || auth.user?.email || "U").slice(0, 1).toUpperCase()
                : null}
            </div>
            <div className={styles.mediaActions}>
              <p className={styles.mediaHint}>
                {formatUiMessage(locale, "settings.profile.mediaHint")}
              </p>
              <div className={styles.mediaButtonRow}>
                <button
                  className="app-button-secondary"
                  disabled={busy}
                  type="button"
                  onClick={() => avatarInputRef.current?.click()}
                >
                  {formatUiMessage(locale, "settings.profile.avatarLabel")} ·{" "}
                  {formatUiMessage(locale, "settings.profile.uploadAction")}
                </button>
                {avatarUrl ? (
                  <button
                    className="app-button-ghost"
                    disabled={busy}
                    type="button"
                    onClick={() => void handleMediaRemove("avatar")}
                  >
                    {formatUiMessage(locale, "settings.profile.removeMediaAction")}
                  </button>
                ) : null}
              </div>
              <div className={styles.mediaButtonRow}>
                <button
                  className="app-button-secondary"
                  disabled={busy}
                  type="button"
                  onClick={() => bannerInputRef.current?.click()}
                >
                  {formatUiMessage(locale, "settings.profile.bannerLabel")} ·{" "}
                  {formatUiMessage(locale, "settings.profile.uploadAction")}
                </button>
                {bannerUrl ? (
                  <button
                    className="app-button-ghost"
                    disabled={busy}
                    type="button"
                    onClick={() => void handleMediaRemove("banner")}
                  >
                    {formatUiMessage(locale, "settings.profile.removeMediaAction")}
                  </button>
                ) : null}
              </div>
              <input
                accept="image/jpeg,image/png,image/webp,image/gif"
                className={styles.hiddenFileInput}
                ref={avatarInputRef}
                type="file"
                onChange={(event) => {
                  const file = event.target.files?.[0] ?? null;
                  event.target.value = "";
                  void handleMediaChange("avatar", file);
                }}
              />
              <input
                accept="image/jpeg,image/png,image/webp,image/gif"
                className={styles.hiddenFileInput}
                ref={bannerInputRef}
                type="file"
                onChange={(event) => {
                  const file = event.target.files?.[0] ?? null;
                  event.target.value = "";
                  void handleMediaChange("banner", file);
                }}
              />
            </div>
          </div>
        </div>

        <form
          className={shared.formGrid}
          noValidate
          onSubmit={profileForm.handleSubmit(handleSubmit)}
        >
          <div>
            <label className="app-form-label" htmlFor="settings-profile-email">
              {formatUiMessage(locale, "settings.profile.emailLabel")}
            </label>
            <input
              className={`app-input ${styles.readonlyInput}`}
              id="settings-profile-email"
              readOnly
              type="email"
              value={auth.user?.email ?? ""}
            />
          </div>
          <div>
            <label className="app-form-label" htmlFor="settings-profile-name">
              {formatUiMessage(locale, "settings.profile.nameLabel")}
            </label>
            <input
              className="app-input"
              id="settings-profile-name"
              placeholder={formatUiMessage(locale, "settings.profile.namePlaceholder")}
              type="text"
              {...profileForm.register("fullName")}
            />
            {profileForm.formState.errors.fullName?.message ? (
              <p className={`app-form-footnote ${styles.errorText}`}>
                {profileForm.formState.errors.fullName.message}
              </p>
            ) : null}
          </div>
          <div>
            <label className="app-form-label" htmlFor="settings-profile-bio">
              {formatUiMessage(locale, "settings.profile.bioLabel")}
            </label>
            <textarea
              className={`app-input ${styles.bioInput}`}
              id="settings-profile-bio"
              placeholder={formatUiMessage(locale, "settings.profile.bioPlaceholder")}
              rows={3}
              {...profileForm.register("bio")}
            />
            {profileForm.formState.errors.bio?.message ? (
              <p className={`app-form-footnote ${styles.errorText}`}>
                {profileForm.formState.errors.bio.message}
              </p>
            ) : null}
          </div>
          <div>
            <label className="app-form-label" htmlFor="settings-profile-contact">
              {formatUiMessage(locale, "settings.profile.contactLabel")}
            </label>
            <input
              className="app-input"
              id="settings-profile-contact"
              placeholder={formatUiMessage(locale, "settings.profile.contactPlaceholder")}
              type="url"
              {...profileForm.register("contactUrl")}
            />
            {profileForm.formState.errors.contactUrl?.message ? (
              <p className={`app-form-footnote ${styles.errorText}`}>
                {profileForm.formState.errors.contactUrl.message}
              </p>
            ) : null}
          </div>
          {banner ? (
            <p className="app-notice-banner" style={bannerStyle("success")}>
              {banner}
            </p>
          ) : null}
          {actionError ? <p className="app-notice-banner">{actionError}</p> : null}
          <div className="app-button-row">
            <button
              className="app-button-primary"
              disabled={busy}
              type="submit"
            >
              {profileMutation.isPending
                ? formatUiMessage(locale, "shareCenter.saving")
                : formatUiMessage(locale, "settings.profile.saveAction")}
            </button>
          </div>
        </form>
      </section>
    </section>
  );
}
