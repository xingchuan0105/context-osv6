"use client";

import { useMemo } from "react";
import styles from "./UsageTrendChart.module.css";
import { formatCompactToken } from "../../lib/billing/format";
import type { DailyUsage } from "../../lib/billing/api";

export type UsageTrendChartProps = {
  daily: DailyUsage[];
  width?: number;
  height?: number;
};

export function UsageTrendChart({ daily, width = 600, height = 200 }: UsageTrendChartProps) {
  const padding = { top: 16, right: 16, bottom: 28, left: 48 };
  const innerW = width - padding.left - padding.right;
  const innerH = height - padding.top - padding.bottom;

  const { points, maxV, yTicks } = useMemo(() => {
    const maxValue = Math.max(...daily.map((d) => d.tokens), 1);
    const stepX = daily.length > 1 ? innerW / (daily.length - 1) : 0;
    const pointList = daily.map((d, i) => {
      const x = padding.left + i * stepX;
      const y = padding.top + innerH - (d.tokens / maxValue) * innerH;
      return `${x.toFixed(1)},${y.toFixed(1)}`;
    });
    const ticks = [0, 0.5, 1].map((t) => ({
      y: padding.top + innerH * (1 - t),
      label: formatCompactToken(Math.round(maxValue * t)),
    }));
    return { points: pointList, maxV: maxValue, yTicks: ticks };
  }, [daily, innerW, innerH, padding.left, padding.top]);

  if (daily.length === 0) {
    return <div className={styles.empty}>暂无用量数据</div>;
  }

  return (
    <svg className={styles.chart} viewBox={`0 0 ${width} ${height}`} role="img" aria-label="近 N 日用量趋势">
      {yTicks.map((t, i) => (
        <g key={i}>
          <line
            x1={padding.left}
            x2={width - padding.right}
            y1={t.y}
            y2={t.y}
            className={styles.grid}
          />
          <text
            x={padding.left - 6}
            y={t.y}
            className={styles.yLabel}
            textAnchor="end"
            dominantBaseline="middle"
          >
            {t.label}
          </text>
        </g>
      ))}
      <polyline points={points.join(" ")} className={styles.line} fill="none" />
      {daily.map((d, i) => {
        const x = padding.left + (daily.length > 1 ? i * (innerW / (daily.length - 1)) : innerW / 2);
        const y = padding.top + innerH - (d.tokens / maxV) * innerH;
        return (
          <g key={d.date}>
            <circle cx={x} cy={y} r={3} className={styles.dot} />
            {i % Math.max(1, Math.floor(daily.length / 7)) === 0 && (
              <text x={x} y={height - 8} className={styles.xLabel} textAnchor="middle">
                {d.date.slice(5)}
              </text>
            )}
          </g>
        );
      })}
    </svg>
  );
}
