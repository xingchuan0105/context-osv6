"use client";

import Link from "next/link";
import { useEffect, useState } from "react";

import { mediaSrc } from "../../lib/http/request";
import type { UiLocale } from "../../lib/i18n/config";
import { formatUiMessage } from "../../lib/i18n/messages";
import {
  getPublicOwnerProfile,
  type PublicOwnerProfile,
} from "../../lib/share/client";
import { useUiPreferences } from "../../lib/ui-preferences";
import styles from "./shared-owner-profile-surface.module.css";

/** access_level wire values ("partial"/"full") → localized labels; pass unknown values through. */
function accessLevelLabel(locale: UiLocale, accessLevel: string) {
  const key =
    accessLevel === "partial" || accessLevel === "full"
      ? (`sharedPublic.accessLevel.${accessLevel}` as const)
      : null;
  return key ? formatUiMessage(locale, key) : accessLevel;
}

export function SharedOwnerProfileSurface({ userId }: { userId: string }) {
  const { locale } = useUiPreferences();
  const [profile, setProfile] = useState<PublicOwnerProfile | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState("");

  useEffect(() => {
    let cancelled = false;
    async function load() {
      if (!userId.trim()) {
        setError("invalid");
        setLoading(false);
        return;
      }
      setLoading(true);
      setError("");
      try {
        const data = await getPublicOwnerProfile(userId);
        if (!cancelled) {
          setProfile(data);
        }
      } catch (err) {
        if (!cancelled) {
          setError(err instanceof Error ? err.message : "not_found");
          setProfile(null);
        }
      } finally {
        if (!cancelled) {
          setLoading(false);
        }
      }
    }
    void load();
    return () => {
      cancelled = true;
    };
  }, [userId]);

  if (loading) {
    return (
      <main className={styles.page} data-testid="shared-owner-profile">
        <p className={styles.status}>{formatUiMessage(locale, "sharedPublic.profileLoading")}</p>
      </main>
    );
  }

  if (error || !profile) {
    return (
      <main className={styles.page} data-testid="shared-owner-profile">
        <div className={styles.card}>
          <h1 className={styles.title}>
            {formatUiMessage(locale, "sharedPublic.profileNotFound")}
          </h1>
          <Link className={styles.backLink} href="/">
            {formatUiMessage(locale, "sharedPublic.profileBackToHome")}
          </Link>
        </div>
      </main>
    );
  }

  const { owner, shares } = profile;
  const displayName =
    owner.display_name?.trim() ||
    formatUiMessage(locale, "sharedPublic.ownerFallbackName");
  const avatarUrl = mediaSrc(owner.avatar_url);
  const bannerUrl = mediaSrc(owner.banner_url);
  const initial = displayName.slice(0, 1).toUpperCase() || "·";

  return (
    <main className={styles.page} data-testid="shared-owner-profile">
      <div className={styles.navRow}>
        <Link className={styles.backLink} href="/">
          {formatUiMessage(locale, "sharedPublic.profileBackToHome")}
        </Link>
        <span className={styles.navMuted}>·</span>
        <span className={styles.navMuted}>
          {formatUiMessage(locale, "sharedPublic.profilePageTitle")}
        </span>
      </div>

      <section className={styles.hero}>
        <div
          className={styles.banner}
          style={bannerUrl ? { backgroundImage: `url(${bannerUrl})` } : undefined}
        />
        <div className={styles.heroBody}>
          <div className={styles.avatarRow}>
            <div
              className={styles.avatar}
              style={avatarUrl ? { backgroundImage: `url(${avatarUrl})` } : undefined}
              aria-hidden
            >
              {!avatarUrl ? initial : null}
            </div>
            <div className={styles.identity}>
              <h1 className={styles.displayName}>{displayName}</h1>
              {owner.bio?.trim() ? <p className={styles.bio}>{owner.bio.trim()}</p> : null}
              {owner.contact_url?.trim() ? (
                <a
                  className={styles.contact}
                  href={owner.contact_url.trim()}
                  rel="noreferrer"
                  target="_blank"
                >
                  {formatUiMessage(locale, "sharedPublic.ownerContactAction")}
                </a>
              ) : null}
            </div>
          </div>
        </div>
      </section>

      <section className={styles.sharesSection}>
        <h2 className={styles.sharesTitle}>
          {formatUiMessage(locale, "sharedPublic.profileSharesTitle")}
          <span className={styles.sharesCount}>{shares.length}</span>
        </h2>

        {shares.length === 0 ? (
          <p className={styles.empty}>
            {formatUiMessage(locale, "sharedPublic.profileSharesEmpty")}
          </p>
        ) : (
          <ul className={styles.shareList}>
            {shares.map((share) => (
              <li className={styles.shareCard} key={share.workspace_id}>
                <div className={styles.shareMain}>
                  <h3 className={styles.shareTitle}>{share.title}</h3>
                  {share.description?.trim() ? (
                    <p className={styles.shareDesc}>{share.description.trim()}</p>
                  ) : null}
                  <div className={styles.shareMeta}>
                    <span>
                      {formatUiMessage(locale, "sharedPublic.profileSourcesMeta", {
                        count: String(share.source_count),
                      })}
                    </span>
                    <span>{accessLevelLabel(locale, share.access_level)}</span>
                  </div>
                </div>
                <Link
                  className={styles.openButton}
                  href={`/shared/kb/${encodeURIComponent(share.share_token)}`}
                >
                  {formatUiMessage(locale, "sharedPublic.profileOpenWorkspace")}
                </Link>
              </li>
            ))}
          </ul>
        )}
      </section>
    </main>
  );
}
