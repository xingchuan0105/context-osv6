"use client";

import { useState } from "react";

import { toSafeHttpUrl } from "../../lib/url/isSafeHttpUrl";
import {
  type ToolResult,
  ToolStatus,
} from "../../lib/workspace/stream";
import styles from "./workspace-chat.module.css";

type ToolResultCardProps = {
  locale: "zh-CN" | "en";
  result: ToolResult;
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

export function ToolResultCard({ locale, result }: ToolResultCardProps) {
  const [expanded, setExpanded] = useState(true);
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
        ? locale === "zh-CN"
          ? "错误"
          : "Error"
        : result.status === ToolStatus.Timeout
          ? locale === "zh-CN"
            ? "超时"
            : "Timeout"
          : result.status === ToolStatus.NotFound
            ? locale === "zh-CN"
              ? "未找到"
              : "Not Found"
            : result.status === ToolStatus.NotImplemented
              ? locale === "zh-CN"
                ? "未实现"
                : "Not Implemented"
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
                {locale === "zh-CN" ? "错误" : "Error"}
              </div>
              <pre>{String(data.error)}</pre>
            </div>
          ) : null}
          {execResult !== "" ? (
            <div className={styles.toolResultSection}>
              <div className={styles.toolResultSectionLabel}>
                {locale === "zh-CN" ? "返回值" : "Result"}
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
              <pre style={{ color: "hsl(0 84% 60%)" }}>{stderr}</pre>
            </div>
          ) : null}
          {!success && data.exit_code !== undefined ? (
            <div className={styles.toolResultSection}>
              <div className={styles.toolResultSectionLabel}>
                {locale === "zh-CN" ? "退出码" : "Exit Code"}
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
                {locale === "zh-CN" ? "表达式" : "Expression"}
              </div>
              <pre>{expression}</pre>
            </div>
          ) : null}
          {calcResult ? (
            <div className={styles.toolResultSection}>
              <div className={styles.toolResultSectionLabel}>
                {locale === "zh-CN" ? "结果" : "Result"}
              </div>
              <pre>{calcResult}</pre>
            </div>
          ) : null}
          {data.error ? (
            <div className={styles.toolResultSection}>
              <div className={styles.toolResultSectionLabel}>
                {locale === "zh-CN" ? "错误" : "Error"}
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
      const temperature = data.temperature !== undefined ? String(data.temperature) : "";
      const feelsLike = data.feels_like !== undefined ? String(data.feels_like) : "";
      const humidity = data.humidity !== undefined ? String(data.humidity) : "";
      const windSpeed = data.wind_speed !== undefined ? String(data.wind_speed) : "";
      const units = typeof data.units === "string" ? data.units : "";

      return (
        <div className={styles.toolResultBody}>
          {location || description ? (
            <div style={{ fontWeight: 600, marginBottom: "0.4rem" }}>
              {location}
              {location && description ? " — " : ""}
              {description}
            </div>
          ) : null}
          <div className={styles.toolResultWeatherGrid}>
            {temperature ? (
              <div className={styles.toolResultWeatherItem}>
                <span className={styles.toolResultWeatherLabel}>
                  {locale === "zh-CN" ? "温度" : "Temperature"}
                </span>
                <span className={styles.toolResultWeatherValue}>
                  {temperature}
                  {units === "metric" ? "°C" : units === "imperial" ? "°F" : ""}
                </span>
              </div>
            ) : null}
            {feelsLike ? (
              <div className={styles.toolResultWeatherItem}>
                <span className={styles.toolResultWeatherLabel}>
                  {locale === "zh-CN" ? "体感" : "Feels Like"}
                </span>
                <span className={styles.toolResultWeatherValue}>
                  {feelsLike}
                  {units === "metric" ? "°C" : units === "imperial" ? "°F" : ""}
                </span>
              </div>
            ) : null}
            {humidity ? (
              <div className={styles.toolResultWeatherItem}>
                <span className={styles.toolResultWeatherLabel}>
                  {locale === "zh-CN" ? "湿度" : "Humidity"}
                </span>
                <span className={styles.toolResultWeatherValue}>{humidity}%</span>
              </div>
            ) : null}
            {windSpeed ? (
              <div className={styles.toolResultWeatherItem}>
                <span className={styles.toolResultWeatherLabel}>
                  {locale === "zh-CN" ? "风速" : "Wind Speed"}
                </span>
                <span className={styles.toolResultWeatherValue}>
                  {windSpeed}
                  {units === "metric" ? " m/s" : units === "imperial" ? " mph" : ""}
                </span>
              </div>
            ) : null}
          </div>
          {data.error ? (
            <div className={styles.toolResultSection}>
              <div className={styles.toolResultSectionLabel}>
                {locale === "zh-CN" ? "错误" : "Error"}
              </div>
              <pre>{String(data.error)}</pre>
            </div>
          ) : null}
        </div>
      );
    }

    if (renderHint === "search") {
      const results = Array.isArray(data.results) ? data.results : [];
      const answer = typeof data.synthesized_answer === "string" ? data.synthesized_answer : "";

      return (
        <div className={styles.toolResultBody}>
          {answer ? (
            <div className={styles.toolResultSection}>
              <div className={styles.toolResultSectionLabel}>
                {locale === "zh-CN" ? "摘要" : "Summary"}
              </div>
              <div style={{ lineHeight: 1.5 }}>{answer}</div>
            </div>
          ) : null}
          {results.length > 0 ? (
            <div className={styles.toolResultSection}>
              <div className={styles.toolResultSectionLabel}>
                {locale === "zh-CN" ? "搜索结果" : "Search Results"}
              </div>
              <div style={{ display: "flex", flexDirection: "column", gap: "0.5rem" }}>
                {results.map((r: any, i: number) => {
                  const safeUrl = toSafeHttpUrl(typeof r.url === "string" ? r.url : null);
                  return (
                  <div
                    key={i}
                    style={{
                      padding: "0.4rem 0.5rem",
                      borderRadius: "6px",
                      background: "hsl(var(--muted) / 0.15)",
                    }}
                  >
                    {safeUrl ? (
                      <a
                        href={safeUrl}
                        target="_blank"
                        rel="noopener noreferrer"
                        style={{
                          fontWeight: 600,
                          fontSize: "0.85rem",
                          color: "hsl(217 91% 60%)",
                          textDecoration: "none",
                        }}
                      >
                        {typeof r.title === "string" ? r.title : safeUrl}
                      </a>
                    ) : (
                      <div style={{ fontWeight: 600, fontSize: "0.85rem" }}>
                        {typeof r.title === "string" ? r.title : ""}
                      </div>
                    )}
                    {typeof r.snippet === "string" && r.snippet ? (
                      <div
                        style={{
                          fontSize: "0.78rem",
                          color: "hsl(var(--muted-foreground))",
                          marginTop: "0.15rem",
                        }}
                      >
                        {r.snippet}
                      </div>
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
                {locale === "zh-CN" ? "错误" : "Error"}
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
      ? locale === "zh-CN"
        ? "代码执行"
        : "Code Execution"
      : renderHint === "calculator"
        ? locale === "zh-CN"
          ? "计算器"
          : "Calculator"
        : renderHint === "weather"
          ? locale === "zh-CN"
            ? "天气查询"
            : "Weather"
          : renderHint === "search"
            ? locale === "zh-CN"
              ? "网页搜索"
              : "Web Search"
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
        <span style={{ fontSize: "0.75rem", color: "hsl(var(--muted-foreground))" }}>
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

export function ToolResultsPanel({ locale, results }: ToolResultsPanelProps) {
  if (results.length === 0) {
    return null;
  }
  return (
    <div className={styles.toolResultsPanel}>
      {results.map((result, index) => (
        <ToolResultCard key={`${result.tool}-${index}`} locale={locale} result={result} />
      ))}
    </div>
  );
}

