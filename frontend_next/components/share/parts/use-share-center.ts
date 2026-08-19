"use client";

import { useEffect, useState } from "react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";

import { useAuth } from "../../../lib/auth/context";
import { formatUiMessage } from "../../../lib/i18n/messages";
import {
  accessLevelFromVisitorMode,
  buildShareUrl,
  createShareLink,
  getShareAccessLogs,
  getShareAnalytics,
  getShareQuota,
  getShareSettings,
  inviteMember,
  listMembers,
  removeMember,
  revokeShareLink,
  shareActionErrorMessage,
  type MembersResponse,
  type VisitorAccessMode,
  updateShareSettings,
  visitorModeFromAccessLevel,
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

export function useShareCenter(
  workspaceId: string,
  options?: { queriesEnabled?: boolean; sharePublicOrigin?: string | null },
) {
  const auth = useAuth();
  const { locale } = useUiPreferences();
  const queryClient = useQueryClient();
  const workspaceReady = hasWorkspaceId(workspaceId);
  const queriesEnabled = options?.queriesEnabled ?? true;
  const canQuery = Boolean(auth.token && workspaceReady && queriesEnabled);
  const invalidWorkspaceMessage =
    formatUiMessage(locale, "shareInvalidWorkspaceId");
  const [actionError, setActionError] = useState("");
  const [actionMessage, setActionMessage] = useState("");
  const [expiresAtDraft, setExpiresAtDraft] = useState<ShareValidityOption>("30d");
  const [visitorModeDraft, setVisitorModeDraft] = useState<VisitorAccessMode>("require_register");
  const [pendingEnableConfirm, setPendingEnableConfirm] = useState(false);
  const [inviteEmail, setInviteEmail] = useState("");
  const [inviteRole, setInviteRole] = useState<"viewer" | "editor">("viewer");
  const [inviteError, setInviteError] = useState("");
  const [pendingRemoveMemberId, setPendingRemoveMemberId] = useState<string | null>(null);
  const [anonLimitDraft, setAnonLimitDraft] = useState("10");
  const [memberLimitDraft, setMemberLimitDraft] = useState("");
  const settingsQuery = useQuery({
    queryKey: shareKeys.settings(workspaceId, auth.token),
    enabled: canQuery,
    queryFn: () => getShareSettings(auth.token as string, workspaceId),
  });
  const quotaQuery = useQuery({
    queryKey: shareKeys.quota(auth.token),
    enabled: Boolean(auth.token && queriesEnabled),
    queryFn: () => getShareQuota(auth.token as string),
  });
  const membersQuery = useQuery<MembersResponse>({
    queryKey: shareKeys.members(workspaceId, auth.token),
    enabled: canQuery,
    queryFn: () => listMembers(auth.token as string, workspaceId),
  });
  const analyticsQuery = useQuery({
    queryKey: shareKeys.analytics(workspaceId, auth.token),
    enabled: canQuery,
    queryFn: () => getShareAnalytics(auth.token as string, workspaceId),
  });
  const accessLogsQuery = useQuery({
    queryKey: shareKeys.accessLogs(workspaceId, auth.token),
    enabled: canQuery,
    queryFn: () => getShareAccessLogs(auth.token as string, workspaceId),
  });
  const toggleShareMutation = useMutation({
    mutationFn: async () => {
      if (!auth.token) {
        throw new Error(formatUiMessage(locale, "shareCenter.loginRequired"));
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
        // Limits only via dedicated save control (do not couple drafts into enable).
        return updateShareSettings(auth.token, workspaceId, {
          access_level: accessLevelFromVisitorMode(visitorModeDraft),
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
      await queryClient.invalidateQueries({
        queryKey: shareKeys.quota(auth.token),
      });
    },
  });
  const refreshShareMutation = useMutation({
    mutationFn: async () => {
      if (!auth.token) {
        throw new Error(formatUiMessage(locale, "shareCenter.loginRequired"));
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

      // Validity refresh only — question caps stay on explicit save.
      return updateShareSettings(auth.token, workspaceId, {
        access_level: accessLevelFromVisitorMode(visitorModeDraft),
        allow_download: false,
      });
    },
    onSuccess: async (settings) => {
      queryClient.setQueryData(shareKeys.settings(workspaceId, auth.token), settings);
      await queryClient.invalidateQueries({
        queryKey: shareKeys.settings(workspaceId, auth.token),
      });
      await queryClient.invalidateQueries({
        queryKey: shareKeys.quota(auth.token),
      });
    },
  });
  const questionLimitsMutation = useMutation({
    mutationFn: async () => {
      if (!auth.token) {
        throw new Error(formatUiMessage(locale, "shareCenter.loginRequired"));
      }
      if (!workspaceReady) {
        throw new Error(invalidWorkspaceMessage);
      }
      return updateShareSettings(auth.token, workspaceId, {
        access_level: settingsQuery.data?.access_level,
        allow_download: settingsQuery.data?.allow_download,
        ...questionLimitsPayload(),
      });
    },
    onSuccess: async (settings) => {
      queryClient.setQueryData(shareKeys.settings(workspaceId, auth.token), settings);
      await queryClient.invalidateQueries({
        queryKey: shareKeys.settings(workspaceId, auth.token),
      });
    },
  });
  const visitorModeMutation = useMutation({
    mutationFn: async (mode: VisitorAccessMode) => {
      if (!auth.token) {
        throw new Error(formatUiMessage(locale, "shareCenter.loginRequired"));
      }

      if (!workspaceReady) {
        throw new Error(invalidWorkspaceMessage);
      }

      // Only persist when share is configured (not private). Draft still updates locally.
      const currentLevel = settingsQuery.data?.access_level;
      if (!currentLevel || currentLevel === "private") {
        return settingsQuery.data ?? null;
      }

      return updateShareSettings(auth.token, workspaceId, {
        access_level: accessLevelFromVisitorMode(mode),
        allow_download: settingsQuery.data?.allow_download ?? false,
      });
    },
    onSuccess: async (settings) => {
      if (!settings) {
        return;
      }

      queryClient.setQueryData(shareKeys.settings(workspaceId, auth.token), settings);
      await queryClient.invalidateQueries({
        queryKey: shareKeys.settings(workspaceId, auth.token),
      });
    },
  });
  const inviteMemberMutation = useMutation({
    mutationFn: async () => {
      if (!auth.token) {
        throw new Error(formatUiMessage(locale, "shareCenter.loginRequired"));
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
        throw new Error(formatUiMessage(locale, "shareCenter.loginRequired"));
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
    if (settingsQuery.data.access_level !== "private") {
      setVisitorModeDraft(visitorModeFromAccessLevel(settingsQuery.data.access_level));
    }
    setAnonLimitDraft(String(settingsQuery.data.anon_question_limit ?? 10));
    setMemberLimitDraft(
      settingsQuery.data.member_question_limit == null
        ? ""
        : String(settingsQuery.data.member_question_limit),
    );
  }, [settingsQuery.data]);

  function questionLimitsPayload() {
    const anonParsed = Number.parseInt(anonLimitDraft.trim(), 10);
    const anon_question_limit = Number.isFinite(anonParsed) && anonParsed >= 0 ? anonParsed : 10;
    const memberTrim = memberLimitDraft.trim();
    if (!memberTrim) {
      return {
        anon_question_limit,
        member_question_limit: null as number | null,
        member_question_limit_set: true,
      };
    }
    const memberParsed = Number.parseInt(memberTrim, 10);
    const member_question_limit =
      Number.isFinite(memberParsed) && memberParsed > 0 ? memberParsed : null;
    return {
      anon_question_limit,
      member_question_limit,
      member_question_limit_set: true,
    };
  }

  const shareUrl = buildShareUrl(
    settingsQuery.data?.share_token ?? "",
    options?.sharePublicOrigin,
  );
  const shareStatus = resolveShareStatus(settingsQuery.data ?? null);
  const shareStatusText = shareStatusLabel(locale, shareStatus);
  const sevenDaySeries = buildDailyViewsSeries(analyticsQuery.data, 7);
  const thirtyDaySeries = buildDailyViewsSeries(analyticsQuery.data, 30);
  const [trendWindowDays, setTrendWindowDays] = useState<7 | 30>(7);
  const trendSeries = trendWindowDays === 7 ? sevenDaySeries : thirtyDaySeries;
  const totalViewsValue =
    analyticsQuery.data?.total_views.toLocaleString() ??
    formatUiMessage(locale, "shareCenter.metricUnavailable");
  const recentViewsValue = analyticsQuery.data
    ? sumViews(sevenDaySeries).toLocaleString()
    : formatUiMessage(locale, "shareCenter.metricUnavailable");
  const activeDaysValue = analyticsQuery.data
    ? String(countActiveDays(thirtyDaySeries))
    : formatUiMessage(locale, "shareCenter.metricUnavailable");
  const latestAccessLog = getLatestAccessLog(accessLogsQuery.data);
  const latestAccessValue = accessLogsQuery.data
    ? latestAccessLog
      ? formatAccessedAt(locale, latestAccessLog.accessed_at)
      : formatUiMessage(locale, "shareCenter.notSet")
    : formatUiMessage(locale, "shareCenter.metricUnavailable");
  const canUseShareLink = shareStatus === "active" && Boolean(shareUrl);
  const shareSwitchChecked = shareStatus === "active";
  const validityOptions: ShareValidityOption[] = ["7d", "30d", "90d", "never"];
  const quotaSummary = quotaQuery.data ?? null;
  const quotaLabel = quotaSummary
    ? formatUiMessage(locale, "shareCenter.quotaValue", {
        used: quotaSummary.used,
        max: quotaSummary.max,
        plan: quotaSummary.plan_id,
      })
    : null;

  function mapShareError(error: unknown) {
    return shareActionErrorMessage(error, locale, "shareCenter.saveError", formatUiMessage);
  }

  async function runToggleShare() {
    setActionError("");
    setActionMessage("");
    setPendingEnableConfirm(false);

    try {
      await toggleShareMutation.mutateAsync();
    } catch (error) {
      setActionError(mapShareError(error));
    }
  }

  async function handleToggleShare() {
    setActionError("");
    setActionMessage("");

    // Force explicit confirm only when enabling (not when turning off).
    if (!shareSwitchChecked) {
      setPendingEnableConfirm(true);
      return;
    }

    await runToggleShare();
  }

  function handleCancelEnableConfirm() {
    setPendingEnableConfirm(false);
  }

  async function handleConfirmEnableShare() {
    await runToggleShare();
  }

  async function handleVisitorModeChange(mode: VisitorAccessMode) {
    setVisitorModeDraft(mode);
    setActionError("");
    setActionMessage("");

    const currentLevel = settingsQuery.data?.access_level;
    if (!currentLevel || currentLevel === "private") {
      return;
    }

    try {
      await visitorModeMutation.mutateAsync(mode);
    } catch (error) {
      setActionError(mapShareError(error));
    }
  }

  async function handleCopyShareLink() {
    setActionError("");
    setActionMessage("");

    if (!canUseShareLink) {
      setActionError(formatUiMessage(locale, "shareCenter.shareLinkUnavailable"));
      return;
    }

    try {
      await navigator.clipboard.writeText(shareUrl);
      setActionMessage(formatUiMessage(locale, "shareCenter.copyLinkSuccess"));
    } catch {
      setActionError(formatUiMessage(locale, "shareCenter.copyLinkError"));
    }
  }

  function handleOpenSharePage() {
    setActionError("");
    setActionMessage("");

    if (!canUseShareLink) {
      setActionError(formatUiMessage(locale, "shareCenter.shareLinkUnavailable"));
      return;
    }

    window.open(shareUrl, "_blank", "noopener,noreferrer");
  }

  async function handleRefreshShare() {
    setActionError("");
    setActionMessage("");

    try {
      await refreshShareMutation.mutateAsync();
      setActionMessage(formatUiMessage(locale, "shareCenter.updateShareSuccess"));
    } catch (error) {
      setActionError(mapShareError(error));
    }
  }

  async function handleSaveQuestionLimits() {
    setActionError("");
    setActionMessage("");
    try {
      await questionLimitsMutation.mutateAsync();
      setActionMessage(formatUiMessage(locale, "shareCenter.updateShareSuccess"));
    } catch (error) {
      setActionError(mapShareError(error));
    }
  }

  async function handleInviteMember() {
    setInviteError("");

    if (!inviteEmail.trim()) {
      setInviteError(formatUiMessage(locale, "shareCenter.inviteEmailRequired"));
      return;
    }

    if (!isValidInviteEmail(inviteEmail)) {
      setInviteError(formatUiMessage(locale, "shareCenter.inviteEmailInvalid"));
      return;
    }

    try {
      await inviteMemberMutation.mutateAsync();
    } catch (error) {
      setInviteError(
        error instanceof Error
          ? error.message
          : formatUiMessage(locale, "shareCenter.membersLoadError"),
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
          : formatUiMessage(locale, "shareCenter.removeError"),
      );
    }
  }

  return {
    accessLogsQuery,
    actionError,
    actionMessage,
    analyticsQuery,
    anonLimitDraft,
    canUseShareLink,
    expiresAtDraft,
    handleCancelEnableConfirm,
    handleConfirmEnableShare,
    handleConfirmRemove,
    handleCopyShareLink,
    handleInviteMember,
    handleOpenSharePage,
    handleRefreshShare,
    handleSaveQuestionLimits,
    handleToggleShare,
    handleVisitorModeChange,
    inviteEmail,
    inviteError,
    inviteMemberMutation,
    inviteRole,
    locale,
    memberLimitDraft,
    membersQuery,
    pendingEnableConfirm,
    pendingRemoveMemberId,
    questionLimitsMutation,
    quotaLabel,
    quotaQuery,
    quotaSummary,
    refreshShareMutation,
    removeMemberMutation,
    setActionError,
    setActionMessage,
    setAnonLimitDraft,
    setExpiresAtDraft,
    setInviteEmail,
    setInviteRole,
    setMemberLimitDraft,
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
    visitorModeDraft,
    visitorModeMutation,
    activeDaysValue,
    latestAccessValue,
    recentViewsValue,
    totalViewsValue,
    workspaceReady,
  };
}
