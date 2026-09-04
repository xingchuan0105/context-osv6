import { expect, type CDPSession, type Page } from "@playwright/test";
import { mkdirSync, writeFileSync } from "node:fs";
import { dirname } from "node:path";
import { seedNextAuth } from "./auth-seed";

export const FIXTURE_MARKER = "退化成早已演练过的常规操作。";
export const COLD_N = Number(process.env.GATE0_COLD_N || 5);
export const HOT_N = Number(process.env.GATE0_HOT_N || 20);

/** 97 chunks × 15ms。首绘与收束若挤在同一帧，不是本夹具的增量流。 */
const MIN_INCREMENTAL_MS = 800;
/** 整篇夹具约 3000 字。首绘已接近整篇才视为积压上屏或上一轮残字。 */
const MAX_FIRST_PAINT_CHARS = 2000;
const STREAM_ATTEMPTS = 3;

export type Side = "rust" | "next";

export type StreamSample = {
  kind: "cold" | "hot";
  stream_first_token_ms: number;
  stream_complete_ms: number;
  heap_used_after_stream: number | null;
  long_frame_count: number | null;
  first_paint_chars?: number | null;
};

export type DiscardedSample = {
  kind: "cold" | "hot";
  reason: string;
  stream_first_token_ms: number;
  stream_complete_ms: number;
  first_paint_chars?: number | null;
};

export type ColdNavSample = {
  nav_lcp_ms: number | null;
  encoded_transfer_bytes: number | null;
};

export type SideResult = {
  side: Side;
  blocked?: string;
  build_note: string;
  cold_nav: ColdNavSample[];
  streams: StreamSample[];
  discarded: DiscardedSample[];
};

type Gate0Marks = {
  start: number;
  first: number | null;
  complete: number | null;
  firstLen: number | null;
};

export function percentile(values: number[], p: number): number | null {
  if (values.length === 0) return null;
  const sorted = [...values].sort((a, b) => a - b);
  const idx = Math.max(0, Math.ceil((sorted.length * p) / 100) - 1);
  return sorted[idx] ?? sorted[sorted.length - 1];
}

export function summarize(values: number[]) {
  return {
    n: values.length,
    min: values.length ? Math.min(...values) : null,
    max: values.length ? Math.max(...values) : null,
    median: percentile(values, 50),
    p95: percentile(values, 95),
  };
}

export function writeResult(path: string, payload: unknown) {
  mkdirSync(dirname(path), { recursive: true });
  writeFileSync(path, `${JSON.stringify(payload, null, 2)}\n`, "utf8");
}

export const STRESS_MS = Number(process.env.GATE0_STRESS_MS || 30 * 60 * 1000);
export const STRESS_EVERY_MS = Number(process.env.GATE0_STRESS_EVERY_MS || 30_000);
/** 后 10 分钟斜率低于此值（字节/分钟）视为平台噪声，不判无界增长。 */
const HEAP_SLOPE_NOISE_BYTES_PER_MIN = 50_000;

export type HeapSample = {
  i: number;
  t_ms: number;
  heap_cdp: number | null;
  heap_perf: number | null;
};

export type HeapSlopeJudgement = {
  n: number;
  last10_n: number;
  slope_bytes_per_min: number | null;
  last10_min: number | null;
  last10_max: number | null;
  last10_range_ratio: number | null;
  plateau: boolean;
  unbounded: boolean;
};

export function linearSlopePerMinute(samples: Array<{ t_ms: number; heap: number }>): number | null {
  if (samples.length < 2) return null;
  const xs = samples.map((s) => s.t_ms / 60_000);
  const ys = samples.map((s) => s.heap);
  const n = xs.length;
  const xMean = xs.reduce((a, b) => a + b, 0) / n;
  const yMean = ys.reduce((a, b) => a + b, 0) / n;
  let num = 0;
  let den = 0;
  for (let i = 0; i < n; i += 1) {
    const dx = xs[i] - xMean;
    num += dx * (ys[i] - yMean);
    den += dx * dx;
  }
  return den === 0 ? 0 : num / den;
}

export function judgeHeapSlope(samples: HeapSample[]): HeapSlopeJudgement {
  const withHeap = samples
    .filter((s): s is HeapSample & { heap_cdp: number } => s.heap_cdp != null)
    .map((s) => ({ t_ms: s.t_ms, heap: s.heap_cdp }));
  const lastT = withHeap.length ? withHeap[withHeap.length - 1].t_ms : 0;
  const last10 = withHeap.filter((s) => s.t_ms >= lastT - 10 * 60_000);
  const slope = linearSlopePerMinute(last10);
  const heaps = last10.map((s) => s.heap);
  const min = heaps.length ? Math.min(...heaps) : null;
  const max = heaps.length ? Math.max(...heaps) : null;
  const mean = heaps.length ? heaps.reduce((a, b) => a + b, 0) / heaps.length : 0;
  const rangeRatio = min != null && max != null && mean > 0 ? (max - min) / mean : null;
  const plateau =
    (rangeRatio != null && rangeRatio < 0.05) ||
    (slope != null && Math.abs(slope) < HEAP_SLOPE_NOISE_BYTES_PER_MIN);
  const unbounded = slope != null && slope > HEAP_SLOPE_NOISE_BYTES_PER_MIN && !plateau;
  return {
    n: withHeap.length,
    last10_n: last10.length,
    slope_bytes_per_min: slope,
    last10_min: min,
    last10_max: max,
    last10_range_ratio: rangeRatio,
    plateau,
    unbounded,
  };
}

export async function attachCdpHeap(page: Page): Promise<CDPSession> {
  return page.context().newCDPSession(page);
}

export async function readHeapCdp(client: CDPSession): Promise<number | null> {
  try {
    const usage = (await client.send("Runtime.getHeapUsage")) as {
      usedSize?: number;
    };
    if (typeof usage.usedSize === "number") {
      return usage.usedSize;
    }
  } catch {
    // Chromium 无此方法时走 Performance 指标
  }
  try {
    const metrics = (await client.send("Performance.getMetrics")) as {
      metrics?: Array<{ name: string; value: number }>;
    };
    const row = metrics.metrics?.find((item) => item.name === "JSHeapUsedSize");
    return row ? row.value : null;
  } catch {
    return null;
  }
}

export function streamSampleIssue(sample: StreamSample): string | null {
  if (!Number.isFinite(sample.stream_first_token_ms) || sample.stream_first_token_ms < 0) {
    return "negative_or_invalid_first_token";
  }
  if (!Number.isFinite(sample.stream_complete_ms)) {
    return "invalid_complete";
  }
  if (sample.stream_complete_ms < sample.stream_first_token_ms) {
    return "complete_before_first";
  }
  if (sample.stream_complete_ms - sample.stream_first_token_ms < MIN_INCREMENTAL_MS) {
    return "not_incremental";
  }
  if (sample.first_paint_chars != null && sample.first_paint_chars > MAX_FIRST_PAINT_CHARS) {
    return "first_paint_too_large";
  }
  return null;
}

export async function rustGotoChat(page: Page, apiBase: string, path = "/chat") {
  await seedNextAuth(page, "gate0-fixture-token");
  await page.addInitScript((base) => {
    (window as unknown as { __POC_CHAT_API_BASE__: string }).__POC_CHAT_API_BASE__ = base;
  }, apiBase);
  await page.goto(path, { waitUntil: "domcontentloaded" });
  await expect(page.getByTestId("chat-canvas")).toBeVisible();
  await rustAwaitIdle(page);
}

export async function rustAwaitIdle(page: Page) {
  await expect(page.getByTestId("chat-canvas")).toBeVisible();
  await expect(page.getByTestId("live-answer")).toHaveCount(0);
  await expect(page.getByTestId("chat-empty")).toBeVisible();
  await expect(page.getByTestId("status-line")).toHaveText("");
  await expect(page.getByTestId("send-button")).toBeEnabled();
}

export async function rustNewChat(page: Page) {
  const button = page.getByTestId("new-chat-button");
  if (await button.count()) {
    await button.click({ noWaitAfter: true });
  }
  await expect(page).toHaveURL(/\/chat\/?$/);
  await rustAwaitIdle(page);
}

export async function rustSendAndMeasure(page: Page): Promise<StreamSample> {
  await rustAwaitIdle(page);
  await page.getByTestId("composer-input").fill("写一篇 3000 字以上的流式系统说明");
  await startLongFrameCounter(page);
  await installStreamMarks(page);
  await page.getByTestId("send-button").evaluate((el) => {
    const w = window as unknown as { __gate0_marks: Gate0Marks };
    w.__gate0_marks.start = performance.now();
    (el as HTMLButtonElement).click();
  });
  await page.waitForFunction(() => {
    const w = window as unknown as { __gate0_marks?: Gate0Marks };
    return w.__gate0_marks != null && w.__gate0_marks.complete != null;
  }, undefined, { timeout: 90_000 });
  const marks = await readStreamMarks(page);
  const first = marks.first != null && marks.start >= 0 ? marks.first - marks.start : Number.NaN;
  const complete =
    marks.complete != null && marks.start >= 0 ? marks.complete - marks.start : Number.NaN;
  return {
    kind: "hot",
    stream_first_token_ms: first,
    stream_complete_ms: complete,
    heap_used_after_stream: await readHeap(page),
    long_frame_count: await stopLongFrameCounter(page),
    first_paint_chars: marks.firstLen,
  };
}

export async function rustCollectStream(
  page: Page,
  kind: "cold" | "hot",
  discarded: DiscardedSample[],
): Promise<StreamSample> {
  let last = "";
  for (let attempt = 0; attempt < STREAM_ATTEMPTS; attempt += 1) {
    if (attempt > 0) {
      await rustNewChat(page);
    }
    const sample = await rustSendAndMeasure(page);
    sample.kind = kind;
    const reason = streamSampleIssue(sample);
    if (!reason) {
      return sample;
    }
    discarded.push({
      kind,
      reason,
      stream_first_token_ms: sample.stream_first_token_ms,
      stream_complete_ms: sample.stream_complete_ms,
      first_paint_chars: sample.first_paint_chars,
    });
    last = `${reason} first=${sample.stream_first_token_ms} complete=${sample.stream_complete_ms} chars=${sample.first_paint_chars}`;
  }
  throw new Error(`Gate 0 ${kind} stream invalid after ${STREAM_ATTEMPTS} attempts: ${last}`);
}

export async function measureColdNav(page: Page): Promise<ColdNavSample> {
  return page.evaluate(() => {
    const nav = performance.getEntriesByType("navigation")[0] as
      | PerformanceNavigationTiming
      | undefined;
    let transfer = nav?.transferSize ?? 0;
    for (const resource of performance.getEntriesByType("resource") as PerformanceResourceTiming[]) {
      transfer += resource.transferSize || 0;
    }
    const paints = performance.getEntriesByType("largest-contentful-paint") as { startTime: number }[];
    const lcp = paints.length ? paints[paints.length - 1].startTime : null;
    return {
      nav_lcp_ms: lcp,
      encoded_transfer_bytes: transfer || null,
    };
  });
}

export async function installLcpObserver(page: Page) {
  await page.addInitScript(() => {
    const w = window as unknown as { __gate0_lcp?: number };
    try {
      const observer = new PerformanceObserver((list) => {
        const entries = list.getEntries();
        const last = entries[entries.length - 1];
        if (last) w.__gate0_lcp = last.startTime;
      });
      observer.observe({ type: "largest-contentful-paint", buffered: true } as PerformanceObserverInit);
    } catch {
      // older engines
    }
  });
}

export async function readObservedLcp(page: Page): Promise<number | null> {
  return page.evaluate(() => {
    const w = window as unknown as { __gate0_lcp?: number };
    return typeof w.__gate0_lcp === "number" ? w.__gate0_lcp : null;
  });
}

async function installStreamMarks(page: Page) {
  await page.evaluate((marker) => {
    const w = window as unknown as {
      __gate0_marks: Gate0Marks;
      __gate0_obs?: MutationObserver;
      __gate0_mark_raf?: number;
    };
    if (w.__gate0_obs) w.__gate0_obs.disconnect();
    if (typeof w.__gate0_mark_raf === "number") cancelAnimationFrame(w.__gate0_mark_raf);
    w.__gate0_marks = { start: -1, first: null, complete: null, firstLen: null };
    const tick = () => {
      if (w.__gate0_marks.start < 0) return;
      const live = document.querySelector("[data-testid='live-answer']");
      const status = document.querySelector("[data-testid='status-line']");
      const text = live?.textContent ?? "";
      const now = performance.now();
      if (w.__gate0_marks.first == null && text.length > 0) {
        w.__gate0_marks.first = now;
        w.__gate0_marks.firstLen = text.length;
      }
      if (
        w.__gate0_marks.complete == null &&
        text.includes(marker) &&
        (status?.textContent ?? "") === "已完成"
      ) {
        w.__gate0_marks.complete = now;
      }
    };
    const obs = new MutationObserver(tick);
    obs.observe(document.body, { subtree: true, childList: true, characterData: true });
    w.__gate0_obs = obs;
    const poll = () => {
      tick();
      if (w.__gate0_marks.complete == null) {
        w.__gate0_mark_raf = requestAnimationFrame(poll);
      }
    };
    w.__gate0_mark_raf = requestAnimationFrame(poll);
  }, FIXTURE_MARKER);
}

async function installNextStreamMarks(page: Page) {
  await page.evaluate((marker) => {
    const w = window as unknown as {
      __gate0_marks: Gate0Marks;
      __gate0_obs?: MutationObserver;
      __gate0_mark_raf?: number;
    };
    if (w.__gate0_obs) w.__gate0_obs.disconnect();
    if (typeof w.__gate0_mark_raf === "number") cancelAnimationFrame(w.__gate0_mark_raf);
    w.__gate0_marks = { start: -1, first: null, complete: null, firstLen: null };
    const tick = () => {
      if (w.__gate0_marks.start < 0) return;
      const nodes = document.querySelectorAll(
        '[data-testid="chat-message"][data-role="assistant"]',
      );
      const last = nodes[nodes.length - 1];
      const text = last?.textContent ?? "";
      const pending = last?.getAttribute("data-pending");
      const now = performance.now();
      if (w.__gate0_marks.first == null && text.length > 0) {
        w.__gate0_marks.first = now;
        w.__gate0_marks.firstLen = text.length;
      }
      if (
        w.__gate0_marks.complete == null &&
        text.includes(marker) &&
        pending !== "true"
      ) {
        w.__gate0_marks.complete = now;
      }
    };
    const obs = new MutationObserver(tick);
    obs.observe(document.body, { subtree: true, childList: true, characterData: true });
    w.__gate0_obs = obs;
    const poll = () => {
      tick();
      if (w.__gate0_marks.complete == null) {
        w.__gate0_mark_raf = requestAnimationFrame(poll);
      }
    };
    w.__gate0_mark_raf = requestAnimationFrame(poll);
  }, FIXTURE_MARKER);
}

async function readStreamMarks(page: Page): Promise<Gate0Marks> {
  return page.evaluate(() => {
    const w = window as unknown as {
      __gate0_marks?: Gate0Marks;
      __gate0_obs?: MutationObserver;
      __gate0_mark_raf?: number;
    };
    if (w.__gate0_obs) w.__gate0_obs.disconnect();
    if (typeof w.__gate0_mark_raf === "number") cancelAnimationFrame(w.__gate0_mark_raf);
    return (
      w.__gate0_marks ?? {
        start: -1,
        first: null,
        complete: null,
        firstLen: null,
      }
    );
  });
}

async function readHeap(page: Page): Promise<number | null> {
  return page.evaluate(() => {
    const mem = (performance as unknown as { memory?: { usedJSHeapSize: number } }).memory;
    return mem ? mem.usedJSHeapSize : null;
  });
}

async function startLongFrameCounter(page: Page) {
  await page.evaluate(() => {
    const w = window as unknown as {
      __gate0_long_frames?: number;
      __gate0_raf?: number;
      __gate0_last_raf?: number;
    };
    w.__gate0_long_frames = 0;
    w.__gate0_last_raf = performance.now();
    const tick = (now: number) => {
      if (w.__gate0_last_raf != null && now - w.__gate0_last_raf > 50) {
        w.__gate0_long_frames = (w.__gate0_long_frames || 0) + 1;
      }
      w.__gate0_last_raf = now;
      w.__gate0_raf = requestAnimationFrame(tick);
    };
    w.__gate0_raf = requestAnimationFrame(tick);
  });
}

async function stopLongFrameCounter(page: Page): Promise<number | null> {
  return page.evaluate(() => {
    const w = window as unknown as {
      __gate0_long_frames?: number;
      __gate0_raf?: number;
    };
    if (typeof w.__gate0_raf === "number") cancelAnimationFrame(w.__gate0_raf);
    return typeof w.__gate0_long_frames === "number" ? w.__gate0_long_frames : null;
  });
}

export type Gate0Auth = {
  token: string;
  user: { id: string; email: string; full_name: string };
};

const LEGAL_TERMS_VERSION = "2026-06-13";
const LEGAL_PRIVACY_VERSION = "2026-06-13";

export async function loginGate0Api(apiBase: string): Promise<Gate0Auth> {
  const { request } = await import("@playwright/test");
  const ctx = await request.newContext({ baseURL: apiBase });
  try {
    const email = process.env.E2E_TEST_USER_EMAIL || "e2e-test@example.com";
    const password = process.env.E2E_TEST_USER_PASSWORD || "E2eTest123!";
    const login = await ctx.post("/api/auth/login", {
      data: { email, password },
      timeout: 30_000,
    });
    if (!login.ok()) {
      throw new Error(`login failed: ${login.status()}`);
    }
    const body = (await login.json()) as {
      success?: boolean;
      data?: Gate0Auth | null;
    };
    if (!body.success || !body.data?.token || !body.data.user?.id) {
      throw new Error("login response missing token/user");
    }
    const accepted = await ctx.post("/api/auth/legal-acceptance", {
      headers: { Authorization: `Bearer ${body.data.token}` },
      data: {
        terms_version: LEGAL_TERMS_VERSION,
        privacy_version: LEGAL_PRIVACY_VERSION,
        context: "re_acceptance",
      },
      timeout: 15_000,
    });
    if (!accepted.ok()) {
      throw new Error(`legal-acceptance failed: ${accepted.status()}`);
    }
    return body.data;
  } finally {
    await ctx.dispose();
  }
}

export async function wireNextChat(
  page: Page,
  opts: { apiBase: string; fixtureBase: string; auth: Gate0Auth },
) {
  const persisted = { token: opts.auth.token, user: opts.auth.user };
  await page.addInitScript(
    ({ payload, fixtureChat }) => {
      window.localStorage.setItem("avrag.auth.v1", JSON.stringify(payload));
      const encoded = encodeURIComponent(JSON.stringify(payload));
      document.cookie = "avrag.auth.session=1; Path=/; SameSite=Lax; Max-Age=31536000";
      document.cookie = `avrag.auth.persisted=${encoded}; Path=/; SameSite=Lax; Max-Age=31536000`;
      const originalFetch = window.fetch.bind(window);
      window.fetch = (input: RequestInfo | URL, init?: RequestInit) => {
        const url =
          typeof input === "string"
            ? input
            : input instanceof URL
              ? input.toString()
              : input.url;
        const method = (init?.method || (input instanceof Request ? input.method : "GET")).toUpperCase();
        if (method === "POST" && /\/api\/v1\/chat\/?$/.test(new URL(url, location.origin).pathname)) {
          return originalFetch(fixtureChat, init);
        }
        return originalFetch(input, init);
      };
    },
    { payload: persisted, fixtureChat: `${opts.fixtureBase}/api/v1/chat` },
  );

  const fixtureOrigin = new URL(opts.fixtureBase).origin;
  const apiOrigin = new URL(opts.apiBase).origin;
  await page.route("**/api/**", async (route) => {
    const req = route.request();
    const url = new URL(req.url());
    if (url.origin === fixtureOrigin || url.origin === apiOrigin) {
      await route.continue();
      return;
    }
    if (req.method() === "GET" && /\/api\/v1\/workspaces\/?$/.test(url.pathname)) {
      await route.fulfill({
        status: 200,
        contentType: "application/json",
        body: JSON.stringify({ workspaces: [] }),
      });
      return;
    }
    if (req.method() === "GET" && /\/api\/v1\/chat\/sessions\/?$/.test(url.pathname)) {
      await route.fulfill({
        status: 200,
        contentType: "application/json",
        body: JSON.stringify({ sessions: [] }),
      });
      return;
    }
    if (req.method() === "GET" && /\/api\/v1\/chat\/sessions\/sess-900$/.test(url.pathname)) {
      await route.fulfill({
        status: 200,
        contentType: "application/json",
        body: JSON.stringify({
          id: "sess-900",
          owner_user_id: "fixture-user",
          scope_kind: "personal",
          agent_type: "chat",
          model_role: "quick_chat",
          created_at: "2026-09-04T00:00:00Z",
          updated_at: "2026-09-04T00:00:00Z",
        }),
      });
      return;
    }
    if (req.method() === "GET" && url.pathname.endsWith("/sessions/sess-900/messages")) {
      await route.fulfill({
        status: 200,
        contentType: "application/json",
        body: JSON.stringify({ messages: [] }),
      });
      return;
    }
    if (req.method() === "GET" && url.pathname.endsWith("/sessions/sess-900/files")) {
      await route.fulfill({
        status: 200,
        contentType: "application/json",
        body: JSON.stringify({ files: [] }),
      });
      return;
    }
    const isChatPost =
      req.method() === "POST" &&
      (url.pathname === "/api/v1/chat" || url.pathname.endsWith("/api/v1/chat"));
    if (isChatPost) {
      await route.continue();
      return;
    }
    const dest = `${opts.apiBase}${url.pathname}${url.search}`;
    const headers = { ...req.headers() };
    delete headers.host;
    try {
      const response = await page.request.fetch(dest, {
        method: req.method(),
        headers,
        data: req.postDataBuffer() ?? undefined,
        timeout: 30_000,
        failOnStatusCode: false,
      });
      const respHeaders = { ...response.headers() };
      delete respHeaders["content-encoding"];
      delete respHeaders["content-length"];
      await route.fulfill({
        status: response.status(),
        headers: respHeaders,
        body: await response.body(),
      });
    } catch {
      if (!page.isClosed()) {
        await route.abort("failed");
      }
    }
  });
}

export async function nextGotoChatHome(page: Page, origin: string) {
  await page.goto(`${origin}/chat`, { waitUntil: "domcontentloaded" });
  await expect(page.getByTestId("workspace-chat-composer")).toBeVisible({ timeout: 20_000 });
}

export async function nextGotoChat(page: Page, origin: string) {
  // 夹具 session。/chat 首发会 replace 到 /chat/:id 并 abort SSE（产品行为），
  // 流式计时必须在已落地的 session 路由上采，否则 complete 永远等不到。
  await page.goto(`${origin}/chat/sess-900`, { waitUntil: "domcontentloaded" });
  await nextAwaitIdle(page);
}

export async function nextAwaitIdle(page: Page) {
  const composer = page.getByTestId("workspace-chat-composer");
  await expect(composer).toBeVisible({ timeout: 20_000 });
  await expect(composer).toBeEnabled({ timeout: 20_000 });
  await expect(page.locator('[data-testid="chat-message"][data-role="assistant"]')).toHaveCount(0);
}

export async function nextNewChat(page: Page, origin: string) {
  await nextGotoChat(page, origin);
}

export async function nextSendAndMeasure(page: Page): Promise<StreamSample> {
  await nextAwaitIdle(page);
  const composer = page.getByTestId("workspace-chat-composer");
  await composer.evaluate((el, value) => {
    const textarea = el as HTMLTextAreaElement;
    const setter = Object.getOwnPropertyDescriptor(
      window.HTMLTextAreaElement.prototype,
      "value",
    )?.set;
    setter?.call(textarea, value);
    textarea.dispatchEvent(new Event("input", { bubbles: true }));
    textarea.dispatchEvent(new Event("change", { bubbles: true }));
  }, "写一篇 3000 字以上的流式系统说明");
  await expect(page.getByTestId("workspace-chat-send")).toBeEnabled();
  await startLongFrameCounter(page);
  await installNextStreamMarks(page);
  await page.getByTestId("workspace-chat-send").evaluate((el) => {
    const w = window as unknown as { __gate0_marks: Gate0Marks };
    w.__gate0_marks.start = performance.now();
    (el as HTMLButtonElement).click();
  });
  await page.waitForFunction(() => {
    const w = window as unknown as { __gate0_marks?: Gate0Marks };
    return w.__gate0_marks != null && w.__gate0_marks.complete != null;
  }, undefined, { timeout: 90_000 });
  const marks = await readStreamMarks(page);
  const first = marks.first != null && marks.start >= 0 ? marks.first - marks.start : Number.NaN;
  const complete =
    marks.complete != null && marks.start >= 0 ? marks.complete - marks.start : Number.NaN;
  return {
    kind: "hot",
    stream_first_token_ms: first,
    stream_complete_ms: complete,
    heap_used_after_stream: await readHeap(page),
    long_frame_count: await stopLongFrameCounter(page),
    first_paint_chars: marks.firstLen,
  };
}

export async function nextCollectStream(
  page: Page,
  origin: string,
  kind: "cold" | "hot",
  discarded: DiscardedSample[],
): Promise<StreamSample> {
  let last = "";
  for (let attempt = 0; attempt < STREAM_ATTEMPTS; attempt += 1) {
    if (attempt > 0) {
      await nextNewChat(page, origin);
    }
    const sample = await nextSendAndMeasure(page);
    sample.kind = kind;
    const reason = streamSampleIssue(sample);
    if (!reason) {
      return sample;
    }
    discarded.push({
      kind,
      reason,
      stream_first_token_ms: sample.stream_first_token_ms,
      stream_complete_ms: sample.stream_complete_ms,
      first_paint_chars: sample.first_paint_chars,
    });
    last = `${reason} first=${sample.stream_first_token_ms} complete=${sample.stream_complete_ms} chars=${sample.first_paint_chars}`;
  }
  throw new Error(`Gate 0 ${kind} stream invalid after ${STREAM_ATTEMPTS} attempts: ${last}`);
}
