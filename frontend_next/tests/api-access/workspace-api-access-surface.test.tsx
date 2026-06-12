import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";


const workspaceId = "550e8400-e29b-41d4-a716-446655440000";

vi.mock("../../lib/auth/context", () => ({
  useAuth: () => mocks.authState,
}));

vi.mock("../../lib/api-access/client", async () => {
  const actual = await vi.importActual("../../lib/api-access/client");

  return {
    ...actual,
    getApiAccessBaseUrl: mocks.getApiAccessBaseUrlMock,
    listApiKeys: mocks.listApiKeysMock,
    createApiKey: mocks.createApiKeyMock,
    revokeApiKey: mocks.revokeApiKeyMock,
  };
});

import { WorkspaceApiAccessSurface } from "../../components/api-access/workspace-api-access-surface";

const mocks = vi.hoisted(() => globalThis.__mockProviders.createWorkspaceApiAccessSurfaceMocks());



describe("WorkspaceApiAccessSurface", () => {
  beforeEach(() => {
    mocks.authState = {
      token: "token-123",
    };
    mocks.getApiAccessBaseUrlMock.mockReset();
    mocks.listApiKeysMock.mockReset();
    mocks.createApiKeyMock.mockReset();
    mocks.revokeApiKeyMock.mockReset();

    mocks.getApiAccessBaseUrlMock.mockReturnValue("https://api.example.test");
    mocks.listApiKeysMock.mockResolvedValue({
      api_keys: [
        {
          id: "key-1",
          org_id: "org-1",
          notebook_id: workspaceId,
          key_prefix: "sk_live_123",
          name: "Existing Key",
          permissions: ["index"],
          rate_limit_rpm: 60,
          expires_at: null,
          last_used_at: null,
          is_active: true,
          created_by: "user-1",
          created_at: "2026-04-17T10:00:00Z",
          updated_at: "2026-04-17T10:00:00Z",
        },
      ],
    });
    mocks.createApiKeyMock.mockResolvedValue({
      api_key: {
        id: "key-2",
        org_id: "org-1",
        notebook_id: workspaceId,
        key_prefix: "sk_live_456",
        name: "Admin Key",
        permissions: ["admin"],
        rate_limit_rpm: 60,
        expires_at: null,
        last_used_at: null,
        is_active: true,
        created_by: "user-1",
        created_at: "2026-04-17T11:00:00Z",
        updated_at: "2026-04-17T11:00:00Z",
      },
      plaintext_key: "sk_workspace_plaintext",
    });
    mocks.revokeApiKeyMock.mockResolvedValue({});
  });

  it("loads existing workspace api keys", async () => {
    render(<WorkspaceApiAccessSurface workspaceId={workspaceId} />);

    expect(await screen.findByText("Existing Key")).toBeTruthy();

    await waitFor(() => {
      expect(mocks.listApiKeysMock).toHaveBeenCalledWith("token-123", workspaceId);
    });
  });

  it("creates a new api key and shows the plaintext key once", async () => {
    const user = userEvent.setup();

    render(<WorkspaceApiAccessSurface workspaceId={workspaceId} />);

    await screen.findByText("Existing Key");
    await user.type(screen.getByLabelText("密钥名称"), "Admin Key");
    await user.selectOptions(screen.getByLabelText("权限"), "admin");
    await user.click(screen.getByRole("button", { name: "创建密钥" }));

    await waitFor(() => {
      expect(mocks.createApiKeyMock).toHaveBeenCalledWith("token-123", workspaceId, {
        name: "Admin Key",
        permissions: ["admin"],
        rate_limit_rpm: 60,
      });
    });

    expect(await screen.findByText("sk_workspace_plaintext")).toBeTruthy();
    expect(screen.getByText("Admin Key")).toBeTruthy();
  });

  it("revokes a listed api key from the active workspace", async () => {
    const user = userEvent.setup();

    render(<WorkspaceApiAccessSurface workspaceId={workspaceId} />);

    await screen.findByText("Existing Key");
    await user.click(screen.getByRole("button", { name: "撤销" }));

    await waitFor(() => {
      expect(mocks.revokeApiKeyMock).toHaveBeenCalledWith("token-123", workspaceId, "key-1");
    });

    expect(screen.queryByText("Existing Key")).toBeNull();
  });

  it("blocks api calls when the workspace id is obviously invalid", async () => {
    render(<WorkspaceApiAccessSurface workspaceId="not-a-uuid" />);

    expect(await screen.findByText(/Workspace ID 无效/)).toBeTruthy();
    expect(mocks.listApiKeysMock).not.toHaveBeenCalled();
  });

  it("renders the llm agent guidance links", () => {
    render(<WorkspaceApiAccessSurface workspaceId={workspaceId} />);

    expect(screen.getByRole("link", { name: "/help/api-access" }).getAttribute("href")).toBe("/help/api-access");
    expect(screen.getByRole("link", { name: "/docs/api-access-for-agents.md" }).getAttribute("href")).toBe(
      "/docs/api-access-for-agents.md",
    );
  });
});
