import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { fireEvent, render, screen, waitFor } from "@testing-library/react";

vi.mock("@/lib/runtime/tauri-ipc", () => ({
  isTauri: vi.fn(),
}));

vi.mock("@/lib/desktop/tauri-cloud", () => ({
  getCloudSession: vi.fn(),
  cloudLogin: vi.fn(),
  cloudLogout: vi.fn(),
  isCloudGateBypassed: vi.fn(),
}));

vi.mock("@/lib/desktop/tauri-license", () => ({
  openInBrowser: vi.fn(),
}));

vi.mock("@/lib/ui-preferences", () => ({
  useUiPreferences: () => ({ locale: "zh-CN" as const, theme: "system" as const }),
}));

import { cloudLogin, getCloudSession, isCloudGateBypassed } from "@/lib/desktop/tauri-cloud";
import { openInBrowser } from "@/lib/desktop/tauri-license";
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
    ingestion_model: "qwen3.7-flash",
    embedding_model: "BAAI/bge-m3",
    rerank_model: "Pro/BAAI/bge-reranker-v2-m3",
  },
  message: "Cloud session active",
};

describe("CloudLoginGate", () => {
  beforeEach(() => {
    vi.mocked(isTauri).mockReset();
    vi.mocked(getCloudSession).mockReset();
    vi.mocked(cloudLogin).mockReset();
    vi.mocked(isCloudGateBypassed).mockReset();
    vi.mocked(isCloudGateBypassed).mockResolvedValue(false);
    vi.mocked(openInBrowser).mockReset();
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

  it("renders children without a session when the E2E bypass env is set", async () => {
    vi.mocked(isTauri).mockReturnValue(true);
    vi.mocked(isCloudGateBypassed).mockResolvedValue(true);

    render(
      <CloudLoginGate>
        <div data-testid="app-child">App</div>
      </CloudLoginGate>,
    );

    await waitFor(() => {
      expect(screen.getByTestId("app-child")).toBeInTheDocument();
    });
    expect(getCloudSession).not.toHaveBeenCalled();
    expect(screen.queryByText("欢迎使用 Context-OS")).not.toBeInTheDocument();
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
      expect(screen.getByText("欢迎使用 Context-OS")).toBeInTheDocument();
    });
    expect(screen.queryByTestId("app-child")).not.toBeInTheDocument();
    expect(screen.getByText(/官方模型开箱即用/)).toBeInTheDocument();
    expect(screen.getByText(/自备的 API Key \(BYOK\)/)).toBeInTheDocument();
    expect(screen.getByText(/数据隐私承诺/)).toBeInTheDocument();
  });

  it("opens the cloud register page in the system browser from the login card", async () => {
    vi.mocked(isTauri).mockReturnValue(true);
    vi.mocked(getCloudSession).mockResolvedValue(loggedOutSession);

    render(
      <CloudLoginGate>
        <div data-testid="app-child">App</div>
      </CloudLoginGate>,
    );

    await waitFor(() => {
      expect(screen.getByText("还没有账号？")).toBeInTheDocument();
    });
    fireEvent.click(screen.getByRole("button", { name: "立即注册 →" }));

    await waitFor(() => {
      expect(openInBrowser).toHaveBeenCalledTimes(1);
    });
    expect(vi.mocked(openInBrowser).mock.calls[0]?.[0]).toContain("/register");
  });

  it("opens the cloud reset password page in the system browser from the forgot password button", async () => {
    vi.mocked(isTauri).mockReturnValue(true);
    vi.mocked(getCloudSession).mockResolvedValue(loggedOutSession);

    render(
      <CloudLoginGate>
        <div data-testid="app-child">App</div>
      </CloudLoginGate>,
    );

    await waitFor(() => {
      expect(screen.getByRole("button", { name: "忘记密码？" })).toBeInTheDocument();
    });
    fireEvent.click(screen.getByRole("button", { name: "忘记密码？" }));

    await waitFor(() => {
      expect(openInBrowser).toHaveBeenCalledTimes(1);
    });
    expect(vi.mocked(openInBrowser).mock.calls[0]?.[0]).toContain("/reset-password");
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
    expect(screen.queryByText("欢迎使用 Context-OS")).not.toBeInTheDocument();
  });

  it("submits credentials through IPC and proceeds on success", async () => {
    vi.mocked(isTauri).mockReturnValue(true);
    vi.mocked(getCloudSession).mockResolvedValue(loggedOutSession);
    vi.mocked(cloudLogin).mockResolvedValue({
      user: { id: "u1", email: "a@b.c", full_name: "A" },
      relay: {
        base_url: "https://app.contextlm.top/v1/relay",
        chat_model: "deepseek-v4-flash",
        ingestion_model: "qwen3.7-flash",
        embedding_model: "BAAI/bge-m3",
        rerank_model: "Pro/BAAI/bge-reranker-v2-m3",
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
      expect(screen.getByLabelText("账号邮箱")).toBeInTheDocument();
    });
    fireEvent.change(screen.getByLabelText("账号邮箱"), { target: { value: "a@b.c" } });
    fireEvent.change(screen.getByLabelText("密码"), { target: { value: "secret-1" } });
    fireEvent.click(screen.getByRole("button", { name: "登录并进入工作区" }));

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
      expect(screen.getByLabelText("账号邮箱")).toBeInTheDocument();
    });
    fireEvent.change(screen.getByLabelText("账号邮箱"), { target: { value: "a@b.c" } });
    fireEvent.change(screen.getByLabelText("密码"), { target: { value: "wrong" } });
    fireEvent.click(screen.getByRole("button", { name: "登录并进入工作区" }));

    await waitFor(() => {
      expect(screen.getByText("邮箱或密码错误")).toBeInTheDocument();
    });
    expect(screen.queryByTestId("app-child")).not.toBeInTheDocument();
  });
});
