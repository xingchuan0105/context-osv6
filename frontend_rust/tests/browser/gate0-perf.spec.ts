import { test, expect } from "@playwright/test";
import { join } from "node:path";
import {
  COLD_N,
  HOT_N,
  type ColdNavSample,
  type SideResult,
  installLcpObserver,
  measureColdNav,
  nextCollectStream,
  nextGotoChat,
  nextGotoChatHome,
  nextNewChat,
  readObservedLcp,
  rustGotoChat,
  rustCollectStream,
  rustNewChat,
  rustSendAndMeasure,
  loginGate0Api,
  wireNextChat,
  summarize,
  writeResult,
  STRESS_EVERY_MS,
  STRESS_MS,
  attachCdpHeap,
  judgeHeapSlope,
  readHeapCdp,
  type HeapSample,
} from "./gate0-harness";

const WEB_BASE = `http://127.0.0.1:${Number(process.env.POC_WEB_PORT || 3200)}`;
const FIXTURE_BASE = `http://127.0.0.1:${Number(process.env.POC_FIXTURE_PORT || 3201)}`;
const NEXT_BASE = (process.env.GATE0_NEXT_BASE || "").replace(/\/$/, "");
const API_BASE = (process.env.GATE0_API_BASE || "").replace(/\/$/, "");
const RUST_PROFILE = process.env.GATE0_RUST_PROFILE === "release" ? "release" : "debug";
const RESULT_PATH = join(
  process.cwd(),
  "results",
  `gate0-${new Date().toISOString().replace(/[:.]/g, "-")}.json`,
);

function pack(side: SideResult) {
  const first = side.streams.map((s) => s.stream_first_token_ms);
  const complete = side.streams.map((s) => s.stream_complete_ms);
  const heap = side.streams
    .map((s) => s.heap_used_after_stream)
    .filter((v): v is number => v != null);
  const longFrames = side.streams
    .map((s) => s.long_frame_count)
    .filter((v): v is number => v != null);
  const lcp = side.cold_nav
    .map((s) => s.nav_lcp_ms)
    .filter((v): v is number => v != null);
  const transfer = side.cold_nav
    .map((s) => s.encoded_transfer_bytes)
    .filter((v): v is number => v != null);
  return {
    ...side,
    summary: {
      stream_first_token_ms: summarize(first),
      stream_complete_ms: summarize(complete),
      heap_used_after_stream: summarize(heap),
      long_frame_count: summarize(longFrames),
      nav_lcp_ms: summarize(lcp),
      encoded_transfer_bytes: summarize(transfer),
    },
  };
}

test.describe.configure({ mode: "serial" });

test("Gate 0 Rust PoC 5 cold + 20 hot", async ({ browser }) => {
  test.skip(process.env.GATE0 !== "1", "set GATE0=1 to collect");
  const rust: SideResult = {
    side: "rust",
    build_note: `target/${RUST_PROFILE}/web-server + target/site (${RUST_PROFILE} hydrate)`,
    cold_nav: [],
    streams: [],
    discarded: [],
  };

  for (let i = 0; i < COLD_N; i += 1) {
    const page = await browser.newPage();
    await installLcpObserver(page);
    await rustGotoChat(page, FIXTURE_BASE);
    await page.waitForTimeout(400);
    const nav = await measureColdNav(page);
    const observed = await readObservedLcp(page);
    rust.cold_nav.push({
      nav_lcp_ms: observed ?? nav.nav_lcp_ms,
      encoded_transfer_bytes: nav.encoded_transfer_bytes,
    });
    const stream = await rustCollectStream(page, "cold", rust.discarded);
    rust.streams.push(stream);
    await page.close();
  }

  const hotPage = await browser.newPage();
  await rustGotoChat(hotPage, FIXTURE_BASE);
  for (let i = 0; i < HOT_N; i += 1) {
    if (i > 0) {
      await rustNewChat(hotPage);
    }
    const stream = await rustCollectStream(hotPage, "hot", rust.discarded);
    rust.streams.push(stream);
  }
  await hotPage.close();

  expect(rust.cold_nav).toHaveLength(COLD_N);
  expect(rust.streams.filter((s) => s.kind === "cold")).toHaveLength(COLD_N);
  expect(rust.streams.filter((s) => s.kind === "hot")).toHaveLength(HOT_N);

  const next: SideResult = {
    side: "next",
    build_note: NEXT_BASE
      ? `GATE0_NEXT_BASE=${NEXT_BASE} GATE0_API_BASE=${API_BASE || "unset"}`
      : "unset",
    cold_nav: [],
    streams: [],
    discarded: [],
  };

  if (!NEXT_BASE) {
    next.blocked = "GATE0_NEXT_BASE unset";
  } else if (!API_BASE) {
    next.blocked = "GATE0_API_BASE unset (Next on :8080 does not proxy /api)";
  } else {
    try {
      await collectNext(browser, next);
    } catch (error) {
      const message = error instanceof Error ? error.message : String(error);
      next.blocked = message.replace(/\u001b\[[0-9;]*m/g, "").slice(0, 500);
    }
  }

  const payload = {
    charter: "docs/plans/2026-09-04-frontend-rust-phase-0-gate0-benchmark-charter.md",
    collected_at: new Date().toISOString(),
    rust_profile: RUST_PROFILE,
    rust_web: WEB_BASE,
    fixture: FIXTURE_BASE,
    next_web: NEXT_BASE || null,
    api: API_BASE || null,
    rust: pack(rust),
    next: pack(next),
  };
  writeResult(RESULT_PATH, payload);
  console.log(`gate0 result written ${RESULT_PATH}`);
});

test("Gate 0 30-minute heap slope (opt-in)", async ({ browser }) => {
  test.skip(process.env.GATE0_STRESS !== "1", "set GATE0_STRESS=1 for the 30-minute sample");
  test.setTimeout(Math.max(STRESS_MS + 120_000, 120_000));

  const page = await browser.newPage();
  await rustGotoChat(page, FIXTURE_BASE);
  const cdp = await attachCdpHeap(page);
  try {
    await cdp.send("Performance.enable").catch(() => undefined);
  } catch {
    // optional
  }

  const started = Date.now();
  const samples: HeapSample[] = [];
  const errors: string[] = [];
  let i = 0;
  while (Date.now() - started < STRESS_MS) {
    const tickStart = Date.now();
    try {
      if (i > 0) {
        await rustNewChat(page);
      }
      await rustSendAndMeasure(page);
    } catch (error) {
      errors.push(error instanceof Error ? error.message.slice(0, 200) : String(error));
    }
    const heapCdp = await readHeapCdp(cdp);
    const heapPerf = await page.evaluate(() => {
      const mem = (performance as unknown as { memory?: { usedJSHeapSize: number } }).memory;
      return mem ? mem.usedJSHeapSize : null;
    });
    samples.push({
      i,
      t_ms: Date.now() - started,
      heap_cdp: heapCdp,
      heap_perf: heapPerf,
    });
    i += 1;
    const wait = STRESS_EVERY_MS - (Date.now() - tickStart);
    if (wait > 0 && Date.now() - started < STRESS_MS) {
      await page.waitForTimeout(wait);
    }
  }

  const judgement = judgeHeapSlope(samples);
  const stressPath = join(
    process.cwd(),
    "results",
    `gate0-stress-${new Date().toISOString().replace(/[:.]/g, "-")}.json`,
  );
  writeResult(stressPath, {
    charter: "docs/plans/2026-09-04-frontend-rust-phase-0-gate0-benchmark-charter.md",
    collected_at: new Date().toISOString(),
    rust_profile: RUST_PROFILE,
    duration_ms: Date.now() - started,
    interval_ms: STRESS_EVERY_MS,
    heap_source: "CDP Runtime.getHeapUsage (performance.memory recorded only)",
    judgement,
    errors,
    samples,
  });
  console.log(`gate0 stress written ${stressPath}`);
  await page.close();
  expect(samples.length, "need at least two heap samples").toBeGreaterThan(1);
  expect(judgement.unbounded, "last 10 min CDP heap slope unbounded").toBe(false);
});

async function collectNext(
  browser: import("@playwright/test").Browser,
  next: SideResult,
) {
  const auth = await loginGate0Api(API_BASE);
  for (let i = 0; i < COLD_N; i += 1) {
    const page = await browser.newPage();
    await installLcpObserver(page);
    await wireNextChat(page, { apiBase: API_BASE, fixtureBase: FIXTURE_BASE, auth });
    await nextGotoChatHome(page, NEXT_BASE);
    await page.waitForTimeout(400);
    const nav = await measureColdNav(page);
    const observed = await readObservedLcp(page);
    next.cold_nav.push({
      nav_lcp_ms: observed ?? nav.nav_lcp_ms,
      encoded_transfer_bytes: nav.encoded_transfer_bytes,
    } satisfies ColdNavSample);
    await nextGotoChat(page, NEXT_BASE);
    const stream = await nextCollectStream(page, NEXT_BASE, "cold", next.discarded);
    next.streams.push(stream);
    await page.unrouteAll({ behavior: "ignoreErrors" });
    await page.close();
  }

  const hotPage = await browser.newPage();
  await wireNextChat(hotPage, { apiBase: API_BASE, fixtureBase: FIXTURE_BASE, auth });
  await nextGotoChat(hotPage, NEXT_BASE);
  for (let i = 0; i < HOT_N; i += 1) {
    if (i > 0) {
      await nextNewChat(hotPage, NEXT_BASE);
    }
    const stream = await nextCollectStream(hotPage, NEXT_BASE, "hot", next.discarded);
    next.streams.push(stream);
  }
  await hotPage.unrouteAll({ behavior: "ignoreErrors" });
  await hotPage.close();
}
