"use client";

import { useEffect, useState } from "react";
import { useMutation } from "@tanstack/react-query";
import { useForm } from "react-hook-form";
import { z } from "zod";

import { describeAuthError } from "../../lib/auth/errors";
import { useAuth } from "../../lib/auth/context";
import { formatSettingsShareMessage } from "../../lib/settings-share-messages";
import { updateProfile } from "../../lib/settings/client";
import { useUiPreferences } from "../../lib/ui-preferences";
import { applyZodErrors, bannerStyle, type ProfileFormValues } from "./settings-shared";

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
        throw new Error(formatSettingsShareMessage(locale, "settings.profile.notAuthenticated"));
      }

      const response = await updateProfile(auth.token, fullName);

      if (!response.success || !response.data) {
        throw new Error(
          response.error ?? formatSettingsShareMessage(locale, "settings.saveError"),
        );
      }

      return response.data.user;
    },
    onSuccess: (user) => {
      auth.updateUser(user);
      setBanner(formatSettingsShareMessage(locale, "settings.saveSuccess"));
    },
  });

  useEffect(() => {
    profileForm.reset({
      fullName: auth.user?.full_name ?? "",
    });
  }, [auth.user?.full_name, profileForm]);

  const profileSchema = z.object({
    fullName: z.string().trim().max(120, {
      message: formatSettingsShareMessage(locale, "settings.profile.nameTooLong"),
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
          formatSettingsShareMessage(locale, "settings.saveError"),
          error,
        ),
      );
    }
  }

  return (
    <section style={{ display: "grid", gap: "1rem" }}>
      <section className="app-inline-surface" style={{ display: "grid", gap: "1rem" }}>
        <div style={{ display: "grid", gap: "0.35rem" }}>
          <h2 style={{ margin: 0 }}>
            {formatSettingsShareMessage(locale, "settings.profile.sectionTitle")}
          </h2>
          <p style={{ margin: 0, color: "hsl(var(--muted-foreground))" }}>
            {formatSettingsShareMessage(locale, "settings.profile.sectionSubtitle")}
          </p>
        </div>
        <form
          noValidate
          style={{ display: "grid", gap: "1rem" }}
          onSubmit={profileForm.handleSubmit(handleSubmit)}
        >
          <div>
            <label className="app-form-label" htmlFor="settings-profile-email">
              {formatSettingsShareMessage(locale, "settings.profile.emailLabel")}
            </label>
            <input
              className="app-input"
              id="settings-profile-email"
              readOnly
              style={{ color: "hsl(var(--muted-foreground))" }}
              type="email"
              value={auth.user?.email ?? ""}
            />
          </div>
          <div>
            <label className="app-form-label" htmlFor="settings-profile-name">
              {formatSettingsShareMessage(locale, "settings.profile.nameLabel")}
            </label>
            <input
              className="app-input"
              id="settings-profile-name"
              placeholder={formatSettingsShareMessage(locale, "settings.profile.namePlaceholder")}
              type="text"
              {...profileForm.register("fullName")}
            />
            {profileForm.formState.errors.fullName?.message ? (
              <p className="app-form-footnote" style={{ color: "hsl(var(--destructive))" }}>
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
                ? formatSettingsShareMessage(locale, "shareCenter.saving")
                : formatSettingsShareMessage(locale, "settings.profile.saveAction")}
            </button>
          </div>
        </form>
      </section>
    </section>
  );
}

