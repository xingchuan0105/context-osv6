import type { ReactElement } from "react";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

const mocks = vi.hoisted(() => ({
  isTauri: vi.fn(),
  getCloudSession: vi.fn(),
  cloudLogout: vi.fn(),
  authLogout: vi.fn(),
  routerReplace: vi.fn(),
  getSubscription: vi.fn(),
  probeAdminAccess: vi.fn(),
}));

vi.mock("@/lib/runtime/tauri-ipc", () => ({
  isTauri: mocks.isTauri,
}));

vi.mock("@/lib/desktop/tauri-cloud", () => ({
  getCloudSession: mocks.getCloudSession,
  cloudLogout: mocks.cloudLogout,
}));

vi.mock("../../lib/auth/context", () => ({
  useAuth: () => ({
    token: "local-token",
    user: {
      id: "u-local",
      email: "local@context-os.client",
      full_name: "Local User",
    },
    isAuthenticated: true,
    initialized: true,
    logout: mocks.authLogout,
  }),
}));

vi.mock("../../lib/settings/client", () => ({
  getSubscription: mocks.getSubscription,
}));

vi.mock("../../lib/admin/client", () => ({
  probeAdminAccess: mocks.probeAdminAccess,
}));

vi.mock("next/navigation", () => ({
  useRouter: () => ({ replace: mocks.routerReplace }),
}));

vi.mock("@/lib/ui-preferences", () => ({
  useUiPreferences: () => ({
    theme: "system" as const,
    setTheme: vi.fn(),
    setLocale: vi.fn(),
  }),
}));

import { AccountMenu } from "../../components/account-menu";

const originalLocation = window.location;

function renderMenu(): void {
  const client = new QueryClient({
    defaultOptions: { queries: { retry: false }, mutations: { retry: false } },
  });
  const tree: ReactElement = (
    <QueryClientProvider client={client}>
      <AccountMenu locale="zh-CN" />
    </QueryClientProvider>
  );
  render(tree);
}

describe("AccountMenu desktop identity", () => {
  beforeEach(() => {
    mocks.isTauri.mockReset();
    mocks.getCloudSession.mockReset();
    mocks.cloudLogout.mockReset();
    mocks.authLogout.mockReset();
    mocks.routerReplace.mockReset();
    mocks.getSubscription.mockReset().mockResolvedValue(null);
    mocks.probeAdminAccess.mockReset().mockResolvedValue(false);
    // jsdom reload is not implemented — replace the whole location object.
    const { reload, ...rest } = originalLocation;
    void reload;
    delete (window as { location?: Location }).location;
    (window as { location: Location }).location = {
      ...rest,
      reload: vi.fn(),
    } as Location;
  });

  it("shows the cloud identity and never the local B2C account in desktop", async () => {
    mocks.isTauri.mockReturnValue(true);
    mocks.getCloudSession.mockResolvedValue({
      logged_in: true,
      cloud_base: "https://app.contextlm.top",
      user: { id: "d1", email: "xingchuan0105@163.com", full_name: "邢川" },
      relay: null,
      message: "Cloud session active",
    });

    renderMenu();
    fireEvent.click(screen.getByTestId("dashboard-account-menu-trigger"));

    await waitFor(() => {
      expect(screen.getByText("邢川")).toBeInTheDocument();
    });
    expect(screen.getByText("xingchuan0105@163.com")).toBeInTheDocument();
    expect(screen.queryByText("Local User")).not.toBeInTheDocument();
    // Desktop has no web subscription concept — no plan badge.
    expect(screen.queryByTestId("account-plan-badge")).not.toBeInTheDocument();
  });

  it("logs out of the cloud session and reloads — never web /login", async () => {
    mocks.isTauri.mockReturnValue(true);
    mocks.getCloudSession.mockResolvedValue({
      logged_in: true,
      cloud_base: "https://app.contextlm.top",
      user: { id: "d1", email: "xingchuan0105@163.com", full_name: "邢川" },
      relay: null,
      message: "Cloud session active",
    });
    mocks.cloudLogout.mockResolvedValue({
      logged_out: true,
      env_updated: true,
      product_restarted: false,
      message: "ok",
    });

    renderMenu();
    fireEvent.click(screen.getByTestId("dashboard-account-menu-trigger"));
    fireEvent.click(await screen.findByTestId("dashboard-logout"));

    await waitFor(() => {
      expect(mocks.cloudLogout).toHaveBeenCalledTimes(1);
    });
    expect(window.location.reload).toHaveBeenCalled();
    expect(mocks.authLogout).not.toHaveBeenCalled();
    expect(mocks.routerReplace).not.toHaveBeenCalledWith("/login");
  });

  it("keeps web logout on the auth context with the /login redirect", async () => {
    mocks.isTauri.mockReturnValue(false);
    mocks.authLogout.mockResolvedValue(undefined);

    renderMenu();
    fireEvent.click(screen.getByTestId("dashboard-account-menu-trigger"));

    // Web identity still comes from the auth context.
    expect(screen.getByText("Local User")).toBeInTheDocument();
    fireEvent.click(screen.getByTestId("dashboard-logout"));

    await waitFor(() => {
      expect(mocks.authLogout).toHaveBeenCalledTimes(1);
    });
    expect(mocks.routerReplace).toHaveBeenCalledWith("/login");
    expect(mocks.cloudLogout).not.toHaveBeenCalled();
    expect(mocks.getCloudSession).not.toHaveBeenCalled();
  });
});
