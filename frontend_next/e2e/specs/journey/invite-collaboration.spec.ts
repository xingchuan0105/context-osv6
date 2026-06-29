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
    try {
      const collabAuth = await loginAndPrepareUserSession(
        request,
        COLLAB_USER.email,
        COLLAB_USER.password,
      );
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
    }
  });
});
