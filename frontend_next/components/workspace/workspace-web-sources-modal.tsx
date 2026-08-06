"use client";

import { AppModal } from "../ui/app-modal";
import { formatUiMessage } from "../../lib/i18n/messages";
import { useUiPreferences } from "../../lib/ui-preferences";
import type { WorkspaceWebSourcesRequest } from "../../lib/workspace/model";
import styles from "./workspace-right-rail.module.css";

/**
 * Web sources list as a centered modal (same surface pattern as citations).
 * Replaces the previous right-rail takeover.
 */
export function WorkspaceWebSourcesModal({
  request,
  onClose,
}: {
  request: WorkspaceWebSourcesRequest | null;
  onClose: () => void;
}) {
  const { locale } = useUiPreferences();
  const open = Boolean(request && request.sources.length > 0);
  const count = request?.sources.length ?? 0;
  const title =
    count === 1
      ? formatUiMessage(locale, "workspaceSourcesCountOne")
      : formatUiMessage(locale, "workspaceSourcesCountMany", { count: String(count) });

  return (
    <AppModal
      open={open}
      size="md"
      title={title}
      closeLabel={formatUiMessage(locale, "commonCancel")}
      testId="workspace-web-sources-modal"
      onClose={onClose}
    >
      <div className={styles.webSourcesList} data-testid="workspace-web-sources-list">
        {(request?.sources ?? []).map((source, index) => (
          <div className={styles.webSourceCard} key={`${source.url}-${index}`}>
            <div className={styles.webSourceTitle}>
              <a
                className={styles.webSourceLink}
                href={source.url}
                rel="noreferrer"
                target="_blank"
              >
                {source.title || source.url}
              </a>
            </div>
            <div className={styles.webSourceUrl}>{source.url}</div>
            {source.snippet ? (
              <div className={styles.webSourceSnippet}>{source.snippet}</div>
            ) : null}
          </div>
        ))}
      </div>
    </AppModal>
  );
}
