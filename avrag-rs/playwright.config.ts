import { defineConfig, devices } from "@playwright/test";

export default defineConfig({
  testDir: "./e2e",
  snapshotPathTemplate: "{testDir}/{testFilePath}-snapshots/{arg}-{projectName}{ext}",
  timeout: 90_000,
  expect: {
    timeout: 10_000,
    toHaveScreenshot: {
      maxDiffPixelRatio: 0.01,
      animations: "disabled",
      caret: "hide",
      scale: "css",
    },
  },
  fullyParallel: false,
  workers: 1,
  retries: 0,
  reporter: [["list"], ["html", { open: "never" }]],

  use: {
    baseURL: process.env.PLAYWRIGHT_BASE_URL || "http://127.0.0.1:8080",
    locale: "zh-CN",
    trace: "retain-on-failure",
    screenshot: "only-on-failure",
    video: "retain-on-failure",
  },

  projects: [
    {
      name: "chromium",
      testIgnore: [/.*visual-ui\.spec\.ts/],
      use: {
        browserName: "chromium",
        viewport: { width: 1280, height: 720 },
      },
    },
    {
      name: "visual-desktop",
      testMatch: [/.*visual-ui\.spec\.ts/],
      use: {
        browserName: "chromium",
        viewport: { width: 1440, height: 900 },
        timezoneId: "UTC",
        trace: "off",
        screenshot: "off",
        video: "off",
        colorScheme: "light",
      },
    },
    {
      name: "visual-mobile",
      testMatch: [/.*visual-ui\.spec\.ts/],
      use: {
        ...devices["Pixel 5"],
        timezoneId: "UTC",
        trace: "off",
        screenshot: "off",
        video: "off",
        colorScheme: "light",
      },
    },
  ],
});
