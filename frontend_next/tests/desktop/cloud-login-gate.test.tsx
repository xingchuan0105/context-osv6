import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { fireEvent, render, screen, waitFor } from "@testing-library/react";

vi.mock("@/lib/runtime/tauri-ipc", () => ({
  isTauri: vi.fn(),
}));

vi.mock("@/lib/desktop/tauri-cloud", () => ({
  getCloudSession: vi.fn(),
  cloudLogin: vi.fn(),
  cloudLogout: vi.fn(),
}));

vi.mock("@/lib/ui-preferences", () => ({
  useUiPreferences: () => ({ locale: "zh-CN" as const, theme: "system" as const }),
}));

import { cloudLogin, getCloudSession } from "@/lib/desktop/tauri-cloud";
import { isTauri } from "@/lib/runtime/tauri-ipc";
import { CloudLoginGate } from "@/components/desktop/CloudLoginGate";

const loggedOutSession = {
  logged_in: false,
  cloud_base: "https://app.contextlm.top",
  user: null,
  relay: null,
  message: "No cloud session",
};

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

describe("CloudLoginGate", () => {
  beforeEach(() => {
    vi.mocked(isTauri).mockReset();
    vi.mocked(getCloudSession).mockReset();
    vi.mocked(cloudLogin).mockReset();
  });

  afterEach(() => {
    vi.clearAllMocks();
  });

  it("renders children immediately outside Tauri", () => {
    vi.mocked(isTauri).mockReturnValue(false);

    render(
      <CloudLoginGate>
        <div data-testid="app-child">App</div>
      </CloudLoginGate>,
    );

    expect(screen.getByTestId("app-child")).toBeInTheDocument();
    expect(getCloudSession).not.toHaveBeenCalled();
  });

  it("shows the login card when no cloud session exists", async () => {
    vi.mocked(isTauri).mockReturnValue(true);
    vi.mocked(getCloudSession).mockResolvedValue(loggedOutSession);

    render(
      <CloudLoginGate>
        <div data-testid="app-child">App</div>
      </CloudLoginGate>,
    );

    await waitFor(() => {
      expect(screen.getByText("登录云账户")).toBeInTheDocument();
    });
    expect(screen.queryByTestId("app-child")).not.toBeInTheDocument();
    // 官方模型（走余额）+ BYOK hint per PRODUCT_IA §6.
    expect(screen.getByText(/官方模型（走余额）/)).toBeInTheDocument();
    expect(screen.getByText(/自定义 Provider（自备 Key）/)).toBeInTheDocument();
  });

  it("renders children when a cloud session exists", async () => {
    vi.mocked(isTauri).mockReturnValue(true);
    vi.mocked(getCloudSession).mockResolvedValue(loggedInSession);

    render(
      <CloudLoginGate>
        <div data-testid="app-child">App</div>
      </CloudLoginGate>,
    );

    await waitFor(() => {
      expect(screen.getByTestId("app-child")).toBeInTheDocument();
    });
    expect(screen.queryByText("登录云账户")).not.toBeInTheDocument();
  });

  it("submits credentials through IPC and proceeds on success", async () => {
    vi.mocked(isTauri).mockReturnValue(true);
    vi.mocked(getCloudSession).mockResolvedValue(loggedOutSession);
    vi.mocked(cloudLogin).mockResolvedValue({
      user: { id: "u1", email: "a@b.c", full_name: "A" },
      relay: {
        base_url: "https://app.contextlm.top/v1/relay",
        chat_model: "deepseek-v4-flash",
        embedding_model: "BAAI/bge-m3",
      },
      env_updated: true,
      product_restarted: false,
      message: "ok",
    });

    render(
      <CloudLoginGate>
        <div data-testid="app-child">App</div>
      </CloudLoginGate>,
    );

    await waitFor(() => {
      expect(screen.getByLabelText("云账户邮箱")).toBeInTheDocument();
    });
    fireEvent.change(screen.getByLabelText("云账户邮箱"), { target: { value: "a@b.c" } });
    fireEvent.change(screen.getByLabelText("密码"), { target: { value: "secret-1" } });
    fireEvent.click(screen.getByRole("button", { name: "登录并启用官方模型" }));

    await waitFor(() => {
      expect(screen.getByTestId("app-child")).toBeInTheDocument();
    });
    expect(cloudLogin).toHaveBeenCalledWith("a@b.c", "secret-1");
  });

  it("shows the inline error on failed login", async () => {
    vi.mocked(isTauri).mockReturnValue(true);
    vi.mocked(getCloudSession).mockResolvedValue(loggedOutSession);
    vi.mocked(cloudLogin).mockRejectedValue(new Error("邮箱或密码错误"));

    render(
      <CloudLoginGate>
        <div data-testid="app-child">App</div>
      </CloudLoginGate>,
    );

    await waitFor(() => {
      expect(screen.getByLabelText("云账户邮箱")).toBeInTheDocument();
    });
    fireEvent.change(screen.getByLabelText("云账户邮箱"), { target: { value: "a@b.c" } });
    fireEvent.change(screen.getByLabelText("密码"), { target: { value: "wrong" } });
    fireEvent.click(screen.getByRole("button", { name: "登录并启用官方模型" }));

    await waitFor(() => {
      expect(screen.getByText("邮箱或密码错误")).toBeInTheDocument();
    });
    expect(screen.queryByTestId("app-child")).not.toBeInTheDocument();
  });
});
