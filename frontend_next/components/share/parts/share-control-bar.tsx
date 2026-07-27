"use client";

import { formatUiMessage } from "../../../lib/i18n/messages";
import { shareStatusBadgeClass } from "./share-center-ui";
import badgeStyles from "./share-center-ui.module.css";
import styles from "./share-control-bar.module.css";
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
    <div data-testid="share-control-bar">
      <div className={styles.stack}>
                <div className={styles.headerRow}>
                  <strong>{formatUiMessage(locale, "shareCenter.controlBarTitle")}</strong>
                  <span
                    className={`${badgeStyles.statusBadge} ${shareStatusBadgeClass(shareStatus)}`}
                  >
                    {shareStatusText}
                  </span>
                </div>
                <p className={styles.subtitle}>
                  {formatUiMessage(locale, "shareCenter.controlBarSubtitle")}
                </p>
              </div>

              <div
                className={`app-inline-surface ${styles.panel}`}
              >
                <div className={styles.switchRow}>
                  <div className={styles.switchLabelStack}>
                    <span className={styles.switchLabel}>
                      {formatUiMessage(locale, "shareCenter.shareSwitchLabel")}
                    </span>
                    <strong className={styles.switchState}>
                      {shareSwitchChecked
                        ? formatUiMessage(locale, "shareCenter.statusActive")
                        : formatUiMessage(locale, "shareCenter.statusInactive")}
                    </strong>
                  </div>
                  <button
                    aria-checked={shareSwitchChecked}
                    className={`app-button-ghost ${styles.switchTrack}`}
                    disabled={toggleShareMutation.isPending || settingsQuery.isLoading}
                    role="switch"
                    style={{
                      background: shareSwitchChecked
                        ? "hsl(var(--accent))"
                        : "hsl(var(--muted))",
                      justifyContent: shareSwitchChecked ? "flex-end" : "flex-start",
                    }}
                    type="button"
                    onClick={() => void handleToggleShare()}
                  >
                    <span
                      aria-hidden="true"
                      className={styles.switchKnob}
                      style={{
                        background: shareSwitchChecked
                          ? "hsl(var(--background))"
                          : "hsl(var(--muted-foreground))",
                      }}
                    />
                  </button>
                </div>
                <div className={styles.fieldStack}>
                  <label className="app-form-label" htmlFor="share-validity">
                    {formatUiMessage(locale, "shareCenter.validityLabel")}
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
                  <p className={`app-form-footnote ${styles.footnote}`}>
                    {formatUiMessage(locale, "shareCenter.validityHint")}
                  </p>
                </div>
                <div className={styles.urlStack}>
                  <span className={styles.urlLabel}>
                    {formatUiMessage(locale, "shareCenter.shareUrlLabel")}
                  </span>
                  <div
                    className={styles.shareUrl}
                    data-testid="share-link"
                  >
                    {shareUrl ||
                      formatUiMessage(locale, "shareCenter.controlBarNoLink")}
                  </div>
                </div>
              </div>

              <div
                className={`app-button-row ${styles.actions}`}
              >
                <button
                  className={`app-button-ghost ${styles.actionButton}`}
                  disabled={!canUseShareLink}
                  type="button"
                  onClick={() => void handleCopyShareLink()}
                >
                  {formatUiMessage(locale, "shareCenter.copyLinkAction")}
                </button>
                <button
                  className={`app-button-secondary ${styles.actionButton}`}
                  disabled={!canUseShareLink}
                  type="button"
                  onClick={() => handleOpenSharePage()}
                >
                  {formatUiMessage(locale, "shareCenter.openShareAction")}
                </button>
                <button
                  className={`app-button-primary ${styles.actionButton}`}
                  disabled={
                    refreshShareMutation.isPending ||
                    settingsQuery.isLoading ||
                    !settingsQuery.data?.share_token
                  }
                  type="button"
                  onClick={() => void handleRefreshShare()}
                >
                  {refreshShareMutation.isPending
                    ? formatUiMessage(locale, "shareCenter.saving")
                    : formatUiMessage(locale, "shareCenter.updateShareAction")}
                </button>
              </div>
    </div>
  );
}
