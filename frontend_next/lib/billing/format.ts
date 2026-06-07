/** 100K / 1.5M / 200 这种紧凑数字格式 */
export function formatCompactToken(n: number): string {
  if (n >= 1_000_000) return `${(n / 1_000_000).toFixed(n >= 10_000_000 ? 0 : 1)}M`;
  if (n >= 1_000) return `${(n / 1_000).toFixed(n >= 100_000 ? 0 : 1)}K`;
  return n.toString();
}

/** "100,000" 这种千分位完整格式 */
export function formatFullToken(n: number): string {
  return n.toLocaleString("en-US");
}

/** 5h 23m 倒计时格式 */
export function formatCountdown(ms: number): string {
  if (ms <= 0) return "0m";
  const totalMin = Math.floor(ms / 60_000);
  const h = Math.floor(totalMin / 60);
  const m = totalMin % 60;
  if (h > 24) {
    const d = Math.floor(h / 24);
    const rh = h % 24;
    return rh > 0 ? `${d}d ${rh}h` : `${d}d`;
  }
  if (h > 0) return `${h}h ${m}m`;
  return `${m}m`;
}

/** 百分比保留 0 位小数 */
export function formatPct(pct: number): string {
  return `${Math.round(pct)}%`;
}
