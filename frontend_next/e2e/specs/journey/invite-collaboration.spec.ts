import { request as playwrightRequest } from "@playwright/test";
import { test, expect } from "../../fixtures/run-context";
import { COLLAB_USER } from "../../fixtures/test-user";
import { DashboardPage } from "../../pom/dashboard-page";
import { SharePage } from "../../pom/share-page";
import {
  ensureE2eOrgMember,
  listNotebookMembers,
  loginAndPrepareUserSession,
  resetAndPrepareTestUser,
  seedBrowserPageAuth,
} from "../../utils/api-helpers";

test.describe("Invite Collaboration", () => {
  test.beforeAll(async ({ request }) => {
    await resetAndPrepareTestUser(request);
    await ensureE2eOrgMember(request);
  });

  test("user A invites user B and B accepts via invite page", async ({
    browser,
    page,
    request,
  }) => {
    test.setTimeout(180_000);

    const dashboard = new DashboardPage(page);
    const share = new SharePage(page);

    await page.goto("/dashboard");
    await dashboard.createWorkspace();

    const workspaceId = page.url().match(/\/dashboard\/([^/]+)/)?.[1];
    if (!workspaceId) {
      throw new Error("Failed to extract workspaceId from URL");
    }

    await share.goto(workspaceId);
    await share.inviteMember(COLLAB_USER.email);

    const members = await listNotebookMembers(request, workspaceId);
    const pendingMember = members.members.find(
      (member) =>
        member.email.trim().toLowerCase() === COLLAB_USER.email.toLowerCase() &&
        member.status === "pending",
    );
    if (!pendingMember) {
      throw new Error(`pending invite for ${COLLAB_USER.email} not found in members list`);
    }

    const inviteUrl = `/invite/${workspaceId}/${pendingMember.member_id}`;

    const userBContext = await browser.newContext();
    const userBPage = await userBContext.newPage();
    // User B must log in via a fresh APIRequestContext with no inherited storageState.
    // The journey `request` fixture carries user A's session (playwright/.auth/user.json);
    // sending user A's auth cookie to /api/auth/login re-issues user A's token instead of
    // authenticating user B, so userBPage would act as user A and the accept would fail
    // with "invite not allowed" (actor email ≠ invite email).
    const collabRequest = await playwrightRequest.newContext({
      baseURL: process.env.PLAYWRIGHT_BASE_URL || "http://127.0.0.1:3000",
    });
    try {
      const collabAuth = await loginAndPrepareUserSession(
        collabRequest,
        COLLAB_USER.email,
        COLLAB_USER.password,
      );
      expect(collabAuth.user.email).toBe(COLLAB_USER.email);
      await seedBrowserPageAuth(userBPage, collabAuth);

      await userBPage.goto(inviteUrl);
      await expect(userBPage.locator('[data-testid="invite-surface"]')).toBeVisible();

      await userBPage.locator('[data-testid="invite-accept-button"]').click();
      await expect(userBPage.getByText(/已接受邀请|accepted/i)).toBeVisible({
        timeout: 30_000,
      });

      await userBPage.getByRole("link", { name: /打开 Workspace|Open Workspace/i }).click();
      await userBPage.waitForURL(new RegExp(`/dashboard/${workspaceId}`), { timeout: 60_000 });
      await expect(userBPage.locator('[data-testid="workspace-top-bar"]')).toBeVisible({
        timeout: 30_000,
      });
    } finally {
      await userBContext.close().catch(() => {});
      await collabRequest.dispose().catch(() => {});
    }
  });
});
