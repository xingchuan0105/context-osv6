"use client";

import { useEffect, useState } from "react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";

import { useAuth } from "../../../lib/auth/context";
import { formatSettingsShareMessage } from "../../../lib/settings-share-messages";
import {
  buildShareUrl,
  createShareLink,
  getShareAccessLogs,
  getShareAnalytics,
  getShareSettings,
  inviteMember,
  listMembers,
  removeMember,
  revokeShareLink,
  type MembersResponse,
  updateShareSettings,
} from "../../../lib/share/client";
import { useUiPreferences } from "../../../lib/ui-preferences";
import {
  buildDailyViewsSeries,
  buildExpiresAtFromValidity,
  countActiveDays,
  formatAccessedAt,
  getLatestAccessLog,
  hasWorkspaceId,
  isValidInviteEmail,
  resolveShareStatus,
  resolveValidityOption,
  shareKeys,
  shareStatusLabel,
  sumViews,
  type ShareValidityOption,
} from "./share-center-utils";

export function useShareCenter(workspaceId: string) {
  const auth = useAuth();
  const { locale } = useUiPreferences();
  const queryClient = useQueryClient();
  const workspaceReady = hasWorkspaceId(workspaceId);
  const invalidWorkspaceMessage =
    locale === "zh-CN" ? "当前工作区标识无效。" : "Invalid workspace identifier.";
  const [actionError, setActionError] = useState("");
  const [actionMessage, setActionMessage] = useState("");
  const [expiresAtDraft, setExpiresAtDraft] = useState<ShareValidityOption>("30d");
  const [inviteEmail, setInviteEmail] = useState("");
  const [inviteRole, setInviteRole] = useState<"viewer" | "editor">("viewer");
  const [inviteError, setInviteError] = useState("");
  const [pendingRemoveMemberId, setPendingRemoveMemberId] = useState<string | null>(null);
  const settingsQuery = useQuery({
    queryKey: shareKeys.settings(workspaceId, auth.token),
    enabled: Boolean(auth.token && workspaceReady),
    queryFn: () => getShareSettings(auth.token as string, workspaceId),
  });
  const membersQuery = useQuery<MembersResponse>({
    queryKey: shareKeys.members(workspaceId, auth.token),
    enabled: Boolean(auth.token && workspaceReady),
    queryFn: () => listMembers(auth.token as string, workspaceId),
  });
  const analyticsQuery = useQuery({
    queryKey: shareKeys.analytics(workspaceId, auth.token),
    enabled: Boolean(auth.token && workspaceReady),
    queryFn: () => getShareAnalytics(auth.token as string, workspaceId),
  });
  const accessLogsQuery = useQuery({
    queryKey: shareKeys.accessLogs(workspaceId, auth.token),
    enabled: Boolean(auth.token && workspaceReady),
    queryFn: () => getShareAccessLogs(auth.token as string, workspaceId),
  });
  const toggleShareMutation = useMutation({
    mutationFn: async () => {
      if (!auth.token) {
        throw new Error(formatSettingsShareMessage(locale, "shareCenter.loginRequired"));
      }

      if (!workspaceReady) {
        throw new Error(invalidWorkspaceMessage);
      }

      const currentSettings = settingsQuery.data;
      const currentStatus = resolveShareStatus(currentSettings ?? null);

      if (currentStatus === "active" && currentSettings?.share_token) {
        await revokeShareLink(auth.token, workspaceId, currentSettings.share_token);
        return updateShareSettings(auth.token, workspaceId, {
          access_level: "private",
          allow_download: false,
        });
      }

      if (currentSettings?.share_token) {
        await revokeShareLink(auth.token, workspaceId, currentSettings.share_token);
      }

      if (!currentSettings?.share_token || currentStatus !== "active") {
        await createShareLink(auth.token, workspaceId, {
          role: "viewer",
          expires_at: buildExpiresAtFromValidity(expiresAtDraft),
        });
        return updateShareSettings(auth.token, workspaceId, {
          access_level: "link",
          allow_download: false,
        });
      }

      return currentSettings;
    },
    onSuccess: async (settings) => {
      queryClient.setQueryData(shareKeys.settings(workspaceId, auth.token), settings);
      await queryClient.invalidateQueries({
        queryKey: shareKeys.settings(workspaceId, auth.token),
      });
    },
  });
  const refreshShareMutation = useMutation({
    mutationFn: async () => {
      if (!auth.token) {
        throw new Error(formatSettingsShareMessage(locale, "shareCenter.loginRequired"));
      }

      if (!workspaceReady) {
        throw new Error(invalidWorkspaceMessage);
      }

      const currentSettings = settingsQuery.data;
      const nextExpiresAt = buildExpiresAtFromValidity(expiresAtDraft);

      if (currentSettings?.share_token) {
        await revokeShareLink(auth.token, workspaceId, currentSettings.share_token);
      }

      await createShareLink(auth.token, workspaceId, {
        role: "viewer",
        expires_at: nextExpiresAt,
      });

      return updateShareSettings(auth.token, workspaceId, {
        access_level: "link",
        allow_download: false,
      });
    },
    onSuccess: async (settings) => {
      queryClient.setQueryData(shareKeys.settings(workspaceId, auth.token), settings);
      await queryClient.invalidateQueries({
        queryKey: shareKeys.settings(workspaceId, auth.token),
      });
    },
  });
  const inviteMemberMutation = useMutation({
    mutationFn: async () => {
      if (!auth.token) {
        throw new Error(formatSettingsShareMessage(locale, "shareCenter.loginRequired"));
      }

      if (!workspaceReady) {
        throw new Error(invalidWorkspaceMessage);
      }

      return inviteMember(auth.token, workspaceId, inviteEmail.trim(), inviteRole);
    },
    onSuccess: async () => {
      setInviteEmail("");
      setInviteError("");
      await queryClient.invalidateQueries({
        queryKey: shareKeys.members(workspaceId, auth.token),
      });
    },
  });
  const removeMemberMutation = useMutation({
    mutationFn: async (memberId: string) => {
      if (!auth.token) {
        throw new Error(formatSettingsShareMessage(locale, "shareCenter.loginRequired"));
      }

      if (!workspaceReady) {
        throw new Error(invalidWorkspaceMessage);
      }

      return removeMember(auth.token, workspaceId, memberId);
    },
    onSuccess: async () => {
      setPendingRemoveMemberId(null);
      await queryClient.invalidateQueries({
        queryKey: shareKeys.members(workspaceId, auth.token),
      });
    },
  });
  useEffect(() => {
    if (!settingsQuery.data) {
      return;
    }

    setExpiresAtDraft(resolveValidityOption(settingsQuery.data.expires_at));
  }, [settingsQuery.data]);

  const shareUrl = buildShareUrl(settingsQuery.data?.share_token ?? "");
  const shareStatus = resolveShareStatus(settingsQuery.data ?? null);
  const shareStatusText = shareStatusLabel(locale, shareStatus);
  const sevenDaySeries = buildDailyViewsSeries(analyticsQuery.data, 7);
  const thirtyDaySeries = buildDailyViewsSeries(analyticsQuery.data, 30);
  const [trendWindowDays, setTrendWindowDays] = useState<7 | 30>(7);
  const trendSeries = trendWindowDays === 7 ? sevenDaySeries : thirtyDaySeries;
  const totalViewsValue =
    analyticsQuery.data?.total_views.toLocaleString() ??
    formatSettingsShareMessage(locale, "shareCenter.metricUnavailable");
  const recentViewsValue = analyticsQuery.data
    ? sumViews(sevenDaySeries).toLocaleString()
    : formatSettingsShareMessage(locale, "shareCenter.metricUnavailable");
  const activeDaysValue = analyticsQuery.data
    ? String(countActiveDays(thirtyDaySeries))
    : formatSettingsShareMessage(locale, "shareCenter.metricUnavailable");
  const latestAccessLog = getLatestAccessLog(accessLogsQuery.data);
  const latestAccessValue = accessLogsQuery.data
    ? latestAccessLog
      ? formatAccessedAt(locale, latestAccessLog.accessed_at)
      : formatSettingsShareMessage(locale, "shareCenter.notSet")
    : formatSettingsShareMessage(locale, "shareCenter.metricUnavailable");
  const canUseShareLink = shareStatus === "active" && Boolean(shareUrl);
  const shareSwitchChecked = shareStatus === "active";
  const validityOptions: ShareValidityOption[] = ["7d", "30d", "90d", "never"];

  async function handleToggleShare() {
    setActionError("");
    setActionMessage("");

    try {
      await toggleShareMutation.mutateAsync();
    } catch (error) {
      setActionError(
        error instanceof Error
          ? error.message
          : formatSettingsShareMessage(locale, "shareCenter.saveError"),
      );
    }
  }


  async function handleCopyShareLink() {
    setActionError("");
    setActionMessage("");

    if (!canUseShareLink) {
      setActionError(formatSettingsShareMessage(locale, "shareCenter.shareLinkUnavailable"));
      return;
    }

    try {
      await navigator.clipboard.writeText(shareUrl);
      setActionMessage(formatSettingsShareMessage(locale, "shareCenter.copyLinkSuccess"));
    } catch {
      setActionError(formatSettingsShareMessage(locale, "shareCenter.copyLinkError"));
    }
  }

  function handleOpenSharePage() {
    setActionError("");
    setActionMessage("");

    if (!canUseShareLink) {
      setActionError(formatSettingsShareMessage(locale, "shareCenter.shareLinkUnavailable"));
      return;
    }

    window.open(shareUrl, "_blank", "noopener,noreferrer");
  }

  async function handleRefreshShare() {
    setActionError("");
    setActionMessage("");

    try {
      await refreshShareMutation.mutateAsync();
      setActionMessage(formatSettingsShareMessage(locale, "shareCenter.updateShareSuccess"));
    } catch (error) {
      setActionError(
        error instanceof Error
          ? error.message
          : formatSettingsShareMessage(locale, "shareCenter.saveError"),
      );
    }
  }

  async function handleInviteMember() {
    setInviteError("");

    if (!inviteEmail.trim()) {
      setInviteError(formatSettingsShareMessage(locale, "shareCenter.inviteEmailRequired"));
      return;
    }

    if (!isValidInviteEmail(inviteEmail)) {
      setInviteError(formatSettingsShareMessage(locale, "shareCenter.inviteEmailInvalid"));
      return;
    }

    try {
      await inviteMemberMutation.mutateAsync();
    } catch (error) {
      setInviteError(
        error instanceof Error
          ? error.message
          : formatSettingsShareMessage(locale, "shareCenter.membersLoadError"),
      );
    }
  }

  async function handleConfirmRemove(memberId: string) {
    setActionError("");

    try {
      await removeMemberMutation.mutateAsync(memberId);
    } catch (error) {
      setActionError(
        error instanceof Error
          ? error.message
          : formatSettingsShareMessage(locale, "shareCenter.removeError"),
      );
    }
  }

  return {
    accessLogsQuery,
    actionError,
    actionMessage,
    analyticsQuery,
    canUseShareLink,
    expiresAtDraft,
    handleConfirmRemove,
    handleCopyShareLink,
    handleInviteMember,
    handleOpenSharePage,
    handleRefreshShare,
    handleToggleShare,
    inviteEmail,
    inviteError,
    inviteMemberMutation,
    inviteRole,
    locale,
    membersQuery,
    pendingRemoveMemberId,
    refreshShareMutation,
    removeMemberMutation,
    setActionError,
    setActionMessage,
    setExpiresAtDraft,
    setInviteEmail,
    setInviteRole,
    setPendingRemoveMemberId,
    setTrendWindowDays,
    settingsQuery,
    shareStatus,
    shareStatusText,
    shareSwitchChecked,
    shareUrl,
    toggleShareMutation,
    trendSeries,
    trendWindowDays,
    validityOptions,
    activeDaysValue,
    latestAccessValue,
    recentViewsValue,
    totalViewsValue,
    workspaceReady,
  };
}
