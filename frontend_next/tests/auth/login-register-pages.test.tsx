import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";


vi.mock("next/navigation", () => ({
  useRouter: mocks.useRouterMock,
  useSearchParams: mocks.useSearchParamsMock,
}));

vi.mock("../../lib/auth/client", () => ({
  login: mocks.loginMock,
  register: mocks.registerMock,
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

import LoginPage from "../../app/(auth)/login/page";
import RegisterPage from "../../app/(auth)/register/page";
import {
  PUBLISHED_PRIVACY_VERSION,
  PUBLISHED_TERMS_VERSION,
} from "../../lib/legal/versions";

const mocks = vi.hoisted(() => globalThis.__mockProviders.createLoginRegisterMocks());



beforeEach(() => {
  mocks.completeAuthMock.mockReset();
  mocks.loginMock.mockReset();
  mocks.registerMock.mockReset();
  mocks.replaceMock.mockReset();
  mocks.useRouterMock.mockReset();
  mocks.useSearchParamsMock.mockReset();
  mocks.useAuthMock.mockReset();

  mocks.useRouterMock.mockReturnValue({ replace: mocks.replaceMock });
  mocks.useSearchParamsMock.mockReturnValue(new URLSearchParams());
  mocks.useAuthMock.mockReturnValue({
    initialized: true,
    isAuthenticated: false,
    passwordResetEnabled: true,
    completeAuth: mocks.completeAuthMock,
  });
});

afterEach(() => {
  vi.clearAllMocks();
});

describe("login and register pages", () => {
  it("submits login successfully and redirects to dashboard", async () => {
    const user = userEvent.setup();
    const payload = {
      token: "token-123",
      reset_ticket: null,
      user: {
        id: "user-1",
        email: "user@example.com",
        full_name: "User Example",
      },
    };

    mocks.loginMock.mockResolvedValue({
      success: true,
      data: payload,
      error: null,
    });

    render(<LoginPage />);

    expect((screen.getByLabelText("密码") as HTMLInputElement).placeholder).toBe("至少 8 位");

    await user.type(screen.getByLabelText("邮箱"), "user@example.com");
    await user.type(screen.getByLabelText("密码"), "password123");
    await user.click(screen.getByRole("button", { name: "继续登录" }));

    await waitFor(() => {
      expect(mocks.loginMock).toHaveBeenCalledWith({
        email: "user@example.com",
        password: "password123",
      });
    });

    await waitFor(() => {
      expect(mocks.completeAuthMock).toHaveBeenCalledWith(payload);
      expect(mocks.replaceMock).toHaveBeenCalledWith("/dashboard");
    });
  });

  it("renders login request errors inline", async () => {
    const user = userEvent.setup();

    mocks.loginMock.mockResolvedValue({
      success: false,
      data: null,
      error: "邮箱或密码不正确。",
    });

    render(<LoginPage />);

    await user.type(screen.getByLabelText("邮箱"), "user@example.com");
    await user.type(screen.getByLabelText("密码"), "wrong-password");
    await user.click(screen.getByRole("button", { name: "继续登录" }));

    await waitFor(() => {
      expect(screen.getByText("邮箱或密码不正确。")).toBeTruthy();
    });
  });

  it("submits registration successfully and redirects to dashboard", async () => {
    const user = userEvent.setup();
    const payload = {
      token: "token-456",
      reset_ticket: null,
      user: {
        id: "user-2",
        email: "new@example.com",
        full_name: "New User",
      },
    };

    mocks.registerMock.mockResolvedValue({
      success: true,
      data: payload,
      error: null,
    });

    render(<RegisterPage />);

    await user.type(screen.getByLabelText("姓名"), "New User");
    await user.type(screen.getByLabelText("邮箱"), "new@example.com");
    await user.type(screen.getByLabelText("密码"), "password123");
    await user.type(screen.getByLabelText("确认密码"), "password123");
    await user.click(screen.getByRole("checkbox"));
    await user.click(screen.getByRole("button", { name: "创建账号" }));

    await waitFor(() => {
      expect(mocks.registerMock).toHaveBeenCalledWith({
        email: "new@example.com",
        password: "password123",
        full_name: "New User",
        terms_version: PUBLISHED_TERMS_VERSION,
        privacy_version: PUBLISHED_PRIVACY_VERSION,
      });
    });

    await waitFor(() => {
      expect(mocks.completeAuthMock).toHaveBeenCalledWith(payload);
      expect(mocks.replaceMock).toHaveBeenCalledWith("/dashboard");
    });
  });

  it("renders registration request errors inline", async () => {
    const user = userEvent.setup();

    mocks.registerMock.mockResolvedValue({
      success: false,
      data: null,
      error: "该邮箱已被注册。",
    });

    render(<RegisterPage />);

    await user.type(screen.getByLabelText("邮箱"), "taken@example.com");
    await user.type(screen.getByLabelText("密码"), "password123");
    await user.type(screen.getByLabelText("确认密码"), "password123");
    await user.click(screen.getByRole("checkbox"));
    await user.click(screen.getByRole("button", { name: "创建账号" }));

    await waitFor(() => {
      expect(screen.getByText("该邮箱已被注册。")).toBeTruthy();
    });
  });
});
