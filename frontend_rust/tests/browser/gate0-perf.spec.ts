import { test, expect } from "@playwright/test";
import { join } from "node:path";
import {
  COLD_N,
  HOT_N,
  type ColdNavSample,
  type SideResult,
  installLcpObserver,
  measureColdNav,
  nextGotoChat,
  nextSendAndMeasure,
  readObservedLcp,
  rustGotoChat,
  rustCollectStream,
  rustNewChat,
  stubNextChatToFixture,
  summarize,
  writeResult,
} from "./gate0-harness";

const WEB_BASE = `http://127.0.0.1:${Number(process.env.POC_WEB_PORT || 3200)}`;
const FIXTURE_BASE = `http://127.0.0.1:${Number(process.env.POC_FIXTURE_PORT || 3201)}`;
const NEXT_BASE = (process.env.GATE0_NEXT_BASE || "").replace(/\/$/, "");
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
    build_note: "target/debug/web-server + target/site (debug hydrate)",
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
    build_note: NEXT_BASE ? `GATE0_NEXT_BASE=${NEXT_BASE}` : "unset",
    cold_nav: [],
    streams: [],
    discarded: [],
  };

  if (!NEXT_BASE) {
    next.blocked = "GATE0_NEXT_BASE unset";
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
    rust_web: WEB_BASE,
    fixture: FIXTURE_BASE,
    rust: pack(rust),
    next: pack(next),
  };
  writeResult(RESULT_PATH, payload);
  console.log(`gate0 result written ${RESULT_PATH}`);
});

test("Gate 0 30-minute heap slope (opt-in)", async () => {
  test.skip(process.env.GATE0_STRESS !== "1", "set GATE0_STRESS=1 for the 30-minute sample");
  test.fail(true, "30-minute collector not implemented in this slice");
});

async function collectNext(
  browser: import("@playwright/test").Browser,
  next: SideResult,
) {
  for (let i = 0; i < COLD_N; i += 1) {
    const page = await browser.newPage();
    await installLcpObserver(page);
    await stubNextChatToFixture(page, FIXTURE_BASE);
    await nextGotoChat(page, NEXT_BASE);
    await page.waitForTimeout(400);
    const nav = await measureColdNav(page);
    const observed = await readObservedLcp(page);
    next.cold_nav.push({
      nav_lcp_ms: observed ?? nav.nav_lcp_ms,
      encoded_transfer_bytes: nav.encoded_transfer_bytes,
    } satisfies ColdNavSample);
    const stream = await nextSendAndMeasure(page);
    stream.kind = "cold";
    next.streams.push(stream);
    await page.close();
  }

  const hotPage = await browser.newPage();
  await stubNextChatToFixture(hotPage, FIXTURE_BASE);
  await nextGotoChat(hotPage, NEXT_BASE);
  for (let i = 0; i < HOT_N; i += 1) {
    await hotPage.goto(`${NEXT_BASE}/chat`, { waitUntil: "domcontentloaded" });
    const stream = await nextSendAndMeasure(hotPage);
    stream.kind = "hot";
    next.streams.push(stream);
  }
  await hotPage.close();
}
