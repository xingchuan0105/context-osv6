"use client";

import { useEffect, useState } from "react";
import { useMutation } from "@tanstack/react-query";
import { useForm } from "react-hook-form";
import { z } from "zod";

import { describeAuthError } from "../../lib/auth/errors";
import { useAuth } from "../../lib/auth/context";
import { formatUiMessage } from "../../lib/i18n/messages";
import { updateProfile } from "../../lib/settings/client";
import { useUiPreferences } from "../../lib/ui-preferences";
import { applyZodErrors, bannerStyle, type ProfileFormValues } from "./settings-shared";
import styles from "./settings-profile-panel.module.css";
import shared from "./settings-ui-shared.module.css";

export function ProfilePanel() {
  const auth = useAuth();
  const { locale } = useUiPreferences();
  const profileForm = useForm<ProfileFormValues>({
    defaultValues: {
      fullName: auth.user?.full_name ?? "",
    },
  });
  const [banner, setBanner] = useState("");
  const [actionError, setActionError] = useState("");
  const profileMutation = useMutation({
    mutationFn: async (fullName: string | null) => {
      if (!auth.token) {
        throw new Error(formatUiMessage(locale, "settings.profile.notAuthenticated"));
      }

      const response = await updateProfile(auth.token, fullName);

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

  useEffect(() => {
    profileForm.reset({
      fullName: auth.user?.full_name ?? "",
    });
  }, [auth.user?.full_name, profileForm]);

  const profileSchema = z.object({
    fullName: z.string().trim().max(120, {
      message: formatUiMessage(locale, "settings.profile.nameTooLong"),
    }),
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
      await profileMutation.mutateAsync(parsed.data.fullName || null);
    } catch (error) {
      setActionError(
        describeAuthError(
          formatUiMessage(locale, "settings.saveError"),
          error,
        ),
      );
    }
  }

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
          {banner ? (
            <p className="app-notice-banner" style={bannerStyle("success")}>
              {banner}
            </p>
          ) : null}
          {actionError ? <p className="app-notice-banner">{actionError}</p> : null}
          <div className="app-button-row">
            <button
              className="app-button-primary"
              disabled={profileMutation.isPending}
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

