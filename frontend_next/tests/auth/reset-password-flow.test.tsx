import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import {
  readResetEmail,
  readResetTicket,
  storeResetEmail,
  storeResetTicket,
} from "../../lib/auth/reset-state";

const mocks = vi.hoisted(() => ({
  replaceMock: vi.fn(),
  useAuthMock: vi.fn(),
  useRouterMock: vi.fn(),
  sendResetCodeMock: vi.fn(),
  verifyResetCodeMock: vi.fn(),
  confirmResetPasswordMock: vi.fn(),
}));

vi.mock("next/navigation", () => ({
  useRouter: mocks.useRouterMock,
}));

vi.mock("../../lib/auth/context", () => ({
  useAuth: mocks.useAuthMock,
}));

vi.mock("../../lib/ui-preferences", () => ({
  useUiPreferences: () => ({
    locale: "zh-CN" as const,
    theme: "system" as const,
    setLocale: vi.fn(),
    setTheme: vi.fn(),
  }),
}));

vi.mock("../../lib/auth/client", () => ({
  confirmResetPassword: mocks.confirmResetPasswordMock,
  sendResetCode: mocks.sendResetCodeMock,
  verifyResetCode: mocks.verifyResetCodeMock,
}));

import ConfirmResetPage from "../../app/(auth)/reset-password/confirm/page";
import ResetPasswordPage from "../../app/(auth)/reset-password/page";
import VerifyResetPage from "../../app/(auth)/reset-password/verify/page";

beforeEach(() => {
  window.sessionStorage.clear();
  mocks.replaceMock.mockReset();
  mocks.useAuthMock.mockReset();
  mocks.useRouterMock.mockReset();
  mocks.sendResetCodeMock.mockReset();
  mocks.verifyResetCodeMock.mockReset();
  mocks.confirmResetPasswordMock.mockReset();
  mocks.useRouterMock.mockReturnValue({ replace: mocks.replaceMock });
  mocks.useAuthMock.mockReturnValue({ passwordResetEnabled: true });
  mocks.sendResetCodeMock.mockResolvedValue({
    success: true,
    data: {
      token: "token-1",
      reset_ticket: null,
      user: { id: "user-1", email: "user@example.com", full_name: "User Example" },
    },
    error: null,
  });
  mocks.verifyResetCodeMock.mockResolvedValue({
    success: true,
    data: {
      token: "token-123",
      reset_ticket: "ticket-123",
      user: { id: "user-1", email: "user@example.com", full_name: "User Example" },
    },
    error: null,
  });
  mocks.confirmResetPasswordMock.mockResolvedValue({
    success: true,
    data: {
      token: "token-456",
      reset_ticket: null,
      user: { id: "user-1", email: "user@example.com", full_name: "User Example" },
    },
    error: null,
  });
});

afterEach(() => {
  vi.clearAllMocks();
});

describe("reset password flow pages", () => {
  it("renders an unavailable state on the send-code page when the feature is disabled", () => {
    mocks.useAuthMock.mockReturnValue({ passwordResetEnabled: false });

    render(<ResetPasswordPage />);

    expect(screen.getByText("密码重置暂不可用。", { selector: ".app-notice-banner" })).toBeTruthy();
    expect(screen.queryByLabelText("邮箱")).toBeNull();
    expect(screen.queryByRole("button", { name: "发送验证码" })).toBeNull();
  });

  it("sends the reset code and stores the reset email on the first step", async () => {
    const user = userEvent.setup();

    render(<ResetPasswordPage />);

    await user.type(screen.getByLabelText("邮箱"), "user@example.com");
    await user.click(screen.getByRole("button", { name: "发送验证码" }));

    await waitFor(() => {
      expect(mocks.sendResetCodeMock).toHaveBeenCalledWith({
        email: "user@example.com",
        lang: "zh-CN",
      });
    });

    await waitFor(() => {
      expect(readResetEmail()).toBe("user@example.com");
      expect(mocks.replaceMock).toHaveBeenCalledWith("/reset-password/verify");
    });
  });

  it("preloads the reset email, verifies the code, and stores the reset ticket", async () => {
    const user = userEvent.setup();

    storeResetEmail("user@example.com");

    render(<VerifyResetPage />);

    expect((screen.getByLabelText("邮箱") as HTMLInputElement).value).toBe("user@example.com");

    await user.type(screen.getByLabelText("验证码"), "123456");
    await user.click(screen.getByRole("button", { name: "继续" }));

    await waitFor(() => {
      expect(mocks.verifyResetCodeMock).toHaveBeenCalledWith({
        email: "user@example.com",
        code: "123456",
      });
    });

    await waitFor(() => {
      expect(readResetTicket()).toBe("ticket-123");
      expect(mocks.replaceMock).toHaveBeenCalledWith("/reset-password/confirm");
    });
  });

  it("renders an unavailable state on the confirm page when no reset ticket is stored", () => {
    render(<ConfirmResetPage />);

    expect(screen.getByText("请先完成验证码验证。", { selector: ".app-notice-banner" })).toBeTruthy();
    expect(screen.queryByLabelText("新密码")).toBeNull();
  });

  it("confirms the new password, clears reset flow state, and returns to login", async () => {
    const user = userEvent.setup();

    storeResetEmail("user@example.com");
    storeResetTicket("ticket-123");

    render(<ConfirmResetPage />);

    await user.type(screen.getByLabelText("新密码"), "new-pass");
    await user.click(screen.getByRole("button", { name: "完成重置" }));

    await waitFor(() => {
      expect(mocks.confirmResetPasswordMock).toHaveBeenCalledWith({
        reset_ticket: "ticket-123",
        new_password: "new-pass",
      });
    });

    await waitFor(() => {
      expect(readResetEmail()).toBeNull();
      expect(readResetTicket()).toBeNull();
      expect(mocks.replaceMock).toHaveBeenCalledWith("/login");
    });
  });
});
