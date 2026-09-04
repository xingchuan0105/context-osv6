import { expect, type Page } from "@playwright/test";
import { mkdirSync, writeFileSync } from "node:fs";
import { dirname } from "node:path";

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
  await page.addInitScript((base) => {
    (window as unknown as { __POC_CHAT_API_BASE__: string }).__POC_CHAT_API_BASE__ = base;
  }, apiBase);
  await page.goto(path, { waitUntil: "domcontentloaded" });
  await expect(page.getByTestId("chat-canvas")).toBeVisible();
  const token = page.getByTestId("poc-token-input");
  if (await token.count()) {
    await page.locator(".poc-token summary").click();
    await token.fill("gate0-fixture-token");
  }
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

export async function stubNextChatToFixture(page: Page, fixtureBase: string) {
  const auth = {
    token: "gate0-fixture",
    user: {
      id: "gate0-user",
      email: "e2e-test@example.com",
      full_name: "Gate0 Fixture",
    },
    reset_ticket: null,
  };
  await page.context().addCookies([
    {
      name: "avrag.auth.session",
      value: "1",
      domain: "127.0.0.1",
      path: "/",
    },
    {
      name: "avrag.auth.persisted",
      value: encodeURIComponent(JSON.stringify(auth)),
      domain: "127.0.0.1",
      path: "/",
    },
  ]);
  await page.addInitScript((payload) => {
    window.localStorage.setItem("avrag.auth.v1", JSON.stringify(payload));
    document.cookie = "avrag.auth.session=1; path=/";
    document.cookie = `avrag.auth.persisted=${encodeURIComponent(JSON.stringify(payload))}; path=/`;
  }, auth);

  await page.route("**/api/auth/me", async (route) => {
    await route.fulfill({
      status: 200,
      contentType: "application/json",
      body: JSON.stringify({
        success: true,
        data: auth,
        error: null,
      }),
    });
  });
  await page.route("**/api/v1/chat/sessions**", async (route) => {
    if (route.request().method() === "GET") {
      await route.fulfill({
        status: 200,
        contentType: "application/json",
        body: JSON.stringify({ sessions: [] }),
      });
      return;
    }
    await route.fallback();
  });
  await page.route("**/api/v1/chat", async (route) => {
    if (route.request().method() !== "POST") {
      await route.fallback();
      return;
    }
    await route.continue({ url: `${fixtureBase}/api/v1/chat` });
  });
}

export async function nextGotoChat(page: Page, origin: string) {
  await page.goto(`${origin}/chat`, { waitUntil: "domcontentloaded" });
  await expect(page.getByTestId("workspace-chat-composer")).toBeVisible({ timeout: 20_000 });
}

export async function nextSendAndMeasure(page: Page): Promise<StreamSample> {
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
  const start = Date.now();
  await startLongFrameCounter(page);
  await page.getByTestId("workspace-chat-send").click();
  const assistant = page.locator('[data-testid="chat-message"][data-role="assistant"]').last();
  await expect(assistant).toBeVisible({ timeout: 30_000 });
  await expect(assistant).not.toHaveText("", { timeout: 30_000 });
  const first = Date.now() - start;
  await expect(assistant).toContainText(FIXTURE_MARKER, { timeout: 90_000 });
  const pending = page.locator(
    '[data-testid="chat-message"][data-role="assistant"][data-pending="false"]',
  );
  if ((await pending.count()) > 0) {
    await expect(pending.last()).toBeVisible({ timeout: 15_000 });
  }
  const complete = Date.now() - start;
  return {
    kind: "hot",
    stream_first_token_ms: first,
    stream_complete_ms: complete,
    heap_used_after_stream: await readHeap(page),
    long_frame_count: await stopLongFrameCounter(page),
  };
}
