"use client";

import Link from "next/link";

import { mediaSrc } from "../../../lib/http/request";
import { formatUiMessage } from "../../../lib/i18n/messages";
import type { ShareOwnerCard } from "../../../lib/share/client";
import { useUiPreferences } from "../../../lib/ui-preferences";
import styles from "./share-owner-hero.module.css";

export type ShareOwnerHeroProps = {
  owner: ShareOwnerCard | null | undefined;
  workspaceTitle: string;
  workspaceDescription?: string | null;
  sourceCount: number;
  allowDownload: boolean;
};

/**
 * Shared-page title bar: workspace identity on the left, owner as a compact
 * banner/profile card on the right. Avatar / name → public owner profile.
 */
export function ShareOwnerHero({
  owner,
  workspaceTitle,
  workspaceDescription,
  sourceCount,
  allowDownload,
}: ShareOwnerHeroProps) {
  const { locale } = useUiPreferences();
  const displayName =
    owner?.display_name?.trim() ||
    formatUiMessage(locale, "sharedPublic.ownerFallbackName");
  const bio = owner?.bio?.trim() || "";
  const contactUrl = owner?.contact_url?.trim() || "";
  const avatarUrl = mediaSrc(owner?.avatar_url);
  const bannerUrl = mediaSrc(owner?.banner_url);
  const description = workspaceDescription?.trim() || "";
  const initial = displayName.slice(0, 1).toUpperCase() || "·";
  const profileHref =
    owner?.user_id?.trim() && owner?.profile_enabled !== false
      ? `/shared/u/${owner.user_id.trim()}`
      : null;

  const avatarEl = (
    <div
      className={styles.avatar}
      style={avatarUrl ? { backgroundImage: `url(${avatarUrl})` } : undefined}
      data-testid="share-owner-avatar"
    >
      {!avatarUrl ? initial : null}
    </div>
  );

  return (
    <header className={styles.titleBar} data-testid="share-owner-card">
      <div className={styles.workspaceBlock}>
        <nav className={styles.crumb} aria-label={formatUiMessage(locale, "sharedPublic.pageTitle")}>
          <Link className={styles.backLink} href="/">
            {formatUiMessage(locale, "sharedPublic.backHomeAction")}
          </Link>
          <span className={styles.crumbDot} aria-hidden>
            ·
          </span>
          <span className={styles.crumbLabel}>
            {formatUiMessage(locale, "sharedPublic.pageTitle")}
          </span>
        </nav>

        <h1 className={styles.workspaceTitle}>{workspaceTitle}</h1>

        {description ? <p className={styles.workspaceDesc}>{description}</p> : null}

        <div className={styles.metaRow}>
          <span className={styles.metaChip}>
            {formatUiMessage(locale, "sharedPublic.sourcesCountChip", {
              count: String(sourceCount),
            })}
          </span>
          <span className={styles.metaChip}>
            {formatUiMessage(locale, "sharedPublic.modeRagChip")}
          </span>
          <span className={styles.metaChip}>
            {allowDownload
              ? formatUiMessage(locale, "sharedPublic.downloadAllowed")
              : formatUiMessage(locale, "sharedPublic.downloadOnlineOnly")}
          </span>
        </div>
      </div>

      <article
        className={styles.ownerCard}
        aria-label={formatUiMessage(locale, "sharedPublic.ownerCardLabel")}
      >
        <div
          className={styles.banner}
          style={bannerUrl ? { backgroundImage: `url(${bannerUrl})` } : undefined}
          data-testid="share-owner-banner"
          role="img"
          aria-label={formatUiMessage(locale, "sharedPublic.ownerBannerLabel")}
        />

        <div className={styles.ownerBody}>
          <div className={styles.ownerIdentity}>
            {profileHref ? (
              <Link
                className={styles.avatarLink}
                href={profileHref}
                title={formatUiMessage(locale, "sharedPublic.openOwnerProfile")}
                aria-label={formatUiMessage(locale, "sharedPublic.openOwnerProfile")}
              >
                {avatarEl}
              </Link>
            ) : (
              avatarEl
            )}

            <div className={styles.ownerText}>
              <div className={styles.nameRow}>
                {profileHref ? (
                  <Link className={styles.nameLink} href={profileHref}>
                    <span className={styles.displayName}>{displayName}</span>
                  </Link>
                ) : (
                  <span className={styles.displayName}>{displayName}</span>
                )}
                <span className={styles.rolePill}>
                  {formatUiMessage(locale, "sharedPublic.ownerRolePill")}
                </span>
              </div>
              {bio ? <p className={styles.bio}>{bio}</p> : null}
            </div>
          </div>

          {contactUrl ? (
            <a
              className={styles.contactButton}
              href={contactUrl}
              rel="noreferrer"
              target="_blank"
            >
              {formatUiMessage(locale, "sharedPublic.ownerContactAction")}
            </a>
          ) : null}
        </div>
      </article>
    </header>
  );
}
