import { defineConfig } from "@playwright/test";

// Packaged Windows WebView2 only. This config must stay separate from
// playwright.config.ts: it must not start :3000/:8080 and must not use the
// cloud storageState/globalSetup.
export default defineConfig({
  testDir: "./e2e/specs/desktop-client",
  timeout: 180_000,
  fullyParallel: false,
  workers: 1,
  reporter: [["list"]],
  use: {
    browserName: "chromium",
    trace: "retain-on-failure",
    screenshot: "only-on-failure",
  },
});
