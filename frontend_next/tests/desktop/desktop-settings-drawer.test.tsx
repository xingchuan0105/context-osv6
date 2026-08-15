import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { fireEvent, render, screen, waitFor } from "@testing-library/react";

vi.mock("@/lib/runtime/tauri-ipc", () => ({
  isTauri: vi.fn(),
}));

vi.mock("@/lib/desktop/tauri-cloud", () => ({
  getCloudSession: vi.fn(),
  getCloudWalletBalance: vi.fn(),
  cloudLogin: vi.fn(),
  cloudLogout: vi.fn(),
}));

vi.mock("@/lib/desktop/tauri-license", () => ({
  openInBrowser: vi.fn(),
}));

vi.mock("@/lib/desktop/tauri-local", () => ({
  getAppDataDir: vi.fn(),
  getAppVersion: vi.fn(),
  getLocalProductStatus: vi.fn(),
  getLocalStackStatus: vi.fn(),
  openDataDir: vi.fn(),
  openLogsDir: vi.fn(),
}));

vi.mock("@/lib/auth/context", () => ({
  useAuth: () => ({ token: "test-token" }),
}));

vi.mock("@/lib/settings/client", () => ({
  listProviderSecrets: vi.fn(),
}));

vi.mock("@/lib/ui-preferences", () => ({
  useUiPreferences: () => ({ locale: "zh-CN" as const, theme: "system" as const }),
}));

// Top-bar neighbours pull in react-query / auth contexts — stub them; the
// badge-removal assertion only cares about the desktop branch chrome.
vi.mock("../../components/account-menu", () => ({
  AccountMenu: () => <div data-testid="account-menu" />,
}));
vi.mock("../../components/notifications/notification-bell", () => ({
  NotificationBell: () => <div data-testid="notification-bell" />,
}));
vi.mock("../../components/share/workspace-share-quick-modal", () => ({
  WorkspaceShareQuickModal: () => null,
}));

import { getCloudSession, getCloudWalletBalance } from "@/lib/desktop/tauri-cloud";
import { openInBrowser } from "@/lib/desktop/tauri-license";
import {
  getAppDataDir,
  getAppVersion,
  getLocalProductStatus,
  getLocalStackStatus,
} from "@/lib/desktop/tauri-local";
import { listProviderSecrets } from "@/lib/settings/client";
import { isTauri } from "@/lib/runtime/tauri-ipc";
import { DesktopSettingsDrawer } from "@/components/desktop/DesktopSettingsDrawer";
import { WorkspaceTopBar } from "@/components/workspace/workspace-top-bar";

const loggedInSession = {
  logged_in: true,
  cloud_base: "https://app.contextlm.top",
  user: { id: "u1", email: "a@b.c", full_name: "A" },
  relay: {
    base_url: "https://app.contextlm.top/v1/relay",
    chat_model: "deepseek-v4-flash",
    embedding_model: "BAAI/bge-m3",
  },
  message: "Cloud session active",
};

const loggedOutSession = {
  logged_in: false,
  cloud_base: "https://app.contextlm.top",
  user: null,
  relay: null,
  message: "No cloud session",
};

const walletView = {
  logged_in: true,
  balance_fen: 2000,
};

const productStatus = {
  overall_ok: true,
  api_ok: true,
  worker_ok: true,
  api_base_url: "http://127.0.0.1:18080",
  api_endpoint: "127.0.0.1:18080",
  health_detail: "ok",
  worker_detail: "ok",
  compose_hint: "",
  script_path: null,
  log_dir: "/state/logs",
  api_bin_path: null,
  worker_bin_path: null,
};

const stackStatus = {
  overall_ok: false,
  services: [
    { id: "pg", label: "PostgreSQL", endpoint: "127.0.0.1:15432", ok: true, detail: "native" },
    { id: "redis", label: "Redis", endpoint: "127.0.0.1:16379", ok: false, detail: "refused" },
  ],
  compose_hint: "",
  env_file_path: "/state/runtime.env",
  env_file_exists: true,
};

function mockHappyPathIpc() {
  vi.mocked(getCloudSession).mockResolvedValue(loggedInSession);
  vi.mocked(getCloudWalletBalance).mockResolvedValue(walletView);
  vi.mocked(getAppDataDir).mockResolvedValue("/state/com.contextos.desktop");
  vi.mocked(getAppVersion).mockResolvedValue("0.2.0");
  vi.mocked(getLocalProductStatus).mockResolvedValue(productStatus);
  vi.mocked(getLocalStackStatus).mockResolvedValue(stackStatus);
  vi.mocked(listProviderSecrets).mockResolvedValue({ secrets: [] });
}

describe("DesktopSettingsDrawer", () => {
  beforeEach(() => {
    mockHappyPathIpc();
  });

  afterEach(() => {
    vi.clearAllMocks();
  });

  it("renders the rail sections (账户/模型/数据/关于/诊断) on the account section by default", async () => {
    render(<DesktopSettingsDrawer open onClose={() => {}} />);

    const rail = screen.getByTestId("desktop-settings-nav-rail");
    for (const label of ["账户", "模型", "数据", "关于", "诊断"]) {
      expect(rail).toHaveTextContent(label);
    }
    expect(screen.getByRole("dialog", { name: "客户端设置" })).toBeInTheDocument();

    // Default section = 账户 with the cloud account email + wallet balance.
    await waitFor(() => {
      expect(screen.getByText("a@b.c")).toBeInTheDocument();
    });
    await waitFor(() => {
      expect(screen.getByText("¥20.00")).toBeInTheDocument();
    });
  });

  it("模型 section shows 官方模型（走余额） with the pinned relay models when signed in", async () => {
    render(<DesktopSettingsDrawer open onClose={() => {}} />);

    fireEvent.click(screen.getByRole("button", { name: "模型" }));

    await waitFor(() => {
      expect(screen.getByText("官方模型（走余额）")).toBeInTheDocument();
    });
    expect(screen.getByText("deepseek-v4-flash")).toBeInTheDocument();
    expect(screen.getByText("BAAI/bge-m3")).toBeInTheDocument();
    expect(screen.getByText("管理 Provider（自备 Key）→")).toBeInTheDocument();
  });

  it("模型 section shows 自定义 Provider（自备 Key） when signed out", async () => {
    vi.mocked(getCloudSession).mockResolvedValue(loggedOutSession);
    render(<DesktopSettingsDrawer open onClose={() => {}} />);

    fireEvent.click(screen.getByRole("button", { name: "模型" }));

    await waitFor(() => {
      expect(screen.getByText("自定义 Provider（自备 Key）")).toBeInTheDocument();
    });
  });

  it("模型 section prefers an active BYOK secret over the official relay (BYOK 优先序)", async () => {
    vi.mocked(listProviderSecrets).mockResolvedValue({
      secrets: [
        {
          id: "s1",
          purpose: "llm",
          provider: "deepseek",
          base_url: null,
          model_hint: "deepseek-chat",
          key_fingerprint: "fp-1",
          revoked_at: null,
        },
      ],
    });
    render(<DesktopSettingsDrawer open onClose={() => {}} />);

    fireEvent.click(screen.getByRole("button", { name: "模型" }));

    await waitFor(() => {
      expect(screen.getByText("自定义 Provider（自备 Key）")).toBeInTheDocument();
    });
    expect(screen.getByText("deepseek · deepseek-chat")).toBeInTheDocument();
    expect(screen.queryByText("官方模型（走余额）")).not.toBeInTheDocument();
    expect(screen.queryByText("deepseek-v4-flash")).not.toBeInTheDocument();
  });

  it("revoked BYOK secrets do not override the official source", async () => {
    vi.mocked(listProviderSecrets).mockResolvedValue({
      secrets: [
        {
          id: "s1",
          purpose: "llm",
          provider: "deepseek",
          base_url: null,
          model_hint: null,
          key_fingerprint: "fp-1",
          revoked_at: "2026-08-01T00:00:00Z",
        },
      ],
    });
    render(<DesktopSettingsDrawer open onClose={() => {}} />);

    fireEvent.click(screen.getByRole("button", { name: "模型" }));

    await waitFor(() => {
      expect(screen.getByText("官方模型（走余额）")).toBeInTheDocument();
    });
  });

  it("账户 section offers 登录 when no cloud session exists", async () => {
    vi.mocked(getCloudSession).mockResolvedValue(loggedOutSession);
    render(<DesktopSettingsDrawer open onClose={() => {}} />);

    await waitFor(() => {
      expect(screen.getByRole("button", { name: "登录" })).toBeInTheDocument();
    });
    expect(screen.getByText(/未登录云账户/)).toBeInTheDocument();
    expect(getCloudWalletBalance).not.toHaveBeenCalled();
  });

  it("诊断 stays collapsed by default and expands on selection", async () => {
    render(<DesktopSettingsDrawer open onClose={() => {}} />);

    await waitFor(() => {
      expect(screen.getByText("a@b.c")).toBeInTheDocument();
    });
    expect(screen.queryByText("PostgreSQL")).not.toBeInTheDocument();
    expect(getLocalStackStatus).not.toHaveBeenCalled();
    // Product status (logs dir source) is lazy too — not probed at drawer open.
    expect(getLocalProductStatus).not.toHaveBeenCalled();

    fireEvent.click(screen.getByRole("button", { name: "诊断" }));

    await waitFor(() => {
      expect(screen.getByText("PostgreSQL")).toBeInTheDocument();
    });
    expect(screen.getByText("OK")).toBeInTheDocument();
    expect(screen.getByText("DOWN")).toBeInTheDocument();
    expect(getLocalProductStatus).toHaveBeenCalled();
  });

  it("充值 opens /pricing#topup in the external browser", async () => {
    render(<DesktopSettingsDrawer open onClose={() => {}} />);

    await waitFor(() => {
      expect(screen.getByRole("button", { name: "充值" })).toBeInTheDocument();
    });
    fireEvent.click(screen.getByRole("button", { name: "充值" }));

    expect(openInBrowser).toHaveBeenCalledWith(expect.stringContaining("/pricing#topup"));
  });

  it("renders no dev-console surface (stack lifecycle, raw env, CLI hints)", async () => {
    const { container } = render(<DesktopSettingsDrawer open onClose={() => {}} />);

    await waitFor(() => {
      expect(screen.getByText("a@b.c")).toBeInTheDocument();
    });
    for (const section of ["模型", "数据", "关于", "诊断"]) {
      fireEvent.click(screen.getByRole("button", { name: section }));
    }
    await waitFor(() => {
      expect(getLocalStackStatus).toHaveBeenCalled();
    });

    expect(container.textContent).not.toMatch(
      /启动并迁移|停止栈|client\.env|本机个人账户|本机产品进程|Monorepo|desktop-local-stack/,
    );
  });
});

describe("WorkspaceTopBar (desktop branch)", () => {
  beforeEach(() => {
    vi.mocked(isTauri).mockReturnValue(true);
    mockHappyPathIpc();
  });

  afterEach(() => {
    vi.clearAllMocks();
  });

  function renderTopBar() {
    return render(
      <WorkspaceTopBar
        workspaceId="ws-1"
        workspaceTitle="WS"
        workspaceDescription=""
        workspaceTitleDraft="WS"
        onWorkspaceTitleDraftChange={() => {}}
        onSaveWorkspaceTitle={() => {}}
        onCreateWorkspaceSubmit={() => {}}
      />,
    );
  }

  it("keeps the 客户端设置 gear but drops the 「已激活」 license badge", () => {
    renderTopBar();

    expect(screen.getByRole("button", { name: "客户端设置" })).toBeInTheDocument();
    expect(screen.queryByText("已激活")).not.toBeInTheDocument();
    expect(screen.queryByTestId("desktop-status-badge")).not.toBeInTheDocument();
  });
});
