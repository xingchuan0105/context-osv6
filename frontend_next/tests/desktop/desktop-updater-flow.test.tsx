import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { fireEvent, render, screen, waitFor } from "@testing-library/react";

vi.mock("@/lib/runtime/tauri-ipc", () => ({
  isTauri: vi.fn(() => true),
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

// Updater wrapper is mocked at the module boundary — the drawer owns the
// state machine under test here (idle → checking → available → downloading
// → installing, plus error and upToDate terminals).
vi.mock("@/lib/desktop/tauri-updater", () => ({
  desktopUpdateSupported: vi.fn(),
  checkForUpdate: vi.fn(),
  downloadAndInstallUpdate: vi.fn(),
}));

vi.mock("@/lib/auth/context", () => ({
  useAuth: () => ({ token: null }),
}));

vi.mock("@/lib/settings/client", () => ({
  listProviderSecrets: vi.fn(),
}));

vi.mock("@/lib/ui-preferences", () => ({
  useUiPreferences: () => ({ locale: "zh-CN" as const, theme: "system" as const }),
}));

import {
  checkForUpdate,
  desktopUpdateSupported,
  downloadAndInstallUpdate,
} from "@/lib/desktop/tauri-updater";
import { getCloudSession } from "@/lib/desktop/tauri-cloud";
import { getAppDataDir, getAppVersion } from "@/lib/desktop/tauri-local";
import { DesktopSettingsDrawer } from "@/components/desktop/DesktopSettingsDrawer";

const loggedInSession = {
  logged_in: false,
  cloud_base: "https://app.contextlm.top",
  user: null,
  relay: null,
  message: "No cloud session",
};

function openAboutSection() {
  render(<DesktopSettingsDrawer open onClose={() => {}} />);
  fireEvent.click(screen.getByText("关于"));
}

beforeEach(() => {
  vi.mocked(getCloudSession).mockResolvedValue(loggedInSession);
  vi.mocked(getAppDataDir).mockResolvedValue("/state/com.contextos.desktop");
  vi.mocked(getAppVersion).mockResolvedValue("0.4.0");
});

afterEach(() => {
  vi.clearAllMocks();
});

describe("DesktopSettingsDrawer 检查更新", () => {
  it("desktop 环境渲染「检查更新」按钮；查新无更新 → 已是最新版本", async () => {
    vi.mocked(desktopUpdateSupported).mockResolvedValue(true);
    vi.mocked(checkForUpdate).mockResolvedValue({ kind: "upToDate" });

    openAboutSection();

    const button = await screen.findByRole("button", { name: "检查更新" });
    fireEvent.click(button);
    await waitFor(() => {
      expect(screen.getByRole("button", { name: "已是最新版本" })).toBeInTheDocument();
    });
    expect(checkForUpdate).toHaveBeenCalledTimes(1);
  });

  it("发现新版本 → 「下载并安装」+ 版本提示；安装期进度文案随 progress 更新", async () => {
    vi.mocked(desktopUpdateSupported).mockResolvedValue(true);
    vi.mocked(checkForUpdate).mockResolvedValue({ kind: "available", version: "0.5.0" });
    vi.mocked(downloadAndInstallUpdate).mockImplementation(async (onProgress) => {
      onProgress?.(37);
      return { kind: "installing" };
    });

    openAboutSection();

    const check = await screen.findByRole("button", { name: "检查更新" });
    fireEvent.click(check);
    const install = await screen.findByRole("button", { name: "下载并安装" });
    expect(
      screen.getByText("发现新版本 v0.5.0，下载后将自动安装并重启。")
    ).toBeInTheDocument();

    fireEvent.click(install);
    await waitFor(() => {
      expect(screen.getByText("已下载 37%")).toBeInTheDocument();
    });
    await waitFor(() => {
      expect(screen.getByRole("button", { name: "正在更新…" })).toBeDisabled();
    });
    expect(downloadAndInstallUpdate).toHaveBeenCalledTimes(1);
  });

  it("查新失败 → 展示错误信息且按钮回到可点的「检查更新」", async () => {
    vi.mocked(desktopUpdateSupported).mockResolvedValue(true);
    vi.mocked(checkForUpdate).mockResolvedValue({
      kind: "error",
      message: "updater endpoint unreachable",
    });

    openAboutSection();

    const button = await screen.findByRole("button", { name: "检查更新" });
    fireEvent.click(button);
    await waitFor(() => {
      expect(screen.getByText("updater endpoint unreachable")).toBeInTheDocument();
    });
    expect(screen.getByRole("button", { name: "检查更新" })).toBeEnabled();
  });

  it("非桌面环境（网页构建）不渲染更新按钮", async () => {
    vi.mocked(desktopUpdateSupported).mockResolvedValue(false);

    openAboutSection();

    await waitFor(() => {
      expect(desktopUpdateSupported).toHaveBeenCalled();
    });
    expect(screen.queryByRole("button", { name: "检查更新" })).not.toBeInTheDocument();
  });
});
