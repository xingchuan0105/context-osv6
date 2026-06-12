"use client";

import { formatSettingsShareMessage } from "../../../lib/settings-share-messages";
import { SectionHeader } from "./share-center-ui";
import { formatAccessedAt, parseAccessedAt } from "./share-center-utils";
import type { useShareCenter } from "./use-share-center";

type ShareCenter = ReturnType<typeof useShareCenter>;

export function ShareActivityPanel({ center }: { center: ShareCenter }) {
  const { accessLogsQuery, locale } = center;

  return (
    <>
      {accessLogsQuery.isLoading && !accessLogsQuery.data ? (
          <section className="app-surface-card" id="activity" style={{ scrollMarginTop: "6rem" }}>
            <p style={{ margin: 0 }}>
              {formatSettingsShareMessage(locale, "shareCenter.accessLogsLoading")}
            </p>
          </section>
        ) : accessLogsQuery.error && !accessLogsQuery.data ? (
          <section className="app-surface-card" id="activity" style={{ scrollMarginTop: "6rem" }}>
            <p className="app-notice-banner" style={{ margin: 0 }}>
              {accessLogsQuery.error instanceof Error
                ? accessLogsQuery.error.message
                : formatSettingsShareMessage(locale, "shareCenter.accessLogsLoadError")}
            </p>
          </section>
        ) : (
          <section
            className="app-surface-card"
            id="activity"
            style={{
              background:
                "linear-gradient(180deg, hsl(var(--background)) 0%, hsl(var(--muted) / 0.24) 100%)",
              display: "grid",
              gap: "0.95rem",
              padding: "0.95rem 1rem 1rem",
              scrollMarginTop: "6rem",
            }}
          >
            <SectionHeader
              subtitle={formatSettingsShareMessage(locale, "shareCenter.activitySectionSubtitle")}
              title={formatSettingsShareMessage(locale, "shareCenter.activitySectionTitle")}
            />

            {accessLogsQuery.data && accessLogsQuery.data.logs.length > 0 ? (
              <div style={{ display: "grid", gap: "0.75rem" }}>
                {accessLogsQuery.data.logs
                  .slice()
                  .sort((left, right) => {
                    const leftTime = parseAccessedAt(left.accessed_at) ?? 0;
                    const rightTime = parseAccessedAt(right.accessed_at) ?? 0;
                    return rightTime - leftTime;
                  })
                  .slice(0, 10)
                  .map((log) => (
                    <div
                      className="app-inline-surface"
                      key={log.id}
                      style={{
                        borderLeft: "3px solid hsl(var(--primary) / 0.24)",
                        display: "grid",
                        gap: "0.6rem",
                        gridTemplateColumns: "minmax(0, 1fr)",
                        padding: "0.72rem 0.82rem 0.78rem",
                      }}
                    >
                      <div style={{ display: "grid", gap: "0.2rem" }}>
                        <span style={{ color: "hsl(var(--muted-foreground))" }}>
                          {formatSettingsShareMessage(locale, "shareCenter.activityActionLabel")}
                        </span>
                        <strong>{log.action}</strong>
                      </div>
                      <div style={{ display: "grid", gap: "0.2rem" }}>
                        <span style={{ color: "hsl(var(--muted-foreground))" }}>
                          {formatSettingsShareMessage(locale, "shareCenter.activityTimeLabel")}
                        </span>
                        <span>{formatAccessedAt(locale, log.accessed_at)}</span>
                      </div>
                    </div>
                  ))}
              </div>
            ) : (
              <div className="app-inline-surface" style={{ display: "grid", gap: "0.25rem" }}>
                <strong>{formatSettingsShareMessage(locale, "shareCenter.activityEmptyTitle")}</strong>
                <p style={{ margin: 0, color: "hsl(var(--muted-foreground))" }}>
                  {formatSettingsShareMessage(locale, "shareCenter.activityEmptyBody")}
                </p>
              </div>
            )}
          </section>
        )}
    </>
  );
}
