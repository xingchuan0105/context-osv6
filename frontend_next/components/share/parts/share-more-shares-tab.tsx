"use client";

import Link from "next/link";
import { useEffect, useState } from "react";

import { formatUiMessage } from "../../../lib/i18n/messages";
import {
  getPublicOwnerProfile,
  type PublicOwnerShareItem,
} from "../../../lib/share/client";
import { useUiPreferences } from "../../../lib/ui-preferences";
import styles from "./share-tabs.module.css";

export type ShareMoreSharesTabProps = {
  userId: string;
  /** Current share token — excluded from the list. */
  currentShareToken: string;
};

/**
 * "More shares" tab: other public shared workspaces by the same owner.
 * Data comes from the public owner profile endpoint (no auth).
 */
export function ShareMoreSharesTab({ userId, currentShareToken }: ShareMoreSharesTabProps) {
  const { locale } = useUiPreferences();
  const [shares, setShares] = useState<PublicOwnerShareItem[] | null>(null);
  const [failed, setFailed] = useState(false);

  useEffect(() => {
    let cancelled = false;
    async function load() {
      try {
        const profile = await getPublicOwnerProfile(userId);
        if (!cancelled) {
          setShares(profile.shares.filter((s) => s.share_token !== currentShareToken));
        }
      } catch {
        if (!cancelled) {
          setFailed(true);
        }
      }
    }
    void load();
    return () => {
      cancelled = true;
    };
  }, [userId, currentShareToken]);

  return (
    <section className={styles.tabPane} data-testid="shared-more-shares-tab">
      {shares === null && !failed ? (
        <p className={styles.emptyState} role="status">
          {formatUiMessage(locale, "sharedPublic.moreSharesLoading")}
        </p>
      ) : null}
      {failed || (shares !== null && shares.length === 0) ? (
        <p className={styles.emptyState}>
          {formatUiMessage(locale, "sharedPublic.moreSharesEmpty")}
        </p>
      ) : null}
      {shares !== null && shares.length > 0 ? (
        <ul className={styles.shareCards}>
          {shares.map((share) => (
            <li className={styles.shareCard} key={share.workspace_id}>
              <div className={styles.shareCardMain}>
                <h3 className={styles.shareCardTitle}>{share.title}</h3>
                {share.description?.trim() ? (
                  <p className={styles.shareCardDesc}>{share.description.trim()}</p>
                ) : null}
                <span className={styles.shareCardMeta}>
                  {formatUiMessage(locale, "sharedPublic.profileSourcesMeta", {
                    count: String(share.source_count),
                  })}
                </span>
              </div>
              <Link
                className={styles.shareCardOpen}
                href={`/shared/kb/${encodeURIComponent(share.share_token)}`}
              >
                {formatUiMessage(locale, "sharedPublic.profileOpenWorkspace")}
              </Link>
            </li>
          ))}
        </ul>
      ) : null}
    </section>
  );
}
