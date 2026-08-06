"use client";

import { useState } from "react";

import { toSafeHttpUrl } from "../../lib/url/isSafeHttpUrl";
import { formatUiMessage } from "../../lib/i18n/messages";
import {
  type ToolResult,
  ToolStatus,
} from "../../lib/workspace/stream";
import styles from "./workspace-chat.module.css";

type ToolResultCardProps = {
  locale: "zh-CN" | "en";
  result: ToolResult;
};

type SearchResultRow = {
  title?: unknown;
  url?: unknown;
  snippet?: unknown;
};

const TOOL_RENDER_HINTS: Record<string, string> = {
  calculator: "calculator",
  code_interpreter: "code",
  weather_query: "weather",
  web_search: "search",
};

function getToolRenderHint(toolName: string): string {
  return TOOL_RENDER_HINTS[toolName] ?? "json";
}

function isCompactToolByDefault(toolName: string): boolean {
  // Product rule: only web_search is shown (and folded). Other user-facing tools expand.
  return toolName === "web_search" || getToolRenderHint(toolName) === "search";
}

export function ToolResultCard({ locale, result }: ToolResultCardProps) {
  const [expanded, setExpanded] = useState(() => !isCompactToolByDefault(result.tool));
  const data = (result.data ?? {}) as Record<string, unknown>;
  const isError = result.status === ToolStatus.Error;
  const isOk = result.status === ToolStatus.Ok;
  const renderHint = getToolRenderHint(result.tool);

  const statusClass = isOk
    ? styles.toolResultStatusOk
    : isError
      ? styles.toolResultStatusError
      : styles.toolResultStatusOther;

  const statusLabel =
    result.status === ToolStatus.Ok
      ? "OK"
      : result.status === ToolStatus.Error
        ? formatUiMessage(locale, "workspaceToolStatusError")
        : result.status === ToolStatus.Timeout
          ? formatUiMessage(locale, "workspaceToolStatusTimeout")
          : result.status === ToolStatus.NotFound
            ? formatUiMessage(locale, "workspaceToolStatusNotFound")
            : result.status === ToolStatus.NotImplemented
              ? formatUiMessage(locale, "workspaceToolStatusNotImplemented")
              : result.status;

  function renderBody() {
    if (renderHint === "code") {
      const stdout = typeof data.stdout === "string" ? data.stdout : "";
      const stderr = typeof data.stderr === "string" ? data.stderr : "";
      const execResult = data.result ?? "";
      const success = data.success === true;

      return (
        <div className={styles.toolResultBody}>
          {data.error ? (
            <div className={styles.toolResultSection}>
              <div className={styles.toolResultSectionLabel}>
                {formatUiMessage(locale, "workspaceToolStatusError")}
              </div>
              <pre>{String(data.error)}</pre>
            </div>
          ) : null}
          {execResult !== "" ? (
            <div className={styles.toolResultSection}>
              <div className={styles.toolResultSectionLabel}>
                {formatUiMessage(locale, "workspaceToolResult")}
              </div>
              <pre>{typeof execResult === "string" ? execResult : JSON.stringify(execResult, null, 2)}</pre>
            </div>
          ) : null}
          {stdout ? (
            <div className={styles.toolResultSection}>
              <div className={styles.toolResultSectionLabel}>stdout</div>
              <pre>{stdout}</pre>
            </div>
          ) : null}
          {stderr ? (
            <div className={styles.toolResultSection}>
              <div className={styles.toolResultSectionLabel}>stderr</div>
              <pre className={styles.toolResultStderr}>{stderr}</pre>
            </div>
          ) : null}
          {!success && data.exit_code !== undefined ? (
            <div className={styles.toolResultSection}>
              <div className={styles.toolResultSectionLabel}>
                {formatUiMessage(locale, "workspaceToolExitCode")}
              </div>
              <pre>{String(data.exit_code)}</pre>
            </div>
          ) : null}
        </div>
      );
    }

    if (renderHint === "calculator") {
      const expression = typeof data.expression === "string" ? data.expression : "";
      const calcResult = data.result !== undefined ? String(data.result) : "";

      return (
        <div className={styles.toolResultBody}>
          {expression ? (
            <div className={styles.toolResultSection}>
              <div className={styles.toolResultSectionLabel}>
                {formatUiMessage(locale, "workspaceToolExpression")}
              </div>
              <pre>{expression}</pre>
            </div>
          ) : null}
          {calcResult ? (
            <div className={styles.toolResultSection}>
              <div className={styles.toolResultSectionLabel}>
                {formatUiMessage(locale, "workspaceToolResult")}
              </div>
              <pre>{calcResult}</pre>
            </div>
          ) : null}
          {data.error ? (
            <div className={styles.toolResultSection}>
              <div className={styles.toolResultSectionLabel}>
                {formatUiMessage(locale, "workspaceToolStatusError")}
              </div>
              <pre>{String(data.error)}</pre>
            </div>
          ) : null}
        </div>
      );
    }

    if (renderHint === "weather") {
      const location = typeof data.location === "string" ? data.location : "";
      const description = typeof data.description === "string" ? data.description : "";
      const temperature = data.temperature !== undefined && data.temperature !== null
        ? String(data.temperature)
        : "";
      const feelsLike = data.feels_like !== undefined && data.feels_like !== null
        ? String(data.feels_like)
        : "";
      const humidity = data.humidity !== undefined && data.humidity !== null
        ? String(data.humidity)
        : "";
      const windSpeed = data.wind_speed !== undefined && data.wind_speed !== null
        ? String(data.wind_speed)
        : "";
      const units = typeof data.units === "string" ? data.units : "";
      const windUnit =
        typeof data.wind_speed_unit === "string"
          ? data.wind_speed_unit
          : units === "°F" || units === "imperial"
            ? "mph"
            : "km/h";
      const unitSuffix =
        units === "°C" || units === "metric"
          ? "°C"
          : units === "°F" || units === "imperial"
            ? "°F"
            : units;
      const daily = Array.isArray(data.daily) ? data.daily : [];
      const air =
        data.air && typeof data.air === "object"
          ? (data.air as Record<string, unknown>)
          : null;
      const aqi = air && air.aqi !== undefined && air.aqi !== null ? String(air.aqi) : "";
      const aqiCat =
        air && typeof air.category === "string" ? air.category : "";

      return (
        <div className={styles.toolResultBody}>
          {location || description ? (
            <div className={styles.toolResultWeatherHeader}>
              {location}
              {location && description ? " — " : ""}
              {description}
            </div>
          ) : null}
          <div className={styles.toolResultWeatherGrid}>
            {temperature ? (
              <div className={styles.toolResultWeatherItem}>
                <span className={styles.toolResultWeatherLabel}>
                  {formatUiMessage(locale, "workspaceToolTemperature")}
                </span>
                <span className={styles.toolResultWeatherValue}>
                  {temperature}
                  {unitSuffix}
                </span>
              </div>
            ) : null}
            {feelsLike ? (
              <div className={styles.toolResultWeatherItem}>
                <span className={styles.toolResultWeatherLabel}>
                  {formatUiMessage(locale, "workspaceToolFeelsLike")}
                </span>
                <span className={styles.toolResultWeatherValue}>
                  {feelsLike}
                  {unitSuffix}
                </span>
              </div>
            ) : null}
            {humidity ? (
              <div className={styles.toolResultWeatherItem}>
                <span className={styles.toolResultWeatherLabel}>
                  {formatUiMessage(locale, "workspaceToolHumidity")}
                </span>
                <span className={styles.toolResultWeatherValue}>{humidity}%</span>
              </div>
            ) : null}
            {windSpeed ? (
              <div className={styles.toolResultWeatherItem}>
                <span className={styles.toolResultWeatherLabel}>
                  {formatUiMessage(locale, "workspaceToolWindSpeed")}
                </span>
                <span className={styles.toolResultWeatherValue}>
                  {windSpeed} {windUnit}
                </span>
              </div>
            ) : null}
            {aqi ? (
              <div className={styles.toolResultWeatherItem}>
                <span className={styles.toolResultWeatherLabel}>AQI</span>
                <span className={styles.toolResultWeatherValue}>
                  {aqi}
                  {aqiCat ? ` (${aqiCat})` : ""}
                </span>
              </div>
            ) : null}
          </div>
          {daily.length > 0 ? (
            <div className={styles.toolResultSection}>
              <div className={styles.toolResultSectionLabel}>
                {formatUiMessage(locale, "workspaceToolForecast")}
              </div>
              <pre>
                {daily
                  .slice(0, 7)
                  .map((row) => {
                    const d = (row ?? {}) as Record<string, unknown>;
                    const date = d.date !== undefined ? String(d.date) : "";
                    const tmin = d.temp_min !== undefined ? String(d.temp_min) : "?";
                    const tmax = d.temp_max !== undefined ? String(d.temp_max) : "?";
                    const text =
                      typeof d.text_day === "string"
                        ? d.text_day
                        : typeof d.description === "string"
                          ? d.description
                          : "";
                    return `${date}  ${tmin}~${tmax}${unitSuffix}  ${text}`.trim();
                  })
                  .join("\n")}
              </pre>
            </div>
          ) : null}
          {data.error ? (
            <div className={styles.toolResultSection}>
              <div className={styles.toolResultSectionLabel}>
                {formatUiMessage(locale, "workspaceToolStatusError")}
              </div>
              <pre>{String(data.error)}</pre>
            </div>
          ) : null}
        </div>
      );
    }

    if (renderHint === "search") {
      const results: SearchResultRow[] = Array.isArray(data.results)
        ? (data.results as SearchResultRow[])
        : [];
      const answer = typeof data.synthesized_answer === "string" ? data.synthesized_answer : "";

      return (
        <div className={styles.toolResultBody}>
          {answer ? (
            <div className={styles.toolResultSection}>
              <div className={styles.toolResultSectionLabel}>
                {formatUiMessage(locale, "workspaceToolSummary")}
              </div>
              <div className={styles.toolResultAnswer}>{answer}</div>
            </div>
          ) : null}
          {results.length > 0 ? (
            <div className={styles.toolResultSection}>
              <div className={styles.toolResultSectionLabel}>
                {formatUiMessage(locale, "workspaceToolSearchResults")}
              </div>
              <div className={styles.toolResultSearchList}>
                {results.map((r: SearchResultRow, i: number) => {
                  const safeUrl = toSafeHttpUrl(typeof r.url === "string" ? r.url : null);
                  return (
                  <div key={i} className={styles.toolResultSearchRow}>
                    {safeUrl ? (
                      <a
                        className={styles.toolResultSearchLink}
                        href={safeUrl}
                        target="_blank"
                        rel="noopener noreferrer"
                      >
                        {typeof r.title === "string" ? r.title : safeUrl}
                      </a>
                    ) : (
                      <div className={styles.toolResultSearchTitle}>
                        {typeof r.title === "string" ? r.title : ""}
                      </div>
                    )}
                    {typeof r.snippet === "string" && r.snippet ? (
                      <div className={styles.toolResultSearchSnippet}>{r.snippet}</div>
                    ) : null}
                  </div>
                  );
                })}
              </div>
            </div>
          ) : null}
          {data.error ? (
            <div className={styles.toolResultSection}>
              <div className={styles.toolResultSectionLabel}>
                {formatUiMessage(locale, "workspaceToolStatusError")}
              </div>
              <pre>{String(data.error)}</pre>
            </div>
          ) : null}
        </div>
      );
    }

    // Generic fallback: render data as JSON
    return (
      <div className={styles.toolResultBody}>
        <pre>{JSON.stringify(data, null, 2)}</pre>
      </div>
    );
  }

  const toolLabel =
    renderHint === "code"
      ? formatUiMessage(locale, "workspaceToolCodeExecution")
      : renderHint === "calculator"
        ? formatUiMessage(locale, "workspaceToolCalculator")
        : renderHint === "weather"
          ? formatUiMessage(locale, "workspaceToolWeather")
          : renderHint === "search"
            ? formatUiMessage(locale, "workspaceToolWebSearch")
            : result.tool;

  return (
    <div className={styles.toolResultCard}>
      <button
        className={styles.toolResultHeader}
        onClick={() => setExpanded((prev) => !prev)}
        type="button"
      >
        <span className={styles.toolResultTitle}>
          {toolLabel}
          <span className={[styles.toolResultStatus, statusClass].join(" ")}>{statusLabel}</span>
        </span>
        <span className={styles.toolResultChevron} aria-hidden="true">
          {expanded ? "▾" : "▸"}
        </span>
      </button>
      {expanded ? renderBody() : null}
    </div>
  );
}

type ToolResultsPanelProps = {
  locale: "zh-CN" | "en";
  results: ToolResult[];
};

export function ToolResultsPanel({ locale: _locale, results: _results }: ToolResultsPanelProps) {
  // Product rule (2026-07-13): never surface tool-call records in any of the 4 modes.
  // Retrieval/search process belongs in ProgressTimeline; final answer + citations only.
  return null;
}

