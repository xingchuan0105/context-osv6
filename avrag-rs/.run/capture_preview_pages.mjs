import { chromium } from "playwright";
import fs from "node:fs/promises";
import path from "node:path";

const outDir = "/home/chuan/context-osv6/frontend_rust/.run/visual_compare/playwright";
await fs.mkdir(outDir, { recursive: true });

const routes = [
  ["login", "/preview/login"],
  ["dashboard", "/preview/dashboard"],
  ["workspace", "/preview/workspace"],
  ["account", "/preview/account"],
  ["settings", "/preview/settings"],
  ["help", "/preview/help"],
];

const browser = await chromium.launch({ headless: true });
const page = await browser.newPage({ viewport: { width: 1440, height: 1024 } });

for (const [name, route] of routes) {
  await page.goto(`http://127.0.0.1:4173${route}`, {
    waitUntil: "networkidle",
    timeout: 30000,
  });
  await page.screenshot({
    path: path.join(outDir, `${name}.png`),
    fullPage: false,
  });
}

await browser.close();
console.log(JSON.stringify({ outDir, files: routes.map(([name]) => `${name}.png`) }));
