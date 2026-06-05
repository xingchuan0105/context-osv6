import { defineConfig, devices } from "@playwright/test";
import path from "path";
import { config as dotenvConfig } from "dotenv";

dotenvConfig({ path: path.resolve(__dirname, "../../.env") });

/**
 * Frontend E2E configuration — full-stack, real LLM, production build.
 *
 * Design:
 * - globalSetup launches Rust backend (PG + Milvus + worker + HTTP server)
 *   via a blocking Rust test, then builds and starts Next.js frontend.
 * - globalTeardown tears down both sides.
 * - Tests run serially (workers=1) because real LLM APIs have rate limits.
 * - Failures are recorded with screenshot + video + trace for audit.
 */
export default defineConfig({
  testDir: "./specs",
  outputDir: "./output/artifacts",
  /* Real LLM tests can take 30-60s per request */
  timeout: 120_000,

  /* Serial execution — real LLM rate limits prevent parallelism */
  fullyParallel: false,
  workers: 1,

  /* Retry once on failure (real LLM can be flaky) */
  retries: 1,

  reporter: [
    ["html", { outputFolder: "./output/report" }],
    ["json", { outputFile: "./output/results.json" }],
    ["list"],
  ],

  use: {
    baseURL: "http://localhost:3001",
    trace: "on-first-retry",
    video: "on-first-retry",
    screenshot: "only-on-failure",
    /* Default viewport */
    viewport: { width: 1280, height: 720 },
  },

  globalSetup: path.resolve(__dirname, "./global-setup.ts"),
  globalTeardown: path.resolve(__dirname, "./global-teardown.ts"),

  projects: [
    {
      name: "chromium",
      use: { ...devices["Desktop Chrome"] },
    },
  ],
});
