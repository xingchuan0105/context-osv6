"use client";

import { formatSettingsShareMessage } from "../../../lib/settings-share-messages";
import { shareStatusBadgeStyle } from "./share-center-ui";
import { shareValidityLabel, type ShareValidityOption } from "./share-center-utils";
import type { useShareCenter } from "./use-share-center";

type ShareCenter = ReturnType<typeof useShareCenter>;

export function ShareControlBar({ center }: { center: ShareCenter }) {
  const {
    canUseShareLink,
    expiresAtDraft,
    handleCopyShareLink,
    handleOpenSharePage,
    handleRefreshShare,
    handleToggleShare,
    locale,
    refreshShareMutation,
    settingsQuery,
    shareStatus,
    shareStatusText,
    shareSwitchChecked,
    shareUrl,
    toggleShareMutation,
    validityOptions,
    setExpiresAtDraft,
  } = center;

  return (
    <>
      <div style={{ display: "grid", gap: "0.25rem" }}>
                <div
                  style={{
                    alignItems: "center",
                    display: "flex",
                    flexWrap: "wrap",
                    gap: "0.6rem",
                    justifyContent: "space-between",
                  }}
                >
                  <strong>{formatSettingsShareMessage(locale, "shareCenter.controlBarTitle")}</strong>
                  <span
                    style={{
                      ...shareStatusBadgeStyle(shareStatus),
                      borderRadius: "999px",
                      fontSize: "0.76rem",
                      fontWeight: 600,
                      letterSpacing: "-0.01em",
                      padding: "0.28rem 0.62rem",
                      whiteSpace: "nowrap",
                    }}
                  >
                    {shareStatusText}
                  </span>
                </div>
                <p
                  style={{
                    color: "hsl(var(--muted-foreground))",
                    margin: 0,
                    fontSize: "0.96rem",
                    lineHeight: 1.5,
                  }}
                >
                  {formatSettingsShareMessage(locale, "shareCenter.controlBarSubtitle")}
                </p>
              </div>

              <div
                className="app-inline-surface"
                style={{
                  display: "grid",
                  gap: "0.8rem",
                  padding: "0.78rem 0.88rem 0.82rem",
                }}
              >
                <div
                  style={{
                    alignItems: "center",
                    display: "flex",
                    flexWrap: "wrap",
                    gap: "0.8rem",
                    justifyContent: "space-between",
                  }}
                >
                  <div style={{ display: "grid", gap: "0.2rem" }}>
                    <span style={{ color: "hsl(var(--muted-foreground))", fontSize: "0.82rem" }}>
                      {formatSettingsShareMessage(locale, "shareCenter.shareSwitchLabel")}
                    </span>
                    <strong style={{ fontSize: "0.92rem", letterSpacing: "-0.01em" }}>
                      {shareSwitchChecked
                        ? formatSettingsShareMessage(locale, "shareCenter.statusActive")
                        : formatSettingsShareMessage(locale, "shareCenter.statusInactive")}
                    </strong>
                  </div>
                  <button
                    aria-checked={shareSwitchChecked}
                    className="app-button-ghost"
                    disabled={toggleShareMutation.isPending || settingsQuery.isLoading}
                    role="switch"
                    style={{
                      alignItems: "center",
                      background: shareSwitchChecked
                        ? "hsl(var(--foreground))"
                        : "hsl(var(--muted))",
                      border: "1px solid hsl(var(--border))",
                      borderRadius: "999px",
                      display: "inline-flex",
                      height: "2rem",
                      justifyContent: shareSwitchChecked ? "flex-end" : "flex-start",
                      minWidth: "3.55rem",
                      padding: "0.16rem",
                    }}
                    type="button"
                    onClick={() => void handleToggleShare()}
                  >
                    <span
                      aria-hidden="true"
                      style={{
                        background: shareSwitchChecked
                          ? "hsl(var(--background))"
                          : "hsl(var(--foreground))",
                        borderRadius: "999px",
                        display: "block",
                        height: "1.52rem",
                        width: "1.52rem",
                      }}
                    />
                  </button>
                </div>
                <div style={{ display: "grid", gap: "0.35rem" }}>
                  <label className="app-form-label" htmlFor="share-validity">
                    {formatSettingsShareMessage(locale, "shareCenter.validityLabel")}
                  </label>
                  <select
                    className="app-input"
                    disabled={toggleShareMutation.isPending || refreshShareMutation.isPending}
                    id="share-validity"
                    value={expiresAtDraft}
                    onChange={(event) =>
                      setExpiresAtDraft(event.target.value as ShareValidityOption)
                    }
                  >
                    {validityOptions.map((option) => (
                      <option key={option} value={option}>
                        {shareValidityLabel(locale, option)}
                      </option>
                    ))}
                  </select>
                  <p className="app-form-footnote" style={{ fontSize: "0.82rem", margin: 0 }}>
                    {formatSettingsShareMessage(locale, "shareCenter.validityHint")}
                  </p>
                </div>
                <div
                  style={{
                    color: "hsl(var(--muted-foreground))",
                    display: "grid",
                    gap: "0.3rem",
                  }}
                >
                  <span style={{ fontSize: "0.82rem" }}>
                    {formatSettingsShareMessage(locale, "shareCenter.shareUrlLabel")}
                  </span>
                  <div
                    style={{
                      fontFamily: "ui-monospace, SFMono-Regular, Menlo, monospace",
                      fontSize: "0.84rem",
                      lineHeight: 1.5,
                      overflowWrap: "anywhere",
                    }}
                  >
                    {shareUrl ||
                      formatSettingsShareMessage(locale, "shareCenter.controlBarNoLink")}
                  </div>
                </div>
              </div>

              <div
                className="app-button-row"
                style={{
                  display: "grid",
                  gap: "0.6rem",
                  gridTemplateColumns: "minmax(0, 1fr)",
                }}
              >
                <button
                  className="app-button-ghost"
                  disabled={!canUseShareLink}
                  style={{
                    fontSize: "0.9rem",
                    justifyContent: "center",
                    minHeight: "2.4rem",
                    padding: "0.62rem 0.8rem",
                    width: "100%",
                  }}
                  type="button"
                  onClick={() => void handleCopyShareLink()}
                >
                  {formatSettingsShareMessage(locale, "shareCenter.copyLinkAction")}
                </button>
                <button
                  className="app-button-secondary"
                  disabled={!canUseShareLink}
                  style={{
                    fontSize: "0.9rem",
                    justifyContent: "center",
                    minHeight: "2.4rem",
                    padding: "0.62rem 0.8rem",
                    width: "100%",
                  }}
                  type="button"
                  onClick={() => handleOpenSharePage()}
                >
                  {formatSettingsShareMessage(locale, "shareCenter.openShareAction")}
                </button>
                <button
                  className="app-button-primary"
                  disabled={
                    refreshShareMutation.isPending ||
                    settingsQuery.isLoading ||
                    !settingsQuery.data?.share_token
                  }
                  style={{
                    fontSize: "0.9rem",
                    justifyContent: "center",
                    minHeight: "2.4rem",
                    padding: "0.62rem 0.8rem",
                    width: "100%",
                  }}
                  type="button"
                  onClick={() => void handleRefreshShare()}
                >
                  {refreshShareMutation.isPending
                    ? formatSettingsShareMessage(locale, "shareCenter.saving")
                    : formatSettingsShareMessage(locale, "shareCenter.updateShareAction")}
                </button>
              </div>
    </>
  );
}
