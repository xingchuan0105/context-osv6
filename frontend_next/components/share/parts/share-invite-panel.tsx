"use client";

import { formatUiMessage } from "../../../lib/i18n/messages";
import { SectionHeader } from "./share-center-ui";
import styles from "./share-invite-panel.module.css";
import {
  formatAccessedAt,
  memberDisplayName,
  memberRoleLabel,
  memberStatusLabel,
} from "./share-center-utils";
import type { useShareCenter } from "./use-share-center";

type ShareCenter = ReturnType<typeof useShareCenter>;

export function ShareInvitePanel({ center }: { center: ShareCenter }) {
  const {
    handleConfirmRemove,
    handleInviteMember,
    inviteEmail,
    inviteError,
    inviteMemberMutation,
    inviteRole,
    locale,
    membersQuery,
    pendingRemoveMemberId,
    removeMemberMutation,
    setInviteEmail,
    setInviteRole,
    setPendingRemoveMemberId,
  } = center;

  return (
        <section
          className={`app-surface-card ${styles.panel}`}
          data-testid="share-invite-panel"
        >
          <SectionHeader
            subtitle={formatUiMessage(locale, "shareCenter.inviteSectionSubtitle")}
            title={formatUiMessage(locale, "shareCenter.inviteSectionTitle")}
          />

          <div
            className={`app-inline-surface ${styles.formCard}`}
          >
            <div className={styles.fieldStack}>
              <label className="app-form-label" htmlFor="invite-email">
                {locale === "zh-CN" ? "邀请邮箱" : "Invite email"}
              </label>
              <input
                className="app-input"
                data-testid="share-invite-email"
                id="invite-email"
                type="email"
                value={inviteEmail}
                onChange={(event) => setInviteEmail(event.target.value)}
              />
            </div>
            <div className={styles.fieldStack}>
              <label className="app-form-label" htmlFor="invite-role">
                {locale === "zh-CN" ? "邀请角色" : "Invite role"}
              </label>
              <select
                className="app-input"
                id="invite-role"
                value={inviteRole}
                onChange={(event) => setInviteRole(event.target.value as "viewer" | "editor")}
              >
                <option value="viewer">{memberRoleLabel(locale, "viewer")}</option>
                <option value="editor">{memberRoleLabel(locale, "editor")}</option>
              </select>
            </div>
            <button
              className={`app-button-primary ${styles.submitButton}`}
              data-testid="share-invite-send"
              disabled={inviteMemberMutation.isPending}
              type="button"
              onClick={() => void handleInviteMember()}
            >
              {inviteMemberMutation.isPending
                ? formatUiMessage(locale, "shareCenter.inviteSending")
                : locale === "zh-CN"
                  ? "发送邀请"
                  : "Send invite"}
            </button>
            {inviteError ? <p className="app-notice-banner">{inviteError}</p> : null}
          </div>

          {membersQuery.isLoading && !membersQuery.data ? (
            <p className={styles.flushText}>{formatUiMessage(locale, "shareCenter.loading")}</p>
          ) : membersQuery.error && !membersQuery.data ? (
            <p className="app-notice-banner">
              {membersQuery.error instanceof Error
                ? membersQuery.error.message
                : formatUiMessage(locale, "shareCenter.membersLoadError")}
            </p>
          ) : membersQuery.data && membersQuery.data.members.length > 0 ? (
            <div className={styles.memberList}>
              {membersQuery.data.members.map((member) => {
                const displayName = memberDisplayName(member);
                const confirming = pendingRemoveMemberId === member.member_id;

                return (
                  <article
                    className={`app-inline-surface ${styles.memberCard}`}
                    data-member-id={member.member_id}
                    data-testid="share-invite-member"
                    key={member.member_id}
                  >
                    <div className={styles.memberInfo}>
                      <strong>{displayName}</strong>
                      <span className={styles.memberMeta}>
                        {memberRoleLabel(locale, member.role)} · {memberStatusLabel(locale, member.status)}
                      </span>
                      <span className={styles.memberMetaSmall}>
                        {formatUiMessage(locale, "shareCenter.memberInvitedAt", {
                          value: formatAccessedAt(locale, member.invited_at),
                        })}
                      </span>
                    </div>
                    {confirming ? (
                      <div className={`app-button-row ${styles.confirmRow}`}>
                        <button
                          className="app-button-primary"
                          disabled={removeMemberMutation.isPending}
                          type="button"
                          onClick={() => void handleConfirmRemove(member.member_id)}
                        >
                          {formatUiMessage(locale, "shareCenter.confirmRemoveAction")}
                        </button>
                        <button
                          className="app-button-ghost"
                          disabled={removeMemberMutation.isPending}
                          type="button"
                          onClick={() => setPendingRemoveMemberId(null)}
                        >
                          {locale === "zh-CN" ? "取消" : "Cancel"}
                        </button>
                      </div>
                    ) : (
                      <button
                        className={`app-button-ghost ${styles.removeButton}`}
                        type="button"
                        onClick={() => setPendingRemoveMemberId(member.member_id)}
                      >
                        {formatUiMessage(locale, "shareCenter.removeAction")}
                      </button>
                    )}
                  </article>
                );
              })}
            </div>
          ) : (
            <p className={styles.emptyText}>
              {locale === "zh-CN" ? "暂无成员。" : "No members yet."}
            </p>
          )}
        </section>
  );
}
