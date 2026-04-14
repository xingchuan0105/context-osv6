import { test } from "@playwright/test";
import { registerTestUser, createNotebookViaAPI, seedBrowserAuth, deleteNotebookViaAPI } from "./helpers";

test("debug settings and workspace browser errors", async ({ page, request }) => {
  page.on("request", (req) => {
    const url = req.url();
    if (url.includes("/api/")) {
      console.log(`[request] ${req.method()} ${url}`);
    }
  });
  page.on("response", async (resp) => {
    const url = resp.url();
    if (url.includes("/api/")) {
      console.log(`[response] ${resp.status()} ${url}`);
    }
  });
  page.on("console", (msg) => {
    if (msg.type() === "error" || msg.type() === "warning") {
      console.log(`[browser:${msg.type()}] ${msg.text()}`);
    }
  });
  page.on("pageerror", (err) => {
    console.log(`[pageerror] ${err.message}`);
  });

  const auth = await registerTestUser(request);
  const notebookId = await createNotebookViaAPI(request, auth.token, `debug-${Date.now()}`);
  await seedBrowserAuth(page, request, auth.token);

  try {
    await page.goto("/settings");
    await page.getByRole("button", { name: /外观与语言|Appearance/ }).click();
    await page.waitForTimeout(3000);

    await page.goto(`/dashboard/${notebookId}`);
    await page.waitForTimeout(3000);
    await page.getByPlaceholder(/添加网页链接源|Add URL source/).fill("http://127.0.0.1:18083/login");
    await page.getByRole("button", { name: /添加链接|Add URL/ }).click();
    await page.waitForTimeout(5000);
    console.log(`[dom] login.html count=${await page.locator("text=login.html").count()}`);
    await page.getByPlaceholder(/输入问题，围绕当前资料继续研究|Ask a question about your documents/).fill("debug chat paragraph 1");
    await page.locator("form").evaluate((form) => {
      (form as HTMLFormElement).requestSubmit();
    });
    await page.waitForTimeout(5000);
    console.log(`[dom] debug chat count=${await page.getByText(/debug chat paragraph 1/i).count()}`);
  } finally {
    await deleteNotebookViaAPI(request, auth.token, notebookId);
  }
});
