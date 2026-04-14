import { chromium } from "playwright";

const routes = [
  "/preview/login",
  "/preview/dashboard",
  "/preview/workspace",
  "/preview/account",
  "/preview/settings",
  "/preview/help",
];

const browser = await chromium.launch({ headless: true });
const page = await browser.newPage();

for (const route of routes) {
  const errors = [];
  const onPageError = (err) => errors.push(String(err));
  page.on("pageerror", onPageError);
  const response = await page.goto(`http://127.0.0.1:4173${route}`, {
    waitUntil: "networkidle",
    timeout: 30000,
  });
  const heading = await page.locator("h1").first().innerText().catch(() => "");
  console.log(
    JSON.stringify({
      route,
      status: response?.status() ?? null,
      pageErrors: errors.length,
      heading,
    }),
  );
  page.off("pageerror", onPageError);
}

await browser.close();
