"use client";

import { useMemo, useState } from "react";

import type { UiLocale } from "../../../lib/i18n/config";
import { formatDayLabel } from "./share-center-utils";
import styles from "./share-views-bar-chart.module.css";

export type ShareViewsBarPoint = {
  day: string;
  views: number;
};

type ShareViewsBarChartProps = {
  series: ShareViewsBarPoint[];
  locale: UiLocale;
  /** Chart height in CSS px (viewBox scales). */
  height?: number;
  emptyLabel: string;
  testId?: string;
};

/**
 * Vertical bar chart for share views (DeepSeek-style usage panels).
 * Pure SVG — no chart library dependency.
 */
export function ShareViewsBarChart({
  series,
  locale,
  height = 220,
  emptyLabel,
  testId = "share-views-bar-chart",
}: ShareViewsBarChartProps) {
  const [hover, setHover] = useState<number | null>(null);

  const { maxViews, hasData } = useMemo(() => {
    const max = Math.max(...series.map((p) => p.views), 0);
    return { maxViews: Math.max(max, 1), hasData: series.some((p) => p.views > 0) };
  }, [series]);

  if (!hasData) {
    return (
      <div className={styles.empty} data-testid={testId}>
        {emptyLabel}
      </div>
    );
  }

  const pad = { top: 16, right: 12, bottom: 28, left: 40 };
  const width = 640;
  const innerW = width - pad.left - pad.right;
  const innerH = height - pad.top - pad.bottom;
  const n = series.length;
  const gap = 0.28;
  const barSlot = innerW / Math.max(n, 1);
  const barW = Math.max(2, barSlot * (1 - gap));

  const yTicks = [0, 0.5, 1].map((t) => ({
    y: pad.top + innerH * (1 - t),
    label: String(Math.round(maxViews * t)),
  }));

  const hoverPoint = hover !== null ? series[hover] : null;

  return (
    <div className={styles.wrap} data-testid={testId}>
      <svg
        className={styles.svg}
        viewBox={`0 0 ${width} ${height}`}
        role="img"
        aria-label={locale === "zh-CN" ? "访问量柱状图" : "Views bar chart"}
      >
        {yTicks.map((t) => (
          <g key={t.label}>
            <line
              x1={pad.left}
              x2={width - pad.right}
              y1={t.y}
              y2={t.y}
              className={styles.grid}
            />
            <text
              x={pad.left - 8}
              y={t.y}
              className={styles.yLabel}
              textAnchor="end"
              dominantBaseline="middle"
            >
              {t.label}
            </text>
          </g>
        ))}
        {series.map((point, i) => {
          const h = (point.views / maxViews) * innerH;
          const x = pad.left + i * barSlot + (barSlot - barW) / 2;
          const y = pad.top + innerH - h;
          const active = hover === i;
          return (
            <g key={point.day}>
              <rect
                x={x}
                y={y}
                width={barW}
                height={Math.max(h, point.views > 0 ? 2 : 0)}
                className={active ? styles.barActive : styles.bar}
                rx={2}
                onMouseEnter={() => setHover(i)}
                onMouseLeave={() => setHover(null)}
              />
              {/* hit area */}
              <rect
                x={pad.left + i * barSlot}
                y={pad.top}
                width={barSlot}
                height={innerH}
                fill="transparent"
                onMouseEnter={() => setHover(i)}
                onMouseLeave={() => setHover(null)}
              />
              {(i === 0 || i === n - 1 || i % Math.max(1, Math.floor(n / 6)) === 0) && (
                <text
                  x={pad.left + i * barSlot + barSlot / 2}
                  y={height - 8}
                  className={styles.xLabel}
                  textAnchor="middle"
                >
                  {formatDayLabel(locale, point.day).replace(/^\d{4}-/, "")}
                </text>
              )}
            </g>
          );
        })}
      </svg>
      {hoverPoint ? (
        <div className={styles.tooltip} role="status">
          <strong>{formatDayLabel(locale, hoverPoint.day)}</strong>
          <span>
            {locale === "zh-CN" ? "访问" : "Views"} {hoverPoint.views}
          </span>
        </div>
      ) : null}
    </div>
  );
}
