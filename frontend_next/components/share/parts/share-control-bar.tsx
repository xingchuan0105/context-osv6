"use client";

import type { UiLocale } from "../../../lib/i18n/config";
import { formatUiMessage } from "../../../lib/i18n/messages";
import type { VisitorAccessMode } from "../../../lib/share/client";
import { shareStatusBadgeClass } from "./share-center-ui";
import badgeStyles from "./share-center-ui.module.css";
import styles from "./share-control-bar.module.css";
import { shareValidityLabel, type ShareValidityOption } from "./share-center-utils";
import type { useShareCenter } from "./use-share-center";
import type { useDesktopPublishGate } from "./use-desktop-publish-gate";

type ShareCenter = ReturnType<typeof useShareCenter>;
type DesktopPublishGate = ReturnType<typeof useDesktopPublishGate>;

function stageLabel(locale: UiLocale, stage: string) {
  switch (stage) {
    case "pack":
      return formatUiMessage(locale, "shareCenter.desktopPublishStagePack");
    case "upload":
      return formatUiMessage(locale, "shareCenter.desktopPublishStageUpload");
    case "commit":
    case "metadata":
    case "vectors":
      return formatUiMessage(locale, "shareCenter.desktopPublishStageCommit");
    case "done":
      return formatUiMessage(locale, "shareCenter.desktopPublishStageDone");
    default:
      return stage;
  }
}

export function ShareControlBar({
  center,
  publishGate,
}: {
  center: ShareCenter;
  publishGate?: DesktopPublishGate;
}) {
  const {
    canUseShareLink,
    expiresAtDraft,
    handleCancelEnableConfirm,
    handleConfirmEnableShare,
    handleCopyShareLink,
    handleOpenSharePage,
    handleRefreshShare,
    handleSaveQuestionLimits,
    handleToggleShare,
    handleVisitorModeChange,
    locale,
    anonLimitDraft,
    memberLimitDraft,
    pendingEnableConfirm,
    questionLimitsMutation,
    quotaLabel,
    refreshShareMutation,
    settingsQuery,
    shareStatus,
    shareStatusText,
    shareSwitchChecked,
    shareUrl,
    toggleShareMutation,
    validityOptions,
    visitorModeDraft,
    visitorModeMutation,
    setAnonLimitDraft,
    setExpiresAtDraft,
    setMemberLimitDraft,
  } = center;

  return (
    <div className={styles.stack} data-testid="share-control-bar">
      <div className={styles.stack}>
        <div className={styles.headerRow}>
          <strong>{formatUiMessage(locale, "shareCenter.controlBarTitle")}</strong>
          <span className={`${badgeStyles.statusBadge} ${shareStatusBadgeClass(shareStatus)}`}>
            {shareStatusText}
          </span>
        </div>
        <p className={styles.subtitle}>
          {publishGate?.desktop
            ? formatUiMessage(locale, "shareCenter.desktopPublishSubtitle")
            : formatUiMessage(locale, "shareCenter.controlBarSubtitle")}
        </p>
        {publishGate?.desktop && !publishGate.ready ? (
          <div className={`app-inline-surface ${styles.panel}`} data-testid="desktop-publish-gate">
            <p className={styles.subtitle}>
              {formatUiMessage(locale, "shareCenter.desktopPublishHint")}
            </p>
            {publishGate.error ? (
              <p className="app-notice-banner" data-testid="desktop-publish-error">
                {publishGate.error}
              </p>
            ) : null}
            {!publishGate.statusKnown ? (
              <button
                className="app-button-secondary"
                data-testid="desktop-publish-retry-status"
                type="button"
                onClick={() => void publishGate.onRetryStatus()}
              >
                {formatUiMessage(locale, "shareCenter.desktopPublishRetryStatus")}
              </button>
            ) : null}
            {publishGate.progress && (publishGate.publishing || publishGate.status === "publishing") ? (
              <div className={styles.stack} data-testid="desktop-publish-progress">
                <span>
                  {stageLabel(locale, publishGate.progress.stage)}
                  {publishGate.progress.total > 0
                    ? ` ${publishGate.progress.current}/${publishGate.progress.total}`
                    : ""}
                </span>
                <div
                  aria-valuemax={Math.max(publishGate.progress.total, 1)}
                  aria-valuenow={publishGate.progress.current}
                  aria-valuemin={0}
                  role="progressbar"
                  style={{
                    height: 4,
                    background: "hsl(var(--muted))",
                    overflow: "hidden",
                  }}
                >
                  <div
                    style={{
                      height: "100%",
                      width: `${Math.min(
                        100,
                        (publishGate.progress.current /
                          Math.max(publishGate.progress.total, 1)) *
                          100,
                      )}%`,
                      background: "hsl(var(--accent))",
                    }}
                  />
                </div>
              </div>
            ) : null}
            {publishGate.statusKnown ? (
            <button
              className="app-button-primary"
              data-testid="desktop-publish-cta"
              disabled={publishGate.publishing}
              type="button"
              onClick={() => void publishGate.onPublish()}
            >
              {publishGate.publishing
                ? formatUiMessage(locale, "shareCenter.desktopPublishInProgress")
                : formatUiMessage(locale, "shareCenter.desktopPublishCta")}
            </button>
            ) : null}
          </div>
        ) : null}
        {quotaLabel ? (
          <p className={styles.quotaLine} data-testid="share-quota">
            <span className={styles.quotaLabel}>
              {formatUiMessage(locale, "shareCenter.quotaLabel")}
            </span>
            <strong>{quotaLabel}</strong>
          </p>
        ) : null}
      </div>

      <div className={`app-inline-surface ${styles.panel}`}>
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
            data-testid="share-switch"
            disabled={
              Boolean(publishGate?.desktop && !publishGate.ready) ||
              toggleShareMutation.isPending ||
              settingsQuery.isLoading ||
              pendingEnableConfirm
            }
            role="switch"
            style={{
              background: shareSwitchChecked ? "hsl(var(--accent))" : "hsl(var(--muted))",
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

        {pendingEnableConfirm ? (
          <div
            className={`app-inline-surface ${styles.confirmPanel}`}
            data-testid="share-enable-confirm"
          >
            <strong className={styles.confirmTitle}>
              {formatUiMessage(locale, "shareCenter.ownerCostConfirmTitle")}
            </strong>
            <p className={styles.confirmBody}>
              {formatUiMessage(locale, "shareCenter.ownerCostConfirmBody")}
            </p>
            <div className={`app-button-row ${styles.confirmActions}`}>
              <button
                className="app-button-primary"
                data-testid="share-enable-confirm-action"
                disabled={toggleShareMutation.isPending}
                type="button"
                onClick={() => void handleConfirmEnableShare()}
              >
                {toggleShareMutation.isPending
                  ? formatUiMessage(locale, "shareCenter.saving")
                  : formatUiMessage(locale, "shareCenter.ownerCostConfirmAction")}
              </button>
              <button
                className="app-button-ghost"
                data-testid="share-enable-confirm-cancel"
                disabled={toggleShareMutation.isPending}
                type="button"
                onClick={handleCancelEnableConfirm}
              >
                {formatUiMessage(locale, "shareCenter.ownerCostConfirmCancel")}
              </button>
            </div>
          </div>
        ) : null}

        <div className={styles.fieldStack}>
          <label className="app-form-label" htmlFor="share-visitor-mode">
            {formatUiMessage(locale, "shareCenter.visitorModeLabel")}
          </label>
          <select
            className="app-input"
            data-testid="share-visitor-mode"
            disabled={
              toggleShareMutation.isPending ||
              refreshShareMutation.isPending ||
              visitorModeMutation.isPending
            }
            id="share-visitor-mode"
            value={visitorModeDraft}
            onChange={(event) =>
              void handleVisitorModeChange(event.target.value as VisitorAccessMode)
            }
          >
            <option value="require_register">
              {formatUiMessage(locale, "shareCenter.visitorMode.requireRegister")}
            </option>
            <option value="anonymous">
              {formatUiMessage(locale, "shareCenter.visitorMode.anonymous")}
            </option>
          </select>
          <p className={`app-form-footnote ${styles.footnote}`}>
            {formatUiMessage(locale, "shareCenter.visitorModeHint")}
          </p>
        </div>

        {settingsQuery.data ? (
          <div className={styles.fieldStack} data-testid="share-question-limits">
            <p className={styles.switchLabel}>
              {formatUiMessage(locale, "shareCenter.questionLimitsLabel")}
            </p>
            <label className="app-form-label" htmlFor="share-anon-limit">
              {formatUiMessage(locale, "shareCenter.anonLimitLabel")}
            </label>
            <input
              className="app-input"
              data-testid="share-anon-limit"
              id="share-anon-limit"
              inputMode="numeric"
              min={0}
              type="number"
              value={anonLimitDraft}
              onChange={(event) => setAnonLimitDraft(event.target.value)}
            />
            <label className="app-form-label" htmlFor="share-member-limit">
              {formatUiMessage(locale, "shareCenter.memberLimitLabel")}
            </label>
            <input
              className="app-input"
              data-testid="share-member-limit"
              id="share-member-limit"
              inputMode="numeric"
              min={1}
              placeholder="∞"
              type="number"
              value={memberLimitDraft}
              onChange={(event) => setMemberLimitDraft(event.target.value)}
            />
            <p className={`app-form-footnote ${styles.footnote}`}>
              {formatUiMessage(locale, "shareCenter.memberLimitWorkspaceNote")}
            </p>
            <p className={`app-form-footnote ${styles.footnote}`}>
              {formatUiMessage(locale, "shareCenter.ownerPaysHint")}
            </p>
            <button
              className="app-button-secondary"
              data-testid="share-question-limits-save"
              disabled={
                questionLimitsMutation.isPending ||
                toggleShareMutation.isPending ||
                refreshShareMutation.isPending
              }
              type="button"
              onClick={() => void handleSaveQuestionLimits()}
            >
              {questionLimitsMutation.isPending
                ? formatUiMessage(locale, "shareCenter.saving")
                : formatUiMessage(locale, "shareCenter.questionLimitsSave")}
            </button>
          </div>
        ) : null}

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
          <div className={styles.shareUrl} data-testid="share-link">
            {shareUrl || formatUiMessage(locale, "shareCenter.controlBarNoLink")}
          </div>
        </div>
      </div>

      <div className={`app-button-row ${styles.actions}`}>
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
