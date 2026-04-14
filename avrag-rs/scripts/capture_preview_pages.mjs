#!/usr/bin/env node

import { chromium } from "playwright";
import fs from "node:fs/promises";
import path from "node:path";
import { fileURLToPath } from "node:url";

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);
const repoRoot = path.resolve(__dirname, "..");
const frontendRunRoot = path.resolve(
  repoRoot,
  "..",
  "frontend_rust",
  ".run",
  "visual_compare",
);

const baseUrl = process.env.PARITY_BASE_URL || "http://127.0.0.1:4173";
const outDir =
  process.env.PARITY_PLAYWRIGHT_DIR ||
  path.join(frontendRunRoot, "playwright");

const targets = [
  { name: "login", route: "/preview/login" },
  { name: "dashboard", route: "/preview/dashboard" },
  { name: "workspace", route: "/preview/workspace" },
  { name: "account", route: "/preview/account" },
  { name: "settings", route: "/preview/settings" },
  { name: "help", route: "/preview/help" },
];

await fs.mkdir(outDir, { recursive: true });

const browser = await chromium.launch({ headless: true });
const page = await browser.newPage({ viewport: { width: 1440, height: 1024 } });

for (const target of targets) {
  await page.goto(`${baseUrl}${target.route}`, {
    waitUntil: "networkidle",
    timeout: 30000,
  });
  await page.addStyleTag({
    content: `
      *, *::before, *::after {
        transition-duration: 0s !important;
        transition-delay: 0s !important;
        animation-duration: 0s !important;
        animation-delay: 0s !important;
        caret-color: transparent !important;
      }
    `,
  });
  await page.waitForTimeout(100);
  await page.screenshot({
    path: path.join(outDir, `${target.name}.png`),
    fullPage: false,
  });
}

await browser.close();

console.log(
  JSON.stringify(
    {
      baseUrl,
      outDir,
      captured: targets.map((target) => `${target.name}.png`),
    },
    null,
    2,
  ),
);
